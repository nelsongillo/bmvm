use crate::alloc::{Accessible, GuestOnly, Perm, ReadOnly, ReadWrite, WriteOnly};
use bmvm_common::mem::{Align, AlignedNonZeroUsize, DefaultAlign, PhysAddr};
use core::ffi::c_void;
use kvm_bindings::kvm_create_guest_memfd;
use kvm_ioctls::{Cap, VmFd};
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous};
use std::cmp::min;
use std::os::fd::RawFd;
use std::panic;
use std::ptr::NonNull;
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

#[derive(Debug, PartialEq, Eq)]
enum Storage {}
enum StorageBackend {
    Mmap(NonNull<u8>),
    GuestMem(RawFd),
}

/// This represents a memory region on host, where no guest physical address is set, therefore it
/// can not be mapped into the host. To get a mappable region, set the guest address via `set_guest_addr`.
pub struct ProtoRegion<P: Perm, A: Align = DefaultAlign> {
    capacity: usize,
    storage: StorageBackend,
    _perm: std::marker::PhantomData<P>,
    _align: std::marker::PhantomData<A>,
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
            _perm: std::marker::PhantomData,
            _align: std::marker::PhantomData,
        }
    }
}

/// This represents a memory region on host, which can be mapped into the physical memory
/// of the guest.
pub struct Region<P: Perm, A: Align = DefaultAlign> {
    addr: PhysAddr,
    capacity: usize,
    storage: StorageBackend,
    _perm: std::marker::PhantomData<P>,
    _align: std::marker::PhantomData<A>,
}

impl<P: Perm, A: Align> Region<P, A> {
    /// Get the guest physical address, where the region is mapped to
    pub fn guest_addr(&self) -> PhysAddr {
        self.addr
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

macro_rules! impl_as_ref {
    ($target:ident => $($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> AsRef<[u8]> for $target<$struct, A> {
                fn as_ref(&self) -> &[u8] {
                    match self.storage {
                        StorageBackend::GuestMem(_) => {
                            panic!("deref_mut on guest memory");
                        },
                        StorageBackend::Mmap(ptr) => {
                            unsafe { slice::from_raw_parts(ptr.as_ptr(), self.capacity) }
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
                fn read_addr(&self, addr: u64, buf: &mut [u8]) -> Result<usize, Error> {
                    if self.addr.as_u64() < addr {
                        return Err(Error::InvalidAddress{ addr: addr, start: self.addr, size: self.capacity });
                    }

                    if self.addr.as_u64() + (self.capacity as u64) < addr {
                        return Err(Error::InvalidAddress{ addr: addr, start: self.addr, size: self.capacity });
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
                        StorageBackend::GuestMem(_) => {
                            panic!("deref_mut on guest memory");
                        },
                        StorageBackend::Mmap(mut ptr) => {
                            unsafe { slice::from_raw_parts_mut(ptr.as_mut(), self.capacity) }
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
                fn write_addr(&mut self, addr: u64, buf: &[u8]) -> Result<usize, Error> {
                    if self.addr.as_u64() < addr {
                        return Err(Error::InvalidAddress{
                            addr: addr,
                            start: self.addr,
                            size: self.capacity
                        });
                    }

                    if self.addr.as_u64() + (self.capacity as u64) < addr {
                        return Err(Error::InvalidAddress{
                            addr: addr,
                            start: self.addr,
                            size: self.capacity
                        });
                    }

                    let offset = (addr - self.addr.as_u64()) as usize;
                    self.write_offset(offset, buf)
                }
            }
        )*
    };
}

impl_as_mut!(ProtoRegion => WriteOnly, ReadWrite);
impl_write_offset!(ProtoRegion => WriteOnly, ReadWrite);

impl_as_mut!(Region => WriteOnly, ReadWrite);
impl_write_offset!(Region => WriteOnly, ReadWrite);
impl_write_addr!(Region => WriteOnly, ReadWrite);

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
    ) -> Result<ProtoRegion<P, DefaultAlign>, Error> {
        self.allocate::<P>(capacity, vm)
    }

    pub fn alloc_accessible<P>(
        &self,
        capacity: AlignedNonZeroUsize,
    ) -> Result<ProtoRegion<P>, Error>
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
    ) -> Result<ProtoRegion<P>, Error> {
        let flags = P::prot_flags();
        if flags.contains(ProtFlags::PROT_NONE) {
            self.guest_memfd(capacity, vm)
        } else {
            self.mmap(capacity)
        }
    }

    fn mmap<P>(&self, capacity: AlignedNonZeroUsize) -> Result<ProtoRegion<P>, Error>
    where
        P: Perm,
    {
        let flags = P::prot_flags();
        // mmap a region with the required size and flags
        let mem = unsafe { mmap_anonymous(None, capacity.get_non_zero(), flags, self.m_flags) }?;

        let region = ProtoRegion {
            capacity: capacity.get(),
            storage: StorageBackend::Mmap(mem.cast::<u8>()),
            _perm: std::marker::PhantomData,
            _align: std::marker::PhantomData,
        };

        // self.regions.add(region);

        Ok(region)
    }

    // TODO: implement me
    fn guest_memfd<P: Perm>(
        &self,
        capacity: AlignedNonZeroUsize,
        vm: &VmFd,
    ) -> Result<ProtoRegion<P>, Error> {
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
