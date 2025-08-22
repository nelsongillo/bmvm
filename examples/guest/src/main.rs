#![no_std]
#![no_main]

use bmvm_guest::host;
use bmvm_guest::{SharedBuf, expose};

#[host]
unsafe extern "C" {
    fn add(a: u64, b: u64) -> u64;
}

#[expose]
fn hypercall_redirect() -> u64 {
    add(10, 20)
}
