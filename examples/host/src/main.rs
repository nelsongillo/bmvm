use bmvm_common::registry::Serializable;
use bmvm_host::expose;

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
struct Foo {
    a: u32,
}

impl<'de> Serializable<'de> for Foo {}

#[expose]
fn sample(a: u32, foo: Foo) {
    println!("{0:?}, {1:?}\n", a, foo);
}

#[expose]
fn fooo(a: u32) {
    println!("{0:?}", a);
}

fn main() {}
