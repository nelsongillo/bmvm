use crate::alloc::{Accessible, GuestOnly, Perm, ReadOnly, ReadWrite, WriteOnly};
use bmvm_common::mem::{Align, AlignedNonZeroUsize, Arena, DefaultAlign, PhysAddr};
use core::ffi::c_void;
use kvm_bindings::{
    kvm_create_guest_memfd, kvm_userspace_memory_region, kvm_userspace_memory_region2,
};
use kvm_ioctls::{Cap, VmFd};
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous};
use std::cmp::min;
use std::fs::File;
use std::io::Write;
use std::marker::PhantomData;
use std::ops::Range;
use std::os::fd::RawFd;
use std::panic;
use std::ptr::NonNull;
use std::slice;

const MMAP_FLAGS: [MapFlags; 2] = [MapFlags::MAP_PRIVATE, MapFlags::MAP_ANONYMOUS];

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("kvm errno: {0}")]
    KvmErrno(#[from] kvm_ioctls::Error),

    #[error("nix errno: {0}")]
    NixErrno(#[from] nix::errno::Errno),

    #[error("io error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("invalid offset: {offset} (max: {max})")]
    InvalidOffset { offset: usize, max: usize },

    /// The provided address is not included in the region address space.
    /// Provided Address, Starting Address, Size
    #[error("invalid address: {addr:#x} (start: {start:#x}, size: {size})")]
    InvalidAddress {
        addr: u64,
        start: PhysAddr,
        size: usize,
    },

    #[error("region for addr {0:x} not in collection")]
    RegionNotFound(PhysAddr),

    #[error("region at {0:x} is not readable")]
    NotReadable(PhysAddr),

    #[error("failed to set region as user memory ({0:#x}): {1}")]
    RegionMappingFailed(PhysAddr, kvm_ioctls::Error),

    #[error("failed to remove region from user memory ({0:#x}): {1}")]
    RegionUnmappingFailed(PhysAddr, kvm_ioctls::Error),
}

enum StorageBackend {
    Mmap(NonNull<u8>),
    GuestMem(RawFd, NonNull<u8>),
}

pub struct RegionCollection {
    inner: Vec<(Range<usize>, RegionEntry)>,
}

pub enum RegionEntry {
    ReadOnly(Region<ReadOnly, DefaultAlign>),
    WriteOnly(Region<WriteOnly, DefaultAlign>),
    ReadWrite(Region<ReadWrite, DefaultAlign>),
    GuestOnly(Region<GuestOnly, DefaultAlign>),
}

impl RegionEntry {
    pub fn addr(&self) -> PhysAddr {
        match self {
            RegionEntry::ReadOnly(r) => r.addr(),
            RegionEntry::WriteOnly(r) => r.addr(),
            RegionEntry::ReadWrite(r) => r.addr(),
            RegionEntry::GuestOnly(r) => r.addr(),
        }
    }

    pub fn as_ptr(&self) -> *const u8 {
        match self {
            RegionEntry::ReadOnly(r) => r.as_ptr(),
            RegionEntry::WriteOnly(r) => r.as_ptr(),
            RegionEntry::ReadWrite(r) => r.as_ptr(),
            _ => panic!("GuestOnly regions do not have a pointer"),
        }
    }

    pub fn capacity(&self) -> AlignedNonZeroUsize {
        match self {
            RegionEntry::ReadOnly(r) => r.capacity,
            RegionEntry::WriteOnly(r) => r.capacity,
            RegionEntry::ReadWrite(r) => r.capacity,
            RegionEntry::GuestOnly(r) => r.capacity,
        }
    }

    pub fn as_ref(&self) -> Option<&[u8]> {
        match self {
            RegionEntry::ReadOnly(r) => Some(r.as_ref()),
            RegionEntry::ReadWrite(r) => Some(r.as_ref()),
            _ => None,
        }
    }

    pub fn readable(&self) -> bool {
        match self {
            RegionEntry::ReadOnly(_) | RegionEntry::ReadWrite(_) => true,
            _ => false,
        }
    }

    pub fn writeable(&self) -> bool {
        match self {
            RegionEntry::WriteOnly(_) | RegionEntry::ReadWrite(_) => true,
            _ => false,
        }
    }

    pub fn set_as_guest_memory(&mut self, vm: &VmFd, slot: u32) -> Result<()> {
        match self {
            RegionEntry::ReadOnly(r) => r.set_as_guest_memory(vm, slot),
            RegionEntry::WriteOnly(r) => r.set_as_guest_memory(vm, slot),
            RegionEntry::ReadWrite(r) => r.set_as_guest_memory(vm, slot),
            RegionEntry::GuestOnly(r) => r.set_as_guest_memory(vm, slot),
        }
    }

    pub fn remove_from_guest_memory(&mut self, vm: &VmFd) -> Result<()> {
        match self {
            RegionEntry::ReadOnly(r) => r.remove_from_guest_memory(vm),
            RegionEntry::WriteOnly(r) => r.remove_from_guest_memory(vm),
            RegionEntry::ReadWrite(r) => r.remove_from_guest_memory(vm),
            RegionEntry::GuestOnly(r) => r.remove_from_guest_memory(vm),
        }
    }
}

impl RegionCollection {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn push<P, A>(&mut self, region: Region<P, A>)
    where
        P: Perm,
        A: Align,
        Region<P, A>: Into<RegionEntry>,
    {
        let range = region.addr().as_usize()..(region.addr().as_usize() + region.capacity().get());
        self.inner.push((range, region.into()));
    }

    pub fn get(&self, addr: PhysAddr) -> Option<&RegionEntry> {
        self.inner
            .iter()
            .find(|(range, _)| range.contains(&addr.as_usize()))
            .map(|(_, region)| region)
    }

    pub fn append(&mut self, other: &mut Self) {
        self.inner.append(&mut other.inner);
    }

    pub fn as_vec(&self) -> &Vec<(Range<usize>, RegionEntry)> {
        &self.inner
    }

    pub fn iter(&self) -> RegionCollectionIter<'_> {
        RegionCollectionIter::new(self)
    }

    pub fn iter_mut(&mut self) -> RegionCollectionIterMut<'_> {
        RegionCollectionIterMut::new(self)
    }

    pub fn dump(&self, start: PhysAddr, size: usize, file: &mut File) -> Result<()> {
        let mut current = start;
        let mut requested = size;
        while requested > 0 {
            let reg = self.get(start).ok_or(Error::RegionNotFound(current))?;
            let slice = reg.as_ref().ok_or(Error::NotReadable(current))?;
            let dump = if slice.len() > requested {
                &slice[..requested]
            } else {
                slice
            };

            file.write_all(dump)?;
            requested -= dump.len();
            current += dump.len() as u64;
        }

        Ok(())
    }
}

