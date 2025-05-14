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

#[proc_macro_attribute]
pub fn call_host(attr: TokenStream, item: TokenStream) -> TokenStream {
    guest::call_host_impl(attr, item)
}

#[proc_macro_attribute]
pub fn impl_host(attr: TokenStream, item: TokenStream) -> TokenStream {
    host::impl_host_impl(attr, item)
}

#[proc_macro_attribute]
pub fn call_guest(attr: TokenStream, item: TokenStream) -> TokenStream {
    host::call_guest_impl(attr, item)
}
