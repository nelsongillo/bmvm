use crate::mem::bits::{AddrSpace, DefaultAddrSpace};
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Sub, SubAssign};

/// Alias for consistent imports
#[cfg(target_arch = "x86_64")]
pub type VirtAddr = x86_64::VirtAddr;

/// Limit the physical address range to the min(supported address bits, 48) bits.
#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PhysAddr<B: AddrSpace = DefaultAddrSpace> {
    inner: u64,
    _bits: PhantomData<B>,
}

#[cfg(target_arch = "x86_64")]
impl<B: AddrSpace> PhysAddr<B> {
    /// Creates a new physical address.
    ///
    /// The provided address should already be limited to 48bit.
    ///
    /// ## Panics
    ///
    /// This function panics if the bits in the range 49 to 64 are invalid (ie not 0)
    #[inline]
    pub fn new(addr: u64) -> Self {
        assert!(addr & !B::mask() == 0);
        Self {
            inner: addr,
            _bits: PhantomData,
        }
    }

    /// Creates a new physical address without asserting the validity. Use when you `know`
    /// the address is correctly formatted.
    pub const fn new_unchecked(addr: u64) -> Self {
        Self {
            inner: addr,
            _bits: PhantomData,
        }
    }

    /// Creates a new physical address, throwing out bits 49..64.
    #[inline]
    pub fn new_truncate(addr: u64) -> Self {
        Self {
            inner: addr & B::mask(),
            _bits: PhantomData,
        }
    }

    /// Converts the address to an `u64`.
    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.inner
    }

    /// Converts the address to an `usize`.
    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.inner as usize
    }

    /// Convert the physical address to a virtual address.
    /// The system uses two address mapping modes:
    /// For physical addresses in the lower half of the the address space, an identity mapping is
    /// being used.
    /// For address in the upper half of the address space, the offset mapping is used, by
    /// shifting left, until the Virtual address boundary is met.
    /// Example: Physical Address Bits: 39
    /// ```
    /// use bmvm_common::mem::{AddrSpace, PhysAddr, VirtAddr};
    ///
    /// #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    /// struct AddrSpace39;
    /// impl AddrSpace for AddrSpace39 {
    ///    fn bits() -> u8 {
    ///        39
    ///    }
    /// }
    ///
    /// let upper = PhysAddr::<AddrSpace39>::new(0x4000000123);
    /// let lower = PhysAddr::<AddrSpace39>::new(0x3000000123);
    /// assert_eq!(upper.as_virt_addr(), VirtAddr::new_truncate(0x800000024600));
    /// assert_eq!(lower.as_virt_addr(), VirtAddr::new_truncate(0x3000000123));
    /// ```
    #[inline]
    pub fn as_virt_addr(self) -> VirtAddr {
        if self.is_upper_half() {
            let shifted = self.inner << (48 - B::bits());
            VirtAddr::new_truncate(shifted)
        } else {
            VirtAddr::new_truncate(self.inner)
        }
    }

    #[inline(always)]
    fn is_upper_half(&self) -> bool {
        (self.inner & (1 << (B::bits() - 1))) != 0
    }
}

impl<B: AddrSpace> fmt::Debug for PhysAddr<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("PhysAddr")
            .field(&format_args!("{:#x}", self.inner))
            .finish()
    }
}

impl<B: AddrSpace> fmt::Binary for PhysAddr<B> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Binary::fmt(&self.inner, f)
    }
}

impl<B: AddrSpace> fmt::LowerHex for PhysAddr<B> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.inner, f)
    }
}

impl<B: AddrSpace> fmt::Octal for PhysAddr<B> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Octal::fmt(&self.inner, f)
    }
}

impl<B: AddrSpace> fmt::UpperHex for PhysAddr<B> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::UpperHex::fmt(&self.inner, f)
    }
}

impl<B: AddrSpace> Add<u64> for PhysAddr<B> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        PhysAddr::new(self.inner + rhs)
    }
}

impl<B: AddrSpace> AddAssign<u64> for PhysAddr<B> {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        self.inner = self.inner + rhs;
    }
}

impl<B: AddrSpace> Sub<u64> for PhysAddr<B> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        PhysAddr::new(self.inner.checked_sub(rhs).unwrap())
    }
}

impl<B: AddrSpace> SubAssign<u64> for PhysAddr<B> {
    #[inline]
    fn sub_assign(&mut self, rhs: u64) {
        self.inner = self.inner - rhs;
    }
}

impl<B: AddrSpace> Sub<PhysAddr<B>> for PhysAddr<B> {
    type Output = u64;

    #[inline]
    fn sub(self, rhs: PhysAddr<B>) -> Self::Output {
        self.as_u64().checked_sub(rhs.as_u64()).unwrap()
    }
}

impl<B: AddrSpace> TryFrom<u64> for PhysAddr<B> {
    type Error = &'static str;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        let addr = PhysAddr::new_truncate(value);
        if addr.as_u64() == value {
            Ok(addr)
        } else {
            Err("Invalid physical address")
        }
    }
}

pub fn virt_to_phys<B: AddrSpace>(vaddr: VirtAddr) -> PhysAddr<B> {
    let raw = vaddr.as_u64();
    if raw & 1 << (48 - 1) == 0 {
        PhysAddr::new(raw)
    } else {
        PhysAddr::new((raw & ((1 << 48) - 1)) >> (48 - B::bits()))
    }
}

mod tests {
    #![allow(unused)]
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct AddrSpace39;
    impl AddrSpace for AddrSpace39 {
        fn bits() -> u8 {
            39
        }
    }

    #[test]
    fn valid_addr_do_not_panic() {
        let _: PhysAddr<AddrSpace39> = PhysAddr::new(0x4000000123);
    }

    #[test]
    #[should_panic]
    fn invalid_addr_panic() {
        let _: PhysAddr<AddrSpace39> = PhysAddr::new(0x8000000000);
    }

    #[test]
    fn is_upper_half() {
        assert!(PhysAddr::<AddrSpace39>::new_unchecked(0x4000000123).is_upper_half());
        assert!(!PhysAddr::<AddrSpace39>::new_unchecked(0x3000000123).is_upper_half());
    }

    #[test]
    fn as_virt_addr_upper_half() {
        let phys: PhysAddr<AddrSpace39> = PhysAddr::new(0x4000000123);
        let virt = phys.as_virt_addr();
        assert_eq!(virt.as_u64(), 0xffff800000024600);
    }

    #[test]
    fn as_virt_addr_lower_half() {
        let phys: PhysAddr<AddrSpace39> = PhysAddr::new(0x3000000123);
        let virt = phys.as_virt_addr();
        assert_eq!(virt.as_u64(), phys.as_u64());
    }

    #[test]
    fn virt_to_phys_test() {
        // mask and shift
        let virt = unsafe { VirtAddr::new_unsafe(0xffff800000024600) };
        assert_eq!(virt_to_phys::<AddrSpace39>(virt).as_u64(), 0x4000000123);

        let virt = unsafe { VirtAddr::new_unsafe(0x3000000123) };
        assert_eq!(virt_to_phys::<AddrSpace39>(virt).as_u64(), 0x3000000123);
    }
}