impl IntoIterator for RegionCollection {
    type Item = RegionEntry;
    type IntoIter = std::iter::Map<
        std::vec::IntoIter<(Range<usize>, RegionEntry)>,
        fn((Range<usize>, RegionEntry)) -> RegionEntry,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter().map(|(_, e)| e)
    }
}

pub struct RegionCollectionIter<'a> {
    inner: slice::Iter<'a, (Range<usize>, RegionEntry)>,
}

impl<'a> RegionCollectionIter<'a> {
    fn new(collection: &'a RegionCollection) -> Self {
        Self {
            inner: collection.inner.iter(),
        }
    }
}

impl<'a> Iterator for RegionCollectionIter<'a> {
    type Item = &'a RegionEntry;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(_, e)| e)
    }
}

pub struct RegionCollectionIterMut<'a> {
    inner: slice::IterMut<'a, (Range<usize>, RegionEntry)>,
}

impl<'a> RegionCollectionIterMut<'a> {
    fn new(collection: &'a mut RegionCollection) -> Self {
        Self {
            inner: collection.inner.iter_mut(),
        }
    }
}

impl<'a> Iterator for RegionCollectionIterMut<'a> {
    type Item = &'a mut RegionEntry;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(_, e)| e)
    }
}

/// This represents a memory region on host, where no guest physical address is set, therefore it
/// can not be mapped into the host. To get a mappable region, set the guest address via `set_guest_addr`.
pub struct ProtoRegion<P: Perm, A: Align = DefaultAlign> {
    capacity: AlignedNonZeroUsize,
    storage: StorageBackend,
    _perm: PhantomData<P>,
    _align: PhantomData<A>,
}

