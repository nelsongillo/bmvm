#![no_std]
#![no_main]
use bmvm_common::error::ExitCode;
use bmvm_common::mem::{AddrSpace, DefaultAddrSpace};
use bmvm_guest::exit_with_code;
use core::arch::asm;

// Define the I/O port to write to (example: 0x3F8 for COM1)
const IO_PORT: u16 = 0x3f8;

/// write byte value to I/O port
fn write(port: u16, value: u8) {
    unsafe {
        asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        );
    }
}

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

unsafe fn exit() -> ! {
    unsafe { asm!("hlt", options(noreturn)) }
}

/// entrypoint
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let msg = b"Hello, World!";

    // Write the message to the IO Port
    unsafe {
        write_buf(IO_PORT, msg);
    }

    let bits = DefaultAddrSpace::bits();
    write(IO_PORT, bits);
    exit_with_code(ExitCode::Normal);
    loop {}
}
