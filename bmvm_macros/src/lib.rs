#![feature(proc_macro_span)]

mod guest;
mod host;

use proc_macro::TokenStream;

/// Device the VM guest entry point. The marked function will be treated like the main function.
///
/// # Example
/// ```
/// #[bmvm_macros::entry]
/// fn my_main() {}
/// ```
#[proc_macro_attribute]
pub fn entry(attr: TokenStream, item: TokenStream) -> TokenStream {
    guest::entry_impl(attr, item)
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
