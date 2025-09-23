#![no_std]
#![no_main]

use bmvm_guest::*;

#[derive(TypeSignature)]
#[repr(C)]
struct Foo {
    a: u32,
    b: bool,
    c: f64,
}

#[expose]
fn bar(_foo: Foreign<Foo>) -> bool {
    true
}
