#![no_std]
#![no_main]

use bmvm_guest::Foreign;
use bmvm_guest::{ForeignBuf, Shared, SharedBuf, alloc_buf};
use bmvm_guest::{TypeSignature, entry, expose, host};

#[repr(transparent)]
#[derive(TypeSignature)]
struct Foo(Bar);

#[repr(C)]
#[derive(TypeSignature)]
struct Bar {
    a: u32,
    b: u32,
}

/*
#[host]
unsafe extern "C" {
    fn a();
    fn b(a: u32);
    fn c(a: Shared<Foo>);
    fn d(a: SharedBuf);
    fn e() -> u32;
    fn f(a: u32) -> u32;
    fn g(a: u32, b: i32) -> u32;
}
*/

#[expose]
extern "C" fn h() {}

#[expose]
extern "C" fn i(a: u32) {}

#[expose]
extern "C" fn j(a: u32) {}

#[expose]
extern "C" fn k(a: ForeignBuf) {}

#[expose]
extern "C" fn l(a: Foreign<Foo>) {}

#[expose]
extern "C" fn m(a: u32) -> u32 {
    0u32
}

#[expose]
extern "C" fn n(a: u32) -> SharedBuf {
    let buf = unsafe { alloc_buf(16) }.ok().unwrap();
    buf.into_shared()
}

#[entry]
fn main() {}
