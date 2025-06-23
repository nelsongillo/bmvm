use crate::cpuid::phys_address_bits;

pub trait AddrSpace: Clone + Copy + PartialEq + Eq + PartialOrd + Ord {
    fn bits() -> u8;
    fn mask() -> u64 {
        (1 << Self::bits()) - 1
    }
}

pub type DefaultAddrSpace = Impl;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Impl;

impl AddrSpace for Impl {
    fn bits() -> u8 {
        phys_address_bits()
    }
}
