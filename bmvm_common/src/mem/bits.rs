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
        // #[cfg(feature = "std")]
        // use phys_bits_std::phys_address_bits;

        // #[cfg(not(feature = "std"))]
        use phys_bits_nostd::phys_address_bits;

        phys_address_bits()
    }
}

/*
#[cfg(feature = "std")]
mod phys_bits_std {
    use core::arch::x86_64::__cpuid;
    use std::sync::OnceLock;

    static BITS: OnceLock<u8> = OnceLock::new();

    pub fn phys_address_bits() -> u8 {
        *BITS.get_or_init(|| {
            let result = unsafe { __cpuid(0x80000008) };
            let phys = (result.eax & 0xFF) as u8;
            let guest = ((result.ebx >> 8) & 0xFF) as u8;
            if guest != 0 { guest } else { phys }
        })
    }
}

#[cfg(not(feature = "std"))]
*/
mod phys_bits_nostd {
    use core::arch::x86_64::__cpuid;
    use core::sync::atomic::{AtomicU8, Ordering};
    use spin::Once;

    static INIT: Once = Once::new();
    static BITS: AtomicU8 = AtomicU8::new(0);

    pub fn phys_address_bits() -> u8 {
        INIT.call_once(|| {
            let result = unsafe { __cpuid(0x80000008) };
            let phys = (result.eax & 0xFF) as u8;
            let guest = ((result.ebx >> 8) & 0xFF) as u8;
            BITS.store(if guest != 0 { guest } else { phys }, Ordering::Relaxed);
        });
        BITS.load(Ordering::Relaxed)
    }
}
