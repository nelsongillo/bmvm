#![no_std]
#![no_main]

use bmvm_guest::{Foreign, SharedBuf, alloc_buf, expose};
use bmvm_guest::{TypeSignature, entry, host};

#[repr(transparent)]
#[derive(TypeSignature)]
struct Foo(Bar);

#[repr(C)]
#[derive(TypeSignature)]
struct Bar {
    a: u32,
    b: u32,
}

#[host]
unsafe extern "C" {
    fn x(a: Foreign<Foo>, b: i32) -> Foreign<Bar>;
}

#[expose]
fn foo(a: u32, b: char) -> SharedBuf {
    let buf = unsafe { alloc_buf(16) }.ok().unwrap();
    buf.into_shared()
}

#[entry]
fn main() {}
