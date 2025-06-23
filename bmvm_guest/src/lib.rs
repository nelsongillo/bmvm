#![no_std]
#![no_main]

mod panic;
mod setup;

pub use bmvm_common::error::ExitCode;
use bmvm_common::mem::LayoutTableEntry;
pub use bmvm_macros::{entry, expose_guest as expose, host};
use core::arch::asm;
pub use panic::{exit_with_code, halt, panic, panic_with_code};

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
