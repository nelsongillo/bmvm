#![no_std]
#![no_main]
extern crate bmvm_guest;

use core::arch::asm;

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

fn main() {
    let buf = b"Hello, From Guest";
    write_buf(IO_PORT, buf);
}

#[unsafe(no_mangle)]
pub extern "C" fn __process_entry() {
    main();
}
