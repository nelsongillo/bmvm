#![feature(proc_macro_span)]

mod common;
mod guest;
mod host;
mod typehash;

use proc_macro::TokenStream;

#[cfg(all(feature = "host", feature = "guest"))]
compile_error!("Features `host` and `guest` cannot be enabled at the same time.");

#[cfg(not(any(feature = "host", feature = "guest")))]
compile_error!("Either feature `host` or `guest` must be enabled!");

/// Define a custom environment setup function, which should be run after the VM setup finished.
///
/// # Example
/// ```
/// #[bmvm_macros::setup]
/// fn custom_setup() {}
/// ```
#[proc_macro_attribute]
pub fn setup(attr: TokenStream, item: TokenStream) -> TokenStream {
    guest::setup_impl(attr, item)
}

/// This attribute marks a function as a host-provided function.
/// It is a guest-only attribute.
#[proc_macro_attribute]
pub fn host(attr: TokenStream, item: TokenStream) -> TokenStream {
    guest::host_impl(attr, item)
}

/// This attribute enables the attributed function to be called from the host side.
/// It is a guest-only attribute.
#[proc_macro_attribute]
pub fn expose_guest(attr: TokenStream, item: TokenStream) -> TokenStream {
    guest::expose_impl(attr, item)
}

/// This attribute enables the attributed function to be called from the guest-side. It should
/// match an equivalent external function definition on the guest side marked with `#[host]`.
/// It is a host-only attribute.
#[proc_macro_attribute]
pub fn expose_host(attr: TokenStream, item: TokenStream) -> TokenStream {
    host::expose_impl(attr, item)
}

#[proc_macro_derive(TypeSignature)]
pub fn derive_type_signature(input: TokenStream) -> TokenStream {
    typehash::derive_type_signature_impl(input)
}
