///! Disclaimer: The code for `PhysAddr` and `VirtAddr` are heavily inspired by the
///! x86_64 crate (https://crates.io/crates/x86_64)
use crate::mem::Align;
use crate::mem::bits::{AddrSpace, DefaultAddrSpace};
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Sub, SubAssign};

/// Limit the physical address range to the min(supported address bits, 48) bits.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysAddr<B: AddrSpace = DefaultAddrSpace> {
    inner: u64,
    _bits: PhantomData<B>,
}

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

    #[inline]
    pub const fn as_ptr<T>(&self) -> *const T {
        self.inner as *const T
    }

    #[inline]
    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.inner as *mut T
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

impl<B: AddrSpace> From<VirtAddr> for PhysAddr<B> {
    fn from(addr: VirtAddr) -> Self {
        let raw = addr.as_u64();
        if raw & 1 << (48 - 1) == 0 {
            PhysAddr::new(raw)
        } else {
            PhysAddr::new((raw & ((1 << 48) - 1)) >> (48 - B::bits()))
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct VirtAddr(u64);

const INDEX_MASK: u64 = 0b1_1111_1111;

impl VirtAddr {
    #[inline]
    pub const fn new(addr: u64) -> Self {
        let virt = Self::new_truncate(addr);
        if virt.as_u64() != addr {
            panic!("virtual address must be sign extended in bits 48 to 64")
        }

        virt
    }

    /// Creates a new virtual address, removing bits 48 to 64 and sign extending bit 47
    #[inline]
    pub const fn new_truncate(addr: u64) -> VirtAddr {
        VirtAddr(((addr << 16) as i64 >> 16) as u64)
    }

    /// Creates a new virtual address, without any checks.
    #[inline]
    pub const fn new_unchecked(addr: u64) -> VirtAddr {
        VirtAddr(addr)
    }

    /// Converts the address to an `u64`.
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Aligns the virtual address upwards to the given alignment.
    #[inline]
    pub fn align_ceil<A>(self) -> Self
    where
        A: Align,
    {
        Self::new_truncate(A::align_ceil(self.0))
    }

    /// Aligns the virtual address downwards to the given alignment.
    #[inline]
    pub fn align_floor<A>(self) -> Self
    where
        A: Align,
    {
        Self::new_truncate(A::align_floor(self.0))
    }

    /// Checks whether the virtual address has the demanded alignment.
    #[inline]
    pub fn is_aligned<A>(self) -> bool
    where
        A: Align,
    {
        A::is_aligned(self.0)
    }

    /// Returns the 9-bit level 1 page table index.
    #[inline]
    pub const fn p1_index(self) -> usize {
        ((self.0 >> 12) & INDEX_MASK) as usize
    }

    /// Returns the 9-bit level 2 page table index.
    #[inline]
    pub const fn p2_index(self) -> usize {
        ((self.0 >> 12 >> 9) & INDEX_MASK) as usize
    }

    /// Returns the 9-bit level 3 page table index.
    #[inline]
    pub const fn p3_index(self) -> usize {
        ((self.0 >> 12 >> 9 >> 9) & INDEX_MASK) as usize
    }

    /// Returns the 9-bit level 4 page table index.
    #[inline]
    pub const fn p4_index(self) -> usize {
        ((self.0 >> 12 >> 9 >> 9 >> 9) & INDEX_MASK) as usize
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as *const () as u64)
    }

    #[inline]
    pub const fn as_ptr<T>(self) -> *const T {
        self.as_u64() as *const T
    }

    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.as_u64() as *mut T
    }
}

impl fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("VirtAddr")
            .field(&format_args!("{:#x}", self.0))
            .finish()
    }
}

impl fmt::Binary for VirtAddr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Binary::fmt(&self.0, f)
    }
}

impl fmt::LowerHex for VirtAddr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::Octal for VirtAddr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Octal::fmt(&self.0, f)
    }
}

impl fmt::UpperHex for VirtAddr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

impl fmt::Pointer for VirtAddr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&(self.0 as *const ()), f)
    }
}

impl Add<u64> for VirtAddr {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        VirtAddr::new(self.0 + rhs)
    }
}

impl AddAssign<u64> for VirtAddr {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        *self = *self + rhs;
    }
}

impl Sub<u64> for VirtAddr {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        VirtAddr::new(self.0.checked_sub(rhs).unwrap())
    }
}

impl SubAssign<u64> for VirtAddr {
    #[inline]
    fn sub_assign(&mut self, rhs: u64) {
        *self = *self - rhs;
    }
}

impl Sub<VirtAddr> for VirtAddr {
    type Output = u64;
    #[inline]
    fn sub(self, rhs: VirtAddr) -> Self::Output {
        self.as_u64().checked_sub(rhs.as_u64()).unwrap()
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
        let virt = unsafe { VirtAddr::new_unchecked(0xffff800000024600) };
        assert_eq!(PhysAddr::<AddrSpace39>::from(virt).as_u64(), 0x4000000123);

        let virt = unsafe { VirtAddr::new_unchecked(0x3000000123) };
        assert_eq!(PhysAddr::<AddrSpace39>::from(virt).as_u64(), 0x3000000123);
    }
}
