use core::arch::x86_64::__cpuid;
use core::sync::atomic::AtomicU32;
use core::sync::atomic::{AtomicU8, Ordering};
use spin::Once;

pub const ADDR_SPACE_FUNC: u32 = 0x80000008;

static INIT: Once = Once::new();
static EAX: AtomicU32 = AtomicU32::new(0);
static EBX: AtomicU32 = AtomicU32::new(0);
static BITS: AtomicU8 = AtomicU8::new(0);

pub fn cpuid_addr_space() -> (u32, u32) {
    INIT.call_once(|| {
        let result = unsafe { __cpuid(ADDR_SPACE_FUNC) };
        EAX.store(result.eax, Ordering::Relaxed);
        EBX.store(result.ebx, Ordering::Relaxed);
        let phys = (result.eax & 0xFF) as u8;
        let guest = ((result.ebx >> 8) & 0xFF) as u8;
        BITS.store(if guest != 0 { guest } else { phys }, Ordering::Relaxed);
    });
    (EAX.load(Ordering::Relaxed), EBX.load(Ordering::Relaxed))
}

pub(crate) fn phys_address_bits() -> u8 {
    _ = cpuid_addr_space();
    BITS.load(Ordering::Relaxed)
}