impl<P: Perm, A: Align> ProtoRegion<P, A> {
    /// Set the guest address of the region, converting the ProtoRegion into a fully functional
    /// region, which can be mapped into the guest.
    /// The address is used to set the guest address of the region when it is mapped.
    /// Panics, if the provided address is not aligned.
    pub fn set_guest_addr(self, addr: PhysAddr) -> Region<P, A> {
        if !A::is_aligned(addr.as_u64()) {
            panic!(
                "address {:#x} is not properly aligned for {:#x}",
                addr,
                A::ALIGNMENT
            );
        }
        Region {
            addr,
            capacity: self.capacity,
            storage: self.storage,
            slot: None,
            _perm: PhantomData,
            _align: PhantomData,
        }
    }

    /// Get the region size. This will always be a multiple of Align
    pub fn capacity(&self) -> AlignedNonZeroUsize {
        self.capacity
    }
}

/// This represents a memory region on host, which can be mapped into the physical memory
/// of the guest.
pub struct Region<P: Perm, A: Align = DefaultAlign> {
    addr: PhysAddr,
    capacity: AlignedNonZeroUsize,
    storage: StorageBackend,
    slot: Option<u32>,
    _perm: PhantomData<P>,
    _align: PhantomData<A>,
}

impl<P: Perm, A: Align> Region<P, A> {
    /// Get the guest physical address, where the region is mapped to
    pub fn addr(&self) -> PhysAddr {
        self.addr
    }

    /// Get the region size. This will always be a multiple of Align
    pub fn capacity(&self) -> AlignedNonZeroUsize {
        self.capacity
    }

    /// Set the region as a memory region
    pub fn set_as_guest_memory(&mut self, vm: &VmFd, slot: u32) -> Result<()> {
        unsafe {
            let result = match self.storage {
                StorageBackend::Mmap(mem) => {
                    set_as_guest_memory_mmap(vm, slot, self.capacity, self.addr, mem)
                }
                StorageBackend::GuestMem(fd, mem) => {
                    set_as_guest_memory_memfd(vm, slot, self.capacity, self.addr, fd, mem)
                }
            };

            if result.is_ok() {
                self.slot = Some(slot);
            }

            result
        }
    }

    pub fn remove_from_guest_memory(&mut self, vm: &VmFd) -> Result<()> {
        unsafe {
            if self.slot.is_none() {
                return Ok(());
            }

            let result = match self.storage {
                StorageBackend::Mmap(mem) => remove_from_guest_memory_mmap(
                    vm,
                    self.slot.unwrap(),
                    self.capacity,
                    self.addr,
                    mem,
                ),
                StorageBackend::GuestMem(fd, mem) => {
                    remove_from_guest_memory_memfd(vm, self.slot.unwrap(), self.addr, fd, mem)
                }
            };

            if result.is_ok() {
                self.slot = None;
            }

            result
        }
    }
}

impl<P: Perm, A: Align> Drop for Region<P, A> {
    fn drop(&mut self) {
        match self.storage {
            StorageBackend::GuestMem(fd, mem) => unsafe {
                nix::sys::mman::munmap(mem.cast::<c_void>(), self.capacity.get())
                    .expect("Failed to unmap memory");
                nix::unistd::close(fd).expect("Failed to close guest memory file descriptor");
            },
            StorageBackend::Mmap(ptr) => unsafe {
                nix::sys::mman::munmap(ptr.cast::<c_void>(), self.capacity.get())
                    .expect("Failed to unmap memory");
            },
        }
    }
}

impl Region<GuestOnly> {}

macro_rules! impl_as_ref {
    ($target:ident => $($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> AsRef<[u8]> for $target<$struct, A> {
                fn as_ref(&self) -> &[u8] {
                    match self.storage {
                        StorageBackend::GuestMem(_, _) => {
                            panic!("deref_mut on guest memory");
                        },
                        StorageBackend::Mmap(ptr) => {
                            unsafe { slice::from_raw_parts(ptr.as_ptr(), self.capacity.get()) }
                        }
                    }
                }
            }
        )*
    };
}

