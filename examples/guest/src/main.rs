#![no_std]
#![no_main]

use bmvm_guest::{entry, expose, host};
use core::panic::PanicInfo;


#[host]
unsafe extern "C" {
    fn foo(func: u32, args: u32);
}

#[expose]
fn bar(a: u32) {

}

#[expose]
fn baz(a: u32) {

}


#[entry]
fn main() {}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
