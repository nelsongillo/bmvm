/// This is a quick const wrapper for the DefaultAlign::align_floor function
pub const fn align_floor(addr: u64) -> u64 {
    x86_64::align_down(addr, DefaultAlign::ALIGNMENT)
}

/// This is a quick const wrapper for the DefaultAlign::align_ceil function
pub const fn align_ceil(addr: u64) -> u64 {
    x86_64::align_up(addr, DefaultAlign::ALIGNMENT)
}

/// Trait to abstract over different page sizes based on the underlying architecture.
pub trait Align: Copy + Eq + PartialEq + PartialOrd + Ord {
    const ALIGNMENT: u64;

    fn is_aligned(addr: u64) -> bool {
        addr % Self::ALIGNMENT == 0
    }

    /// align an address to the beginning of the page
    fn align_floor(addr: u64) -> u64 {
        x86_64::align_down(addr, Self::ALIGNMENT)
    }

    /// align an address to the beginning of the next page
    fn align_ceil(addr: u64) -> u64 {
        x86_64::align_up(addr, Self::ALIGNMENT)
    }
}

#[cfg(target_arch = "x86_64")]
pub type DefaultAlign = X86_64;

#[cfg(not(target_arch = "x86_64"))]
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

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Page4KiB;

impl Align for Page4KiB {
    const ALIGNMENT: u64 = 0x1000;
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Page2MiB;

impl Align for Page2MiB {
    const ALIGNMENT: u64 = Page4KiB::ALIGNMENT * 512;
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Page1GiB;
impl Align for Page1GiB {
    const ALIGNMENT: u64 = Page2MiB::ALIGNMENT * 512;
}