macro_rules! impl_read_offset {
    ($target:ident => $($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> $target<$struct, A> {
                /// `read_offset` is like read, but tries to read from a given offset in the page.
                /// An offset of 0 indicates the beginning of the page
                pub fn read_offset(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
                    let capacity = self.capacity.get();
                    if offset > capacity {
                        return Err(Error::InvalidOffset{ offset, max: capacity });
                    }

                    // early exit if buffer length is 0
                    if buf.len() == 0 {
                        return Ok(0);
                    }

                    // Copy data into the memory-mapped region
                    let to_copy = min(capacity - offset, buf.len());
                    buf.copy_from_slice(&self.as_ref()[offset..(offset + to_copy)]);
                    Ok(to_copy)
                }
            }
        )*
    };
}

macro_rules! impl_read_addr {
    ($target:ident => $($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> $target<$struct, A> {
                /// `read_abs` is like `read`, but tries to start reading based on the absolute address.
                /// If the provided address is not included in the region address space, an Error will be returned.
                fn read_addr(&self, addr: u64, buf: &mut [u8]) -> Result<usize> {
                    let capacity = self.capacity.get();
                    if self.addr.as_u64() < addr {
                        return Err(Error::InvalidAddress{ addr, start: self.addr, size: capacity });
                    }

                    if self.addr.as_u64() + (capacity as u64) < addr {
                        return Err(Error::InvalidAddress{ addr, start: self.addr, size: capacity });
                    }

                    let offset = (addr - self.addr.as_u64()) as usize;
                    self.read_offset(offset, buf)
                }
            }
        )*
    };
}

impl_as_ref!(ProtoRegion => ReadOnly, WriteOnly, ReadWrite);
impl_read_offset!(ProtoRegion => ReadOnly, ReadWrite);

impl_as_ref!(Region => ReadOnly, WriteOnly, ReadWrite);
impl_read_offset!(Region => ReadOnly, ReadWrite);
impl_read_addr!(Region => ReadOnly, ReadWrite);

macro_rules! impl_as_mut {
    ($target:ident => $($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> AsMut<[u8]> for $target<$struct, A> {
                fn as_mut(&mut self) -> &mut [u8] {
                    match self.storage {
                        StorageBackend::GuestMem(_, _) => {
                            panic!("deref_mut on guest memory");
                        },
                        StorageBackend::Mmap(mut ptr) => {
                            unsafe { slice::from_raw_parts_mut(ptr.as_mut(), self.capacity.get()) }
                        }
                    }
                }
            }
        )*
    };
}

macro_rules! impl_write_offset {
    ($target:ident => $($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> $target<$struct, A> {
                /// `write_offset` is like `write`, but tries to start writing at the given offset in the page.
                /// An offset of 0 indicates the beginning of the page.
                pub fn write_offset(&mut self, offset: usize, buf: &[u8]) -> Result<usize> {
                    let capacity = self.capacity.get();
                    if offset > capacity {
                        return Err(Error::InvalidOffset{ offset, max: capacity });
                    }

                    // early exit if the buffer is empty
                    if buf.is_empty() {
                        return Ok(0);
                    }

                    // Calculate the amount of data that can be written
                    let fit = min(capacity - offset, buf.len());
                    let data = &buf[..fit];

                    // Copy data into the memory-mapped region
                    self.as_mut()[offset..(offset + data.len())].copy_from_slice(data);
                    Ok(fit)
                }
            }
        )*
    };
}

macro_rules! impl_write_addr {
    ($target:ident => $($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> $target<$struct, A> {
                /// `write_abs` is like `write`, but tries to start writing based on the absolute address.
                /// If the provided address is not included in the region address space, an Error will be returned.
                fn write_addr(&mut self, addr: u64, buf: &[u8]) -> Result<usize> {
                    let capacity = self.capacity.get();
                    if self.addr.as_u64() < addr {
                        return Err(Error::InvalidAddress{
                            addr,
                            start: self.addr,
                            size: capacity
                        });
                    }

                    if self.addr.as_u64() + (capacity as u64) < addr {
                        return Err(Error::InvalidAddress{
                            addr,
                            start: self.addr,
                            size: capacity
                        });
                    }

                    let offset = (addr - self.addr.as_u64()) as usize;
                    self.write_offset(offset, buf)
                }
            }
        )*
    };
}

