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
    fn x(a: Foo, b: i32) -> Foreign<Bar>;
}

#[expose]
fn foo(a: u32, b: Foreign<Foo>) -> SharedBuf {
    let mut buf = unsafe { alloc_buf(16) }.ok().unwrap();
    let b = buf.as_mut();
    b.copy_from_slice(&a.to_le_bytes());
    buf.into_shared()
}

#[entry]
fn main() {}
