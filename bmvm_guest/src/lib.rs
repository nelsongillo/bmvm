#![feature(abi_x86_interrupt)]
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
pub use bmvm_common::hash::Djb2;
pub use bmvm_common::mem::{
    Foreign, ForeignBuf, ForeignShareable, OffsetPtr, Owned, OwnedBuf, OwnedShareable,
    RawOffsetPtr, Shared, SharedBuf, Transport, Unpackable, alloc, alloc_buf, dealloc, dealloc_buf,
    get_foreign,
};
pub use bmvm_common::vmi::{Signature, UpcallFn};
pub use bmvm_common::{HYPERCALL_IO_PORT, TypeSignature};

// re-export: bmvm-macros
pub use bmvm_macros::TypeSignature;
pub use bmvm_macros::{entry, expose_guest as expose, host};

unsafe extern "C" {
    fn __process_entry();
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    match setup::setup() {
        Ok(_) => unsafe { __process_entry() },
        Err(err) => exit_with_code(err),
    }

    halt()
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
