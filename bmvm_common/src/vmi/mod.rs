#[cfg(any(feature = "vmi-consume", feature = "vmi-macro"))]
mod meta;
pub mod transport;

#[cfg(any(feature = "vmi-consume", feature = "vmi-macro"))]
pub use meta::*;

pub use transport::*;

pub type Signature = u64;

pub type Function = extern "C" fn() -> ();

#[cfg(any(feature = "vmi-execute", feature = "vmi-macro"))]
#[repr(C)]
pub struct UpcallFn {
    pub sig: Signature,
    pub func: Function,
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
