use core::fmt;
use core::ops::{Add, AddAssign, Sub, SubAssign};

/// Alias for consistent imports
#[cfg(any(target_arch = "x86_64"))]
pub type VirtAddr = x86_64::VirtAddr;

/// Limit the physical address range to 48 bits. As we are strickly identity mapping, this ensures
/// that no invalid addresses can be used.
#[cfg(any(target_arch = "x86_64"))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PhysAddr(u64);

#[cfg(any(target_arch = "x86_64"))]
impl PhysAddr {
    const MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

    /// Creates a new physical address.
    ///
    /// The provided address should already be limited to 48bit.
    ///
    /// ## Panics
    ///
    /// This function panics if the bits in the range 49 to 64 are invalid (ie not 0)
    #[inline]
    pub const fn new(addr: u64) -> Self {
        assert!(addr & !Self::MASK == 0);
        Self(addr)
    }

    /// Creates a new physical address, throwing out bits 49..64.
    #[inline]
    pub const fn new_truncate(addr: u64) -> Self {
        Self(addr & Self::MASK)
    }

    /// Converts the address to an `u64`.

    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    
    #[inline]
    pub const fn as_virt_addr(self) -> VirtAddr {
        VirtAddr::new_truncate(self.0)
    }
}

impl fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("PhysAddr")
            .field(&format_args!("{:#x}", self.0))
            .finish()
    }
}

impl fmt::Binary for PhysAddr {
    #[inline]

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Binary::fmt(&self.0, f)
    }
}

impl fmt::LowerHex for PhysAddr {
    #[inline]

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::Octal for PhysAddr {
    #[inline]

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Octal::fmt(&self.0, f)
    }
}

impl fmt::UpperHex for PhysAddr {
    #[inline]

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

impl Add<u64> for PhysAddr {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        PhysAddr::new(self.0 + rhs)
    }
}

impl AddAssign<u64> for PhysAddr {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        *self = *self + rhs;
    }
}

impl Sub<u64> for PhysAddr {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        PhysAddr::new(self.0.checked_sub(rhs).unwrap())
    }
}

impl SubAssign<u64> for PhysAddr {
    #[inline]
    fn sub_assign(&mut self, rhs: u64) {
        *self = *self - rhs;
    }
}

impl Sub<PhysAddr> for PhysAddr {
    type Output = u64;

    #[inline]
    fn sub(self, rhs: PhysAddr) -> Self::Output {
        self.as_u64().checked_sub(rhs.as_u64()).unwrap()
    }
}