macro_rules! impl_as_ptr {
    ($target:ident => $($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> $target<$struct, A> {
                /// As ptr returns a pointer to the underlying mmaped memory
                 pub fn as_ptr(&self) -> *const u8 {
                    match self.storage {
                        StorageBackend::GuestMem(_, _) => {
                            panic!("tried to get ptr from guest memory");
                        }
                        StorageBackend::Mmap(ptr) => ptr.as_ptr(),
                    }
                }
            }
        )*
    };
}

macro_rules! impl_as_arena {
    ($target:ident => $($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> $target<$struct, A> {
                 pub fn as_arena(&self) -> Arena {
                    match self.storage {
                        StorageBackend::GuestMem(_, _) => {
                            panic!("tried to get ptr from guest memory");
                        }
                        StorageBackend::Mmap(ptr) => {
                            Arena::new(ptr, self.capacity)
                        },
                    }
                }
            }
        )*
    };
}

impl_as_mut!(ProtoRegion => WriteOnly, ReadWrite);
impl_write_offset!(ProtoRegion => WriteOnly, ReadWrite);
impl_as_arena!(ProtoRegion => WriteOnly, ReadWrite);

impl_as_mut!(Region => WriteOnly, ReadWrite);
impl_write_offset!(Region => WriteOnly, ReadWrite);
impl_write_addr!(Region => WriteOnly, ReadWrite);
impl_as_arena!(Region => WriteOnly, ReadWrite);

impl Into<RegionEntry> for Region<ReadOnly> {
    fn into(self) -> RegionEntry {
        RegionEntry::ReadOnly(self)
    }
}

impl Into<RegionEntry> for Region<WriteOnly> {
    fn into(self) -> RegionEntry {
        RegionEntry::WriteOnly(self)
    }
}

impl Into<RegionEntry> for Region<ReadWrite> {
    fn into(self) -> RegionEntry {
        RegionEntry::ReadWrite(self)
    }
}

impl Into<RegionEntry> for Region<GuestOnly> {
    fn into(self) -> RegionEntry {
        RegionEntry::GuestOnly(self)
    }
}

impl_as_ptr!(ProtoRegion => ReadOnly, WriteOnly, ReadWrite);
impl_as_ptr!(Region => ReadOnly, WriteOnly, ReadWrite);

pub struct Allocator {
    m_flags: MapFlags,
    use_guest_only_fallback: bool,
}

impl Allocator {
    pub fn new(vm: &VmFd) -> Self {
        Self {
            m_flags: MMAP_FLAGS.iter().fold(MapFlags::empty(), |acc, x| acc | *x),
            use_guest_only_fallback: !vm.check_extension(Cap::GuestMemfd),
        }
    }

    pub fn alloc<P: Perm>(
        &self,
        capacity: AlignedNonZeroUsize,
        vm: &VmFd,
    ) -> Result<ProtoRegion<P, DefaultAlign>> {
        self.allocate::<P>(capacity, vm)
    }

    pub fn alloc_accessible<P>(&self, capacity: AlignedNonZeroUsize) -> Result<ProtoRegion<P>>
    where
        P: Perm + Accessible,
    {
        self.mmap::<P>(capacity)
    }

    // FIXME: flags.contains(PROT_NONE) may not properly work
    fn allocate<P: Perm>(
        &self,
        capacity: AlignedNonZeroUsize,
        vm: &VmFd,
    ) -> Result<ProtoRegion<P>> {
        let flags = P::prot_flags();
        if flags.contains(ProtFlags::PROT_NONE) {
            self.guest_memfd(capacity, vm)
        } else {
            self.mmap(capacity)
        }
    }

    fn mmap<P>(&self, capacity: AlignedNonZeroUsize) -> Result<ProtoRegion<P>>
    where
        P: Perm,
    {
        let flags = P::prot_flags();
        // mmap a region with the required size and flags
        let mem = unsafe { mmap_anonymous(None, capacity.get_non_zero(), flags, self.m_flags) }?;

        let region = ProtoRegion {
            capacity,
            storage: StorageBackend::Mmap(mem.cast::<u8>()),
            _perm: std::marker::PhantomData,
            _align: std::marker::PhantomData,
        };

        // self.regions.add(region);

        Ok(region)
    }

