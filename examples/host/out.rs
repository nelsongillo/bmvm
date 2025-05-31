#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2024::*;
#[macro_use]
extern crate std;
use bmvm_common::registry::Serializable;
use bmvm_host::expose;
struct Foo {
    a: u32,
}
#[automatically_derived]
impl ::core::fmt::Debug for Foo {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(f, "Foo", "a", &&self.a)
    }
}
#[automatically_derived]
impl ::core::clone::Clone for Foo {
    #[inline]
    fn clone(&self) -> Foo {
        let _: ::core::clone::AssertParamIsClone<u32>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for Foo {}
impl Serializable for Foo {
    fn as_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        let mut as_a = self.a.to_le_bytes().to_vec();
        v.append(&mut as_a);
        v
    }
    fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() != 4 {
            {
                {
                    {
                        ::core::panicking::panic_fmt(format_args!("invalid bytes"));
                    };
                };
            };
        }
        let a = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Foo { a }
    }
}
fn sample(a: u32, foo: Foo) {
    {
        ::std::io::_print(format_args!("{0:?}, {1:?}\n\n", a, foo));
    };
}
struct sampleBMVMWrapper85462629aec5feb
where
    u32: bmvm_common::registry::Type,
    *const u8: bmvm_common::registry::Type,
    u32: bmvm_common::registry::Type,
{
    pub a: u32,
    pub foo_ptr: *const u8,
    pub foo_size: u32,
}
fn sample_bmvm_wrapper_85462629aec5feb(params: sampleBMVMWrapper85462629aec5feb) {
    sample(
        params.a,
        {
            if !params.foo_ptr.is_null() && params.foo_size > 0 {
                let bytes = unsafe {
                    core::slice::from_raw_parts(params.foo_ptr, params.foo_size as usize)
                };
                bmvm_common::registry::Serializable::from_bytes(bytes)
            } else {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "Null pointer or zero size for parameter {0}",
                            "foo",
                        ),
                    );
                };
            }
        },
    )
}
fn fooo(a: u32) {
    {
        ::std::io::_print(format_args!("{0:?}\n", a));
    };
}
struct foooBMVMWrapper85462629aec6387
where
    u32: bmvm_common::registry::Type,
{
    pub a: u32,
}
fn fooo_bmvm_wrapper_85462629aec6387(params: foooBMVMWrapper85462629aec6387) {
    fooo(params.a)
}
fn main() {}
