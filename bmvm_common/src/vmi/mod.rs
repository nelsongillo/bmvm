#[cfg(any(feature = "vmi-consume", feature = "std"))]
mod meta;

#[cfg(any(feature = "vmi-consume", feature = "std"))]
pub use meta::*;
pub type Signature = u64;

#[repr(C)]
pub struct UpcallFn {
    pub sig: Signature,
    pub func: extern "C" fn() -> (),
}

unsafe extern "C" {
    static __start_bmvm_vmi_upcalls: UpcallFn;
    static __stop_bmvm_vmi_upcalls: UpcallFn;
}

pub fn upcalls() -> &'static [UpcallFn] {
    let start = unsafe { &__start_bmvm_vmi_upcalls as *const _ as usize };
    let end = unsafe { &__stop_bmvm_vmi_upcalls as *const _ as usize };
    let count = (end - start) / size_of::<UpcallFn>();
    unsafe { core::slice::from_raw_parts(&__start_bmvm_vmi_upcalls, count) }
}
