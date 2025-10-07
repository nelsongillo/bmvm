#![no_std]
#![no_main]

use bmvm_guest::hypercall;
use bmvm_guest::upcall;

#[hypercall]
unsafe extern "C" {
    fn add(a: u64, b: u64) -> u64;
}

#[upcall]
fn hypercall_redirect() -> u64 {
    add(10, 20)
}
