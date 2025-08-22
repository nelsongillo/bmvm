#![no_std]
#![no_main]

use bmvm_guest::{ExitCode, Foreign, ForeignBuf, SharedBuf, alloc_buf, exit_with_code, expose};
use bmvm_guest::{TypeSignature, host};

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
fn foo(a: u32, b: Foreign<Foo>) -> u32 {
    let foo = b.get();
    a + foo.0.a + foo.0.b
}

#[expose]
fn sum(li: ForeignBuf) -> u64 {
    let mut sum: u64 = 0;
    let buf = li.as_ref();
    for i in 0..buf.len() {
        sum += buf[i] as u64;
    }
    sum
}