    fn guest_memfd<P: Perm>(
        &self,
        capacity: AlignedNonZeroUsize,
        vm: &VmFd,
    ) -> Result<ProtoRegion<P>> {
        let gmem = kvm_create_guest_memfd {
            size: capacity.get() as u64,
            flags: 0,
            reserved: [0; 6],
        };

        let fd = vm.create_guest_memfd(gmem)?;

        let pflags = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
        let mem = unsafe { mmap_anonymous(None, capacity.get_non_zero(), pflags, self.m_flags)? };

        let region = ProtoRegion {
            capacity,
            storage: StorageBackend::GuestMem(fd, mem.cast::<u8>()),
            _perm: PhantomData,
            _align: PhantomData,
        };

        Ok(region)
    }

    /// wrap the P::prot_flags to include the guest only fallback flag
    /// if the Perm is not accessible
    fn perm_to_flags<P: Perm>(&self) -> ProtFlags {
        let flags = P::prot_flags();
        if self.use_guest_only_fallback && flags.contains(ProtFlags::PROT_NONE) {
            return ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
        }

        flags
    }
}

unsafe fn set_as_guest_memory_memfd(
    vm: &VmFd,
    slot: u32,
    capacity: AlignedNonZeroUsize,
    addr: PhysAddr,
    fd: RawFd,
    mem: NonNull<u8>,
) -> Result<()> {
    let mapping = kvm_userspace_memory_region2 {
        slot,
        flags: 0,
        guest_phys_addr: addr.as_u64(),
        memory_size: capacity.get() as u64,
        userspace_addr: mem.as_ptr() as u64,
        guest_memfd_offset: 0,
        guest_memfd: fd as u32,
        pad1: 0,
        pad2: [0; 14],
    };

    unsafe {
        vm.set_user_memory_region2(mapping)
            .map_err(|e| Error::RegionMappingFailed(addr, e))
    }
}

unsafe fn set_as_guest_memory_mmap(
    vm: &VmFd,
    slot: u32,
    capacity: AlignedNonZeroUsize,
    addr: PhysAddr,
    mem: NonNull<u8>,
) -> Result<()> {
    let mapping = kvm_userspace_memory_region {
        slot,
        flags: 0,
        guest_phys_addr: addr.as_u64(),
        memory_size: capacity.get() as u64,
        userspace_addr: mem.as_ptr() as u64,
    };

    unsafe {
        vm.set_user_memory_region(mapping)
            .map_err(|e| Error::RegionMappingFailed(addr, e))
    }
}

unsafe fn remove_from_guest_memory_memfd(
    vm: &VmFd,
    slot: u32,
    addr: PhysAddr,
    fd: RawFd,
    mem: NonNull<u8>,
) -> Result<()> {
    let mapping = kvm_userspace_memory_region2 {
        slot,
        flags: 0,
        guest_phys_addr: addr.as_u64(),
        memory_size: 0u64,
        userspace_addr: mem.as_ptr() as u64,
        guest_memfd_offset: 0,
        guest_memfd: fd as u32,
        pad1: 0,
        pad2: [0; 14],
    };

    unsafe {
        vm.set_user_memory_region2(mapping)
            .map_err(|e| Error::RegionUnmappingFailed(addr, e))
    }
}

unsafe fn remove_from_guest_memory_mmap(
    vm: &VmFd,
    slot: u32,
    capacity: AlignedNonZeroUsize,
    addr: PhysAddr,
    mem: NonNull<u8>,
) -> Result<()> {
    let mapping = kvm_userspace_memory_region {
        slot,
        flags: 0,
        guest_phys_addr: addr.as_u64(),
        memory_size: capacity.get() as u64,
        userspace_addr: mem.as_ptr() as u64,
    };

    unsafe {
        vm.set_user_memory_region(mapping)
            .map_err(|e| Error::RegionUnmappingFailed(addr, e))
    }
}
