use bmvm_host::{Foreign, ForeignBuf, Shared, TypeSignature, alloc, expose};

#[repr(C)]
#[derive(TypeSignature)]
struct Foo {
    a: u32,
    b: ForeignBuf,
}

#[expose]
fn e(a: u32, b: char, c: Foreign<Foo>) -> Shared<Foo> {
    let f = unsafe { alloc::<Foo>() }.ok().unwrap();
    f.into_shared()
}

fn main() {}
