#![no_std]
#![no_main]

use core::panic::PanicInfo;
use bmvm_guest::{call_host, entry};

#[call_host]
unsafe extern "C" {
    fn foo();
}

#[entry]
fn main() {
    
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
