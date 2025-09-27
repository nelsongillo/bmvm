#![no_std]
#![no_main]

mod hypercall;
mod panic;
mod setup;

use core::arch::asm;

pub use hypercall::execute as hypercall;
pub use panic::{exit_with_code, halt, panic, panic_with_code};

// re-export: bmvm-common
pub use bmvm_common::error::ExitCode;
pub use bmvm_common::hash::SignatureHasher;
pub use bmvm_common::mem::{
    Foreign, ForeignBuf, OffsetPtr, Owned, OwnedBuf, RawOffsetPtr, Shared, SharedBuf, Unpackable,
    alloc, alloc_buf, dealloc, dealloc_buf, get_foreign,
};
pub use bmvm_common::vmi::{ForeignShareable, OwnedShareable, Signature, Transport, UpcallFn};
pub use bmvm_common::{EXIT_IO_PORT, HYPERCALL_IO_PORT, TypeSignature};

// re-export: bmvm-macros
use crate::panic::ready;
use crate::setup::setup;
pub use bmvm_macros::TypeSignature;
pub use bmvm_macros::{expose_guest as expose, host};

#[cfg(feature = "setup")]
pub use bmvm_macros::setup;

unsafe extern "C" {
    fn __environment_setup();
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    if let Err(e) = setup() {
        exit_with_code(e);
    }

    #[cfg(feature = "setup")]
    unsafe {
        __environment_setup()
    };

    ready()
}

#[inline]
pub fn write_buf(port: u16, buf: &[u8], len: u16) {
    unsafe {
        asm!(
        "rep outsb",
        in("dx") port,
        in("esi") buf.as_ptr() as u32,
        in("cx") len,
        );
    }
}
