#![no_std]
#![no_main]

use bmvm_guest::setup;

#[setup]
fn noop() {}
