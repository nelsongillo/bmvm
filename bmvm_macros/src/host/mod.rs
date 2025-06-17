use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

pub fn expose_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input as a foreign module (extern block)
    let func = parse_macro_input!(item as ItemFn);

    TokenStream::from(quote!(#func))
}
