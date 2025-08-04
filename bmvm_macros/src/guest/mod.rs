mod entry;
mod guest2host;
mod host2guest;

pub use entry::*;
pub use guest2host::*;
pub use host2guest::*;

use crate::common::{ParamType, VAR_NAME_PARAM, make_type_turbofish};
use proc_macro2::TokenStream;
use quote::quote;

#[cfg(not(any(
    all(debug_assertions, not(feature = "vmi-no-debug")),
    all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
    feature = "vmi-consume",
)))]
/// Stub function which generates no output
fn gen_call_meta_debug() -> TokenStream {
    quote! {}.into()
}

#[cfg(any(
    all(debug_assertions, not(feature = "vmi-no-debug")),
    all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
    feature = "vmi-consume",
))]
/// generate the call meta debug indicator section
fn gen_call_meta_debug() -> TokenStream {
    use bmvm_common::BMVM_META_SECTION_DEBUG;

    let suffix = crate::common::suffix();
    let static_name = quote::format_ident!("BMVM_CALL_META_DEBUG_INDICATOR_{}", suffix);

    quote! {
        #[used]
        #[unsafe(link_section = #BMVM_META_SECTION_DEBUG)]
        static #static_name: [u8; 0] = [];
    }
    .into()
}
