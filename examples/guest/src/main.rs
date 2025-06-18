#![no_std]
#![no_main]
extern crate bmvm_guest;

use core::arch::asm;

fn write(port: u16, value: u8) {
    unsafe {
        asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        );
    }
}

fn main() {
    write(0x123, 0x80);
}

#[unsafe(no_mangle)]
pub extern "C" fn __process_entry() {
    main();
}
