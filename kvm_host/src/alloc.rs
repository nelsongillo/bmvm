use std::ops::{Deref, DerefMut};
use std::slice;

pub enum Error {
    OutOfMemory,
    /// The provided offset is not included in the region address space.
    /// Provided Offset, Max Offset
    InvalidOffset(usize, usize),
    /// The provided address is not included in the region address space.
    /// Provided Address, Starting Address, Size
    InvalidAddress(PhyAddr, PhyAddr, usize)
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::OutOfMemory => write!(f, "out of memory"),
            Error::InvalidOffset(offset, max) => write!(f, "invalid offset: {} (max: {})", offset, max),
            Error::InvalidAddress(addr, start, size) => write!(f, "invalid address: {:#x} (start: {:#x}, size: {})", addr, start, size),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::OutOfMemory => write!(f, "out of memory"),
            Error::InvalidOffset(offset, max) => write!(f, "invalid offset: {} (max: {})", offset, max),
            Error::InvalidAddress(addr, start, size) => write!(f, "invalid address: {:#x} (start: {:#x}, size: {})", addr, start, size),
        }
    }
}

impl std::error::Error for Error {}

/// Trait to abstract over different page sizes based on the underlying architecture.
pub trait Align: Copy + Eq + PartialEq + PartialOrd + Ord {
    /// The page size in bytes.
    const ALIGNMENT: u64;

    fn is_aligned(addr: u64) -> bool {
        addr % Self::ALIGNMENT == 0
    }

    /// align an address to the beginning of the page
    fn align_floor(addr: u64) -> u64 {
        addr & !(Self::ALIGNMENT - 1)
    }

    /// align an address to the beginning of the next page
    fn align_ceil(addr: u64) -> u64 {
        (addr + Self::ALIGNMENT - 1) & !(Self::ALIGNMENT - 1)
    }

}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub type DefaultAlign = X86_64;

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub type DefaultAlign = Arm64;

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct X86_64;


impl Align for X86_64 {
    const ALIGNMENT: u64 = 0x1000;
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Arm64;


impl Align for Arm64 {
    const ALIGNMENT: u64 = 0x1000;
}


type PhyAddr = u64;

pub trait Writable {}

pub trait Readable {}

pub struct ReadOnly;
pub struct WriteOnly;
pub struct ReadWrite;


pub struct Region<P, A: Align = DefaultAlign> {
    physical_addr: PhyAddr,
    size: usize,
    ptr: *mut u8,
    _perm: std::marker::PhantomData<P>,
    _align: std::marker::PhantomData<A>,
}

impl<P, A: Align> Region<P, A> {
    pub fn new() -> Self {
        Self {
            physical_addr: 0,
            size: 0,
            ptr: std::ptr::null_mut(),
            _perm: std::marker::PhantomData,
            _align: std::marker::PhantomData,
        }
    }

    /// Set the guest address of the region.
    /// This is used to set the guest address of the region when it is mapped.
    /// This is not used for the initial allocation of the region.
    /// Panics, if the provided address is not aligned.
    pub fn set_guest_addr(&mut self, addr: PhyAddr) {
        if !A::is_aligned(addr) {
            panic!("address {:#x} is not properly aligned for {:#x}", addr, A::ALIGNMENT);
        }
        self.physical_addr = addr;
    }
    fn guest_addr(&self) -> PhyAddr {
        self.physical_addr
    }
    fn size(&self) -> usize {
        self.size
    }
}

impl<P: Readable> Region<P> {
    /// `read_offset` is like read, but tries to read from a given offset in the page.
    /// An offset of 0 indicates the beginning of the page
    pub fn read_offset(&mut self, offset: usize, buf: &[u8]) -> std::io::Result<usize> {
        // TODO: implement
        panic!("not implemented");
    }

    /// `read_abs` is like `read`, but tries to start reading based on the absolute address.
    /// If the provided address is not included in the region address space, an Error will be returned.
    fn read_addr(&mut self, addr: u64, buf: &[u8]) -> std::io::Result<usize> {
        // TODO: implement
        panic!("not implemented");
    }
}

impl<P: Writable> Region<P> {
    /// `write_offset` is like `write`, but tries to start writing at the given offset in the page.
    /// An offset of 0 indicates the beginning of the page.
    pub fn write_offset(&mut self, offset: usize, buf: &[u8]) -> Result<usize, Error>{
        if offset > self.size {
            return Err(Error::InvalidOffset(offset, self.size))
        }

        // early exit if the buffer is empty
        if buf.is_empty() {
            return Ok(0)
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
        if self.physical_addr < addr {
            return Err(Error::InvalidAddress(addr, self.physical_addr, self.size));
        }


        if self.physical_addr + (self.size as u64) < addr {
            return Err(Error::InvalidAddress(addr, self.physical_addr, self.size));
        }

        let offset = (addr - self.physical_addr) as usize;
        self.write_offset(offset, buf)
    }
}


impl<P, A: Align> Deref for Region<P, A> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr, self.size) }
    }
}

impl<P, A: Align> DerefMut for Region<P, A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.size) }
    }
}

impl<P, A: Align> AsRef<[u8]> for Region<P, A> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}

impl<P, A: Align> AsMut<[u8]> for Region<P, A> {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.deref_mut()
    }
}

pub trait Manager {
    fn allocate<P, A: Align>(&self, size: u64) -> Result<Region<P, A>, Error>;
}
