use crate::guest::util::djb2_u32;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::Error;
use syn::{parse_macro_input, Attribute, ForeignItem, ForeignItemFn, ItemForeignMod, Meta};

pub fn call_host_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input as a foreign module (extern block)
    let foreign_mod = parse_macro_input!(item as ItemForeignMod);

    // Verify that this is an extern "C" block
    if !foreign_mod
        .abi
        .name
        .as_ref()
        .map_or(false, |abi| abi.value() == "C")
    {
        return Error::new_spanned(
            foreign_mod.abi,
            "This attribute can only be applied to extern \"C\" blocks",
        )
            .to_compile_error()
            .into();
    }

    // Process each function in the extern block
    let stubs = foreign_mod.items.iter().filter_map(|item| match item {
        ForeignItem::Fn(func) => {
            let stub = generate_stub(func);
            let meta = generate_meta(func);
            Some(quote! {
                #meta
                #stub
            })
        }
        _ => None,
    });

    // Combine all the stubs and generate the final output
    let expanded = quote! {
        #(#stubs)*
    };

    TokenStream::from(expanded)
}

fn generate_meta(func: &ForeignItemFn) -> proc_macro2::TokenStream {
    let func_name = get_link_name(&func.attrs).unwrap_or_else(|| func.sig.ident.to_string());
    let static_name = format_ident!("KVM_HOST_CALL_META_{}", func_name);

    let id = djb2_u32(func_name.as_str());
    let meta_str = format!("{}:{}", id, func_name.to_uppercase());

    // Convert string to a byte array
    let bytes: Vec<u8> = meta_str.bytes().collect();
    let len = bytes.len();
    let byte_array = quote! { [ #(#bytes),* ] };

    quote! {
        #[used]
        #[unsafe(no_mangle)]
        #[unsafe(link_section = ".kvm_meta")]
        static #static_name: [u8; #len] = #byte_array;

    }
}

fn generate_stub(func: &ForeignItemFn) -> proc_macro2::TokenStream {
    let vis = &func.vis;
    let sig = &func.sig;
    let ident = &sig.ident;

    // Generate the stub implementation
    quote! {
        #vis #sig{
            panic!("Stub called for extern \"C\" function {}", stringify!(#ident));
        }
    }
}

fn get_link_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("link_name") {
            if let Meta::NameValue(link_name) = &attr.meta {
                Some(link_name);
            }
        }
    }
    None
}
