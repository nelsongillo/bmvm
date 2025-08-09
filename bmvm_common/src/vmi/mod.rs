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
