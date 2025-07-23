#![no_std]
#![no_main]

use bmvm_guest::{Foreign, ForeignBuf, TypeHash, entry, expose, host};

#[repr(transparent)]
#[derive(TypeHash)]
struct Foo(Bar);

#[repr(C)]
#[derive(TypeHash)]
struct Bar {
    a: u32,
    b: u32,
}

#[host]
unsafe extern "C" {
    fn foo(_b: Bar, _f: Foo);
    fn another(_a: u32, _b: u32);
}

#[expose]
fn argless() {}

#[expose]
fn with_params(a: u32, b: i64, c: bool, buf: &ForeignBuf, _v: &Foreign<Bar>) -> u64 {
    let mut ret = a as u64 + b as u64;
    for v in buf.as_ref() {
        ret += *v as u64;
    }
    if c { ret + 1 } else { ret - 1 }
}

#[entry]
fn main() {
    foo(Bar { a: 1, b: 2 }, Foo(Bar { a: 3, b: 4 }));
    another(1, 2);
}
