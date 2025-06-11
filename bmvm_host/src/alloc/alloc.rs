use crate::alloc::{Accessible, GuestOnly, Perm, ReadOnly, ReadWrite, WriteOnly};
use bmvm_common::mem::{Align, DefaultAlign, PhysAddr};
use core::ffi::c_void;
use kvm_bindings::kvm_create_guest_memfd;
use kvm_ioctls::{Cap, VmFd};
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous};
use std::cmp::min;
use std::num::NonZeroUsize;
use std::os::fd::RawFd;
use std::panic;
use std::ptr::NonNull;
use std::rc::Rc;
use std::slice;

const MMAP_FLAGS: [MapFlags; 2] = [MapFlags::MAP_PRIVATE, MapFlags::MAP_ANONYMOUS];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("kvm errno: {0}")]
    KvmErrno(#[from] kvm_ioctls::Error),

    #[error("nix errno: {0}")]
    NixErrno(#[from] nix::errno::Errno),

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

    #[error("no guest address set")]
    GuestAddressNotSet,
}

pub struct RegionCollection {
    regions: Vec<RegionEntry>,
}

pub enum RegionEntry {
    ReadOnly(Rc<Region<ReadOnly>>),
    WriteOnly(Rc<Region<WriteOnly>>),
    ReadWrite(Rc<Region<ReadWrite>>),
    GuestOnly(Rc<Region<GuestOnly>>),
}

impl RegionCollection {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            regions: Vec::with_capacity(capacity),
        }
    }

    pub fn add<R>(&mut self, entry: R)
    where
        R: Into<RegionEntry>,
    {
        self.regions.push(entry.into());
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Storage {}
enum StorageBackend {
    Mmap(NonNull<u8>),
    GuestMem(RawFd),
}

/// This represents a memory region on host, which can be mapped into the physical memory
/// of the guest.
pub struct Region<P: Perm, A: Align = DefaultAlign> {
    physical_addr: Option<PhysAddr>,
    capacity: usize,
    storage: StorageBackend,
    _perm: std::marker::PhantomData<P>,
    _align: std::marker::PhantomData<A>,
}

impl<P: Perm, A: Align> Region<P, A> {
    /// Set the guest address of the region.
    /// This is used to set the guest address of the region when it is mapped.
    /// This is not used for the initial allocation of the region.
    /// Panics, if the provided address is not aligned.
    pub fn set_guest_addr(&mut self, addr: PhysAddr) {
        if !A::is_aligned(addr.as_u64()) {
            panic!(
                "address {:#x} is not properly aligned for {:#x}",
                addr,
                A::ALIGNMENT
            );
        }
        self.physical_addr = Some(addr);
    }

    /// Get the guest physical address, where the region is mapped to
    pub fn guest_addr(&self) -> Option<PhysAddr> {
        self.physical_addr
    }

    /// Get the region size. This will always be a multiple of Align
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn as_ptr(&self) -> *const u8 {
        match self.storage {
            StorageBackend::GuestMem(_) => {
                panic!("tried to get ptr from guest memory");
            }
            StorageBackend::Mmap(ptr) => ptr.as_ptr(),
        }
    }
}

impl<P: Perm, A: Align> Drop for Region<P, A> {
    fn drop(&mut self) {
        match self.storage {
            StorageBackend::GuestMem(fd) => {
                nix::unistd::close(fd).expect("Failed to close guest memory file descriptor");
            }
            StorageBackend::Mmap(ptr) => unsafe {
                nix::sys::mman::munmap(ptr.cast::<c_void>(), self.capacity)
                    .expect("Failed to unmap memory");
            },
        }
    }
}

impl Region<GuestOnly> {}

/*
impl Into<RegionEntry> for Rc<Region<ReadOnly>> {
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

*/
macro_rules! impl_as_ref_for_region {
    ($($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> AsRef<[u8]> for Region<$struct, A> {
                fn as_ref(&self) -> &[u8] {
                    match self.storage {
                        StorageBackend::GuestMem(_) => {
                            panic!("deref_mut on guest memory");
                        },
                        StorageBackend::Mmap(ptr) if ptr.as_ptr().is_null() => {
                            panic!("deref_mut on null pointer");
                        }
                        StorageBackend::Mmap(ptr) => {
                            unsafe { slice::from_raw_parts(ptr.as_ptr(), self.capacity) }
                        }
                    }
                }
            }
        )*
    };
}

macro_rules! impl_read_for_region {
    ($($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> Region<$struct, A> {
                /// `read_offset` is like read, but tries to read from a given offset in the page.
                /// An offset of 0 indicates the beginning of the page
                pub fn read_offset(&self, offset: usize, buf: &mut [u8]) -> Result<usize, Error> {
                    if offset > self.capacity {
                        return Err(Error::InvalidOffset{ offset: offset, max: self.capacity });
                    }

                    // early exit if buffer length is 0
                    if buf.len() == 0 {
                        return Ok(0);
                    }

                    // Copy data into the memory-mapped region
                    let to_copy = min(self.capacity - offset, buf.len());
                    buf.copy_from_slice(&self.as_ref()[offset..(offset + to_copy)]);
                    Ok(to_copy)
                }

                /// `read_abs` is like `read`, but tries to start reading based on the absolute address.
                /// If the provided address is not included in the region address space, an Error will be returned.
                fn read_addr(&self, addr: u64, buf: &mut [u8]) -> Result<usize, Error> {
                    if self.physical_addr.is_none() {
                        return Err(Error::GuestAddressNotSet);
                    }

                    let guest = self.physical_addr.unwrap();
                    if guest.as_u64() < addr {
                        return Err(Error::InvalidAddress{ addr: addr, start: guest, size: self.capacity });
                    }

                    if guest.as_u64() + (self.capacity as u64) < addr {
                        return Err(Error::InvalidAddress{ addr: addr, start: guest, size: self.capacity });
                    }

                    let offset = (addr - guest.as_u64()) as usize;
                    self.read_offset(offset, buf)
                }
            }
        )*
    };
}

impl_as_ref_for_region!(ReadOnly, WriteOnly, ReadWrite);
impl_read_for_region!(ReadOnly, ReadWrite);

macro_rules! impl_as_mut_for_region {
    ($($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> AsMut<[u8]> for Region<$struct, A> {
                fn as_mut(&mut self) -> &mut [u8] {
                    match self.storage {
                        StorageBackend::GuestMem(_) => {
                            panic!("deref_mut on guest memory");
                        },
                        StorageBackend::Mmap(ptr) if ptr.as_ptr().is_null() => {
                            panic!("deref_mut on null pointer");
                        }
                        StorageBackend::Mmap(mut ptr) => {
                            unsafe { slice::from_raw_parts_mut(ptr.as_mut(), self.capacity) }
                        }
                    }
                }
            }
        )*
    };
}

macro_rules! impl_write_for_region {
    ($($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> Region<$struct, A> {
                /// `write_offset` is like `write`, but tries to start writing at the given offset in the page.
                /// An offset of 0 indicates the beginning of the page.
                pub fn write_offset(&mut self, offset: usize, buf: &[u8]) -> Result<usize, Error> {
                    if offset > self.capacity {
                        return Err(Error::InvalidOffset{ offset: offset, max: self.capacity });
                    }

                    // early exit if the buffer is empty
                    if buf.is_empty() {
                        return Ok(0);
                    }

                    // Calculate the amount of data that can be written
                    let fit = min(self.capacity - offset, buf.len());
                    let data = &buf[..fit];

                    // Copy data into the memory-mapped region
                    self.as_mut()[offset..(offset + data.len())].copy_from_slice(data);
                    Ok(fit)
                }

                /// `write_abs` is like `write`, but tries to start writing based on the absolute address.
                /// If the provided address is not included in the region address space, an Error will be returned.
                fn write_addr(&mut self, addr: u64, buf: &[u8]) -> Result<usize, Error> {
                    if self.physical_addr.is_none() {
                        return Err(Error::GuestAddressNotSet);
                    }

                    let guest = self.physical_addr.unwrap();
                    if guest.as_u64() < addr {
                        return Err(Error::InvalidAddress{
                            addr: addr,
                            start: guest,
                            size: self.capacity
                        });
                    }

                    if guest.as_u64() + (self.capacity as u64) < addr {
                        return Err(Error::InvalidAddress{
                            addr: addr,
                            start: guest,
                            size: self.capacity
                        });
                    }

                    let offset = (addr - guest.as_u64()) as usize;
                    self.write_offset(offset, buf)
                }
            }
        )*
    };
}

impl_as_mut_for_region!(WriteOnly, ReadWrite);
impl_write_for_region!(WriteOnly, ReadWrite);

pub struct Allocator {
    m_flags: MapFlags,
    use_guest_only_fallback: bool,
    regions: RegionCollection,
}

impl Allocator {
    pub fn new(vm: &VmFd) -> Self {
        Self {
            m_flags: MMAP_FLAGS.iter().fold(MapFlags::empty(), |acc, x| acc | *x),
            use_guest_only_fallback: !vm.check_extension(Cap::GuestMemfd),
            regions: RegionCollection::new(),
        }
    }

    pub fn alloc<P: Perm>(
        &self,
        capacity: NonZeroUsize,
        vm: &VmFd,
    ) -> Result<Region<P, DefaultAlign>, Error> {
        self.allocate::<P>(capacity, vm)
    }

    pub fn alloc_accessible<P>(&self, capacity: NonZeroUsize) -> Result<Region<P>, Error>
    where
        P: Perm + Accessible,
    {
        self.mmap::<P>(capacity)
    }

    fn allocate<P: Perm>(&self, capacity: NonZeroUsize, vm: &VmFd) -> Result<Region<P>, Error> {
        let aligned_cap = DefaultAlign::align_ceil(capacity.get() as u64);
        let cap = NonZeroUsize::new(aligned_cap as usize).unwrap();

        let flags = P::prot_flags();
        if flags.contains(ProtFlags::PROT_NONE) {
            self.guest_memfd(cap, vm)
        } else {
            self.mmap(cap)
        }
    }

    fn mmap<P>(&self, capacity: NonZeroUsize) -> Result<Region<P>, Error>
    where
        P: Perm,
    {
        let flags = P::prot_flags();
        // mmap a region with the required size and flags
        let mem = unsafe { mmap_anonymous(None, capacity, flags, self.m_flags) }?;

        let region = Region {
            physical_addr: None,
            capacity: capacity.get(),
            storage: StorageBackend::Mmap(mem.cast::<u8>()),
            _perm: std::marker::PhantomData,
            _align: std::marker::PhantomData,
        };

        // self.regions.add(region);

        Ok(region)
    }

    // TODO: implement me
    fn guest_memfd<P: Perm>(&self, capacity: NonZeroUsize, vm: &VmFd) -> Result<Region<P>, Error> {
        let gmem = kvm_create_guest_memfd {
            size: capacity.get() as u64,
            flags: 0,
            reserved: [0; 6],
        };

        let fd = vm.create_guest_memfd(gmem)?;

        unimplemented!()
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
