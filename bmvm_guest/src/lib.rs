#![feature(abi_x86_interrupt)]
#![no_std]
#![no_main]

mod panic;
mod setup;
mod vmi;

use bmvm_common::mem::LayoutTableEntry;
use core::arch::asm;

pub use panic::{exit_with_code, halt, panic, panic_with_code};

pub use bmvm_common::error::ExitCode;
pub use bmvm_common::hash::Djb2;
pub use bmvm_common::mem::{
    Foreign, ForeignBuf, ForeignShareable, OffsetPtr, Owned, OwnedBuf, OwnedShareable,
    RawOffsetPtr, Shared, SharedBuf, alloc, alloc_buf, dealloc, dealloc_buf, get_foreign,
};
pub use bmvm_common::vmi::{Signature, UpcallFn};
// re-export: bmvm-common
pub use bmvm_common::TypeHash;

// re-export: bmvm-macros
pub use bmvm_macros::TypeHash;
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

#[inline]
fn write_u64(port: u16, value: u64) {
    write_buf(port, value.to_le_bytes().as_slice(), 8);
}

fn write_lte(value: LayoutTableEntry) {
    write_u64(0x1, value.as_u64());
}

fn write_addr(addr: u64) {
    write_u64(0x2, addr);
}
