#![no_std]
#![no_main]


use core::panic::PanicInfo;
use kvm_guest::entry;

#[entry]
fn main() {
    
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
