#![no_std]

mod panic;
mod setup;

pub use bmvm_common::error::ExitCode;
pub use bmvm_macros::{entry, expose_guest as expose, host};
use core::arch::asm;
pub use panic::{exit_with_code, halt, panic, panic_with_code};

unsafe extern "C" {
    fn __process_entry();
}

const IO_PORT: u16 = 0x3f8;

fn write_buf(port: u16, buf: &[u8]) {
    unsafe {
        asm!(
        "rep outsb",
        in("dx") port,
        in("si") buf.as_ptr() as u32,
        in("cx") buf.len() as u16,
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let hello = b"Hello, World!";
    write_buf(IO_PORT, hello);

    match setup::setup() {
        Ok(_) => unsafe { __process_entry() },
        Err(err) => exit_with_code(err),
    }

    halt()
}
