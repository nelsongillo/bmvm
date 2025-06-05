use crate::alloc::{GuestOnly, Perm, ReadOnly, ReadWrite, WriteOnly};
use bmvm_common::mem::{Align, DefaultAlign, PhysAddr};
use core::ffi::c_void;
use kvm_bindings::kvm_create_guest_memfd;
use kvm_ioctls::{Cap, VmFd};
use nix::errno;
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous};
use std::num::NonZeroUsize;
use std::ops::{Deref, DerefMut};
use std::os::fd::RawFd;
use std::panic;
use std::ptr::NonNull;
use std::slice;

const MMAP_FLAGS: MapFlags = MapFlags::MAP_ANONYMOUS;

#[derive(Debug)]
pub enum Error {
    /// Allocation of a new region failed due to the host error.
    Errno(errno::Errno),
    /// The provided offset is not included in the region address space.
    /// Provided Offset, Max Offset
    InvalidOffset(usize, usize),
    /// The provided address is not included in the region address space.
    /// Provided Address, Starting Address, Size
    InvalidAddress(u64, PhysAddr, usize),
    GuestAddressNotSet,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::GuestAddressNotSet => std::write!(f, "guest address not set"),
            Error::Errno(errno) => std::write!(f, "errno: {}", errno),
            Error::InvalidOffset(offset, max) => {
                std::write!(f, "invalid offset: {} (max: {})", offset, max)
            }
            Error::InvalidAddress(addr, start, size) => std::write!(
                f,
                "invalid address: {:#x} (start: {:#x}, size: {})",
                addr,
                start,
                size
            ),
        }
    }
}

impl std::error::Error for Error {}

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

macro_rules! impl_ref_traits_for_region {
    ($($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> Deref for Region<$struct, A> {
                type Target = [u8];

                #[inline]
                fn deref(&self) -> &[u8] {
                    match self.storage {
                        StorageBackend::GuestMem(_) => {
                            panic!("deref on guest memory");
                        },
                        StorageBackend::Mmap(ptr) if ptr.as_ptr().is_null() => {
                            panic!("deref on null pointer");
                        }
                        StorageBackend::Mmap(ptr) => {
                            unsafe { slice::from_raw_parts(ptr.as_ptr(), self.capacity) }
                        }
                    }
                }
            }

            impl<A: Align> AsRef<[u8]> for Region<$struct, A> {
                #[inline]
                fn as_ref(&self) -> &[u8] {
                    self.deref()
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
                        return Err(Error::InvalidOffset(offset, self.capacity));
                    }

                    // early exit if buffer length is 0
                    if buf.len() == 0 {
                        return Ok(0);
                    }

                    // Copy data into the memory-mapped region
                    let to_copy = if buf.len() > self.capacity - offset {
                        self.capacity - offset
                    } else {
                        buf.len()
                    };
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
                        return Err(Error::InvalidAddress(addr, guest, self.capacity));
                    }

                    if guest.as_u64() + (self.capacity as u64) < addr {
                        return Err(Error::InvalidAddress(addr, guest, self.capacity));
                    }

                    let offset = (addr - guest.as_u64()) as usize;
                    self.read_offset(offset, buf)
                }
            }
        )*
    };
}

impl_ref_traits_for_region!(ReadOnly, WriteOnly, ReadWrite);
impl_read_for_region!(ReadOnly, ReadWrite);

macro_rules! impl_ref_mut_traits_for_structs {
    ($($struct:ty),* $(,)?) => {
        $(
            impl<A: Align> DerefMut for Region<$struct, A> {
                #[inline]
                fn deref_mut(&mut self) -> &mut [u8] {
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

            impl<A: Align> AsMut<[u8]> for Region<$struct, A> {
                #[inline]
                fn as_mut(&mut self) -> &mut [u8] {
                    self.deref_mut()
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
                        return Err(Error::InvalidOffset(offset, self.capacity));
                    }

                    // early exit if the buffer is empty
                    if buf.is_empty() {
                        return Ok(0);
                    }

                    // Calculate the amount of data that can be written
                    let fit = self.capacity - offset;
                    let data = &buf[..fit];

                    // Copy data into the memory-mapped region
                    self.deref_mut()[offset..(offset + data.len())].copy_from_slice(data);
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
                        return Err(Error::InvalidAddress(addr, guest, self.capacity));
                    }

                    if guest.as_u64() + (self.capacity as u64) < addr {
                        return Err(Error::InvalidAddress(addr, guest, self.capacity));
                    }

                    let offset = (addr - guest.as_u64()) as usize;
                    self.write_offset(offset, buf)
                }
            }
        )*
    };
}

impl_ref_mut_traits_for_structs!(WriteOnly, ReadWrite);
impl_write_for_region!(WriteOnly, ReadWrite);

pub struct Manager<'a> {
    vm: &'a VmFd,
    use_guest_only_fallback: bool,
}

impl<'a> Manager<'a> {
    pub fn new(vm: &'a VmFd) -> Self {
        Self {
            vm,
            use_guest_only_fallback: !vm.check_extension(Cap::GuestMemfd),
        }
    }

    pub fn allocate_alignment<P: Perm, A: Align>(
        &self,
        capacity: NonZeroUsize,
    ) -> Result<Region<P, A>, Error> {
        let flags = P::prot_flags();
        if flags.contains(ProtFlags::PROT_NONE) {
            return self.alloc_guest_memfd(capacity);
        }

        // mmap a region with the required size and flags
        let mem = unsafe { mmap_anonymous(None, capacity, flags, MMAP_FLAGS) }
            .map_err(|errno| Error::Errno(errno))?;

        let region = Region {
            physical_addr: None,
            capacity: capacity.get(),
            storage: StorageBackend::Mmap(mem.cast::<u8>()),
            _perm: std::marker::PhantomData,
            _align: std::marker::PhantomData,
        };

        Ok(region)
    }

    pub fn allocate<P: Perm>(
        &self,
        capacity: NonZeroUsize,
    ) -> Result<Region<P, DefaultAlign>, Error> {
        self.allocate_alignment::<P, DefaultAlign>(capacity)
    }

    fn perm_to_flags<P: Perm>(&self) -> ProtFlags {
        let flags = P::prot_flags();
        if self.use_guest_only_fallback && flags.contains(ProtFlags::PROT_NONE) {
            return ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
        }

        flags
    }

    // TODO: implement me
    fn alloc_guest_memfd<P: Perm, A: Align>(
        &self,
        capacity: NonZeroUsize,
    ) -> Result<Region<P, A>, Error> {
        let gmem = kvm_create_guest_memfd {
            size: capacity.get() as u64,
            flags: 0,
            reserved: [0; 6],
        };

        let fd = self
            .vm
            .create_guest_memfd(gmem)
            .map_err(|_| Error::Errno(errno::Errno::EIO))?;

        unimplemented!()
    }
}
