use core::arch::x86_64::__cpuid;
use core::cmp::min;
use core::sync::atomic::{AtomicU8, Ordering};
use spin::Once;

pub const ADDR_SPACE_FUNC: u32 = 0x80000008;

static INIT: Once = Once::new();
static BITS: AtomicU8 = AtomicU8::new(0);

fn phys_address_bits() -> u8 {
    INIT.call_once(|| {
        let result = unsafe { __cpuid(ADDR_SPACE_FUNC) };
        let phys = (result.eax & 0xFF) as u8;
        let guest = ((result.ebx >> 8) & 0xFF) as u8;
        BITS.store(
            if guest != 0 { min(guest, phys) } else { phys },
            Ordering::Relaxed,
        );
    });

    BITS.load(Ordering::Relaxed)
}

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
