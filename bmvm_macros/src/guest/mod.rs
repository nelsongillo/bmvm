mod entry;
mod guest2host;
mod host2guest;
mod util;

pub use entry::*;
pub use guest2host::*;
pub use host2guest::*;

use quote::quote;

#[cfg(not(any(
    all(debug_assertions, not(feature = "vmi-no-debug")),
    all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
    feature = "vmi-consume",
)))]
/// Stub function which generates no output
fn gen_call_meta_debug() -> proc_macro2::TokenStream {
    quote! {}.into()
}

#[cfg(any(
    all(debug_assertions, not(feature = "vmi-no-debug")),
    all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
    feature = "vmi-consume",
))]
/// generate the call meta debug indicator section
fn gen_call_meta_debug() -> proc_macro2::TokenStream {
    use crate::util::suffix;
    use bmvm_common::BMVM_META_SECTION_DEBUG;
    use quote::format_ident;

    let suffix = suffix();
    let static_name = format_ident!("BMVM_CALL_META_DEBUG_INDICATOR_{}", suffix);

    quote! {
        #[used]
        #[unsafe(link_section = #BMVM_META_SECTION_DEBUG)]
        static #static_name: [u8; 0] = [];
    }
    .into()
}
