use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};

pub enum Error {
    OutOfMemory,
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::OutOfMemory => write!(f, "out of memory"),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::OutOfMemory => write!(f, "out of memory"),
        }
    }
}

impl std::error::Error for Error {}

/// Trait to abstract over different page sizes based on the underlying architecture.
pub trait PageSize: Copy + Eq + PartialOrd + Ord {
    /// The page size in bytes.
    const SIZE: u64;
}

pub trait Page: PageSize + Write + Read + Deref + DerefMut + AsRef<[u8]> + AsMut<[u8]> {
    /// `write_at` is like `write`, but tries to start writing at the given offset in the page.
    /// An offset of 0 indicates the beginning of the page.
    fn write_at(&mut self, offset: usize, buf: &[u8]) -> std::io::Result<usize>;
    /// `read_at` is like read, but tries to read from a given offset in the page.
    /// An offset of 0 indicates the beginning of the page
    fn read_at(&mut self, offset: usize, buf: &mut [u8]) -> std::io::Result<usize>;

    fn size(&self) -> usize;
}

pub trait PageAllocator<P: Page> {
    fn allocate(&self, size: u64) -> Result<P, Error>;
}

/// align an address to the beginning of the page
pub fn align_floor<S: PageSize>(addr: u64) -> u64 {
    addr & !(S::SIZE - 1)
}

/// align an address to the beginning of the next page
pub fn align_ceil<S: PageSize>(addr: u64) -> u64 {
    (addr + S::SIZE - 1) & !(S::SIZE - 1)
}
