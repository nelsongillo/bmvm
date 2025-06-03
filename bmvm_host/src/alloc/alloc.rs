use crate::alloc::{Anon, Perm, Readable, Writable};
use bmvm_common::mem::{Align, DefaultAlign, PhysAddr};
use std::ops::{Deref, DerefMut};
use std::panic;
use std::slice;

pub enum Error {
    /// Allocation of a new region failed due to the host being out of memory.
    OutOfMemory,
    /// The provided offset is not included in the region address space.
    /// Provided Offset, Max Offset
    InvalidOffset(usize, usize),
    /// The provided address is not included in the region address space.
    /// Provided Address, Starting Address, Size
    InvalidAddress(u64, PhysAddr, usize),
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::OutOfMemory => std::write!(f, "out of memory"),
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

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::OutOfMemory => std::write!(f, "out of memory"),
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

/// This represents a memory region on host, which can be mapped into the physical memory
/// of the guest.
pub struct Region<P: Perm, A: Align = DefaultAlign> {
    physical_addr: PhysAddr,
    size: usize,
    ptr: *mut u8,
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
        self.physical_addr = addr;
    }

    /// Get the guest physical address, where the region is mapped to
    pub fn guest_addr(&self) -> PhysAddr {
        self.physical_addr
    }

    /// Get the region size. This will always be a multiple of Align
    pub fn size(&self) -> usize {
        self.size
    }
}

impl<P: Readable> Region<P> {
    /// `read_offset` is like read, but tries to read from a given offset in the page.
    /// An offset of 0 indicates the beginning of the page
    pub fn read_offset(&self, offset: usize, buf: &mut [u8]) -> Result<usize, Error> {
        if offset > self.size {
            return Err(Error::InvalidOffset(offset, self.size));
        }

        // early exit if buffer length is 0
        if buf.len() == 0 {
            return Ok(0);
        }

        // Copy data into the memory-mapped region
        let to_copy = if buf.len() > self.size - offset {
            self.size - offset
        } else {
            buf.len()
        };
        buf.copy_from_slice(&self.deref()[offset..(offset + to_copy)]);
        Ok(to_copy)
    }

    /// `read_abs` is like `read`, but tries to start reading based on the absolute address.
    /// If the provided address is not included in the region address space, an Error will be returned.
    fn read_addr(&self, addr: u64, buf: &mut [u8]) -> Result<usize, Error> {
        if self.physical_addr.as_u64() < addr {
            return Err(Error::InvalidAddress(addr, self.physical_addr, self.size));
        }

        if self.physical_addr.as_u64() + (self.size as u64) < addr {
            return Err(Error::InvalidAddress(addr, self.physical_addr, self.size));
        }

        let offset = (addr - self.physical_addr.as_u64()) as usize;
        self.read_offset(offset, buf)
    }
}

impl<P: Writable> Region<P> {
    /// `write_offset` is like `write`, but tries to start writing at the given offset in the page.
    /// An offset of 0 indicates the beginning of the page.
    pub fn write_offset(&mut self, offset: usize, buf: &[u8]) -> Result<usize, Error> {
        if offset > self.size {
            return Err(Error::InvalidOffset(offset, self.size));
        }

        // early exit if the buffer is empty
        if buf.is_empty() {
            return Ok(0);
        }

        // Calculate the amount of data that can be written
        let fit = self.size - offset;
        let data = &buf[..fit];

        // Copy data into the memory-mapped region
        self.deref_mut()[offset..(offset + data.len())].copy_from_slice(data);
        Ok(fit)
    }

    /// `write_abs` is like `write`, but tries to start writing based on the absolute address.
    /// If the provided address is not included in the region address space, an Error will be returned.
    fn write_addr(&mut self, addr: u64, buf: &[u8]) -> Result<usize, Error> {
        if self.physical_addr.as_u64() < addr {
            return Err(Error::InvalidAddress(addr, self.physical_addr, self.size));
        }

        if self.physical_addr.as_u64() + (self.size as u64) < addr {
            return Err(Error::InvalidAddress(addr, self.physical_addr, self.size));
        }

        let offset = (addr - self.physical_addr.as_u64()) as usize;
        self.write_offset(offset, buf)
    }
}

impl<P: Anon> Region<P> {}

impl<P: Perm, A: Align> Deref for Region<P, A> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr, self.size) }
    }
}

impl<P: Perm, A: Align> DerefMut for Region<P, A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.size) }
    }
}

impl<P: Perm, A: Align> AsRef<[u8]> for Region<P, A> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}

impl<P: Perm, A: Align> AsMut<[u8]> for Region<P, A> {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.deref_mut()
    }
}

pub trait Manager {
    fn allocate_alignment<P: Perm, A: Align>(&self, size: u64) -> Result<Region<P, A>, Error>;
    fn allocate<P: Perm>(&self, size: u64) -> Result<Region<P, DefaultAlign>, Error> {
        self.allocate_alignment::<P, DefaultAlign>(size)
    }
}
