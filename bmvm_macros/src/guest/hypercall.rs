use bmvm_common::meta::{CallMeta, DataType};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::Error;
use syn::{parse_macro_input, Attribute, ForeignItem, ForeignItemFn, ItemForeignMod, Meta};

const BMVM_META_SECTION: &str = ".bmvm.call.host";
const BMVM_META_STATIV_PREFIX: &str = "BMVM_CALL_META_";

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
            let meta = gen_callmeta(func);
            let stub = gen_stub(func);
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

fn gen_stub(func: &ForeignItemFn) -> proc_macro2::TokenStream {
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


fn gen_callmeta(func: &ForeignItemFn) -> proc_macro2::TokenStream {
    let fn_name = get_link_name(&func.attrs).unwrap_or_else(|| func.sig.ident.to_string());
    let fn_args = &func.sig.inputs;

    let mut type_codes = Vec::new();

    for arg in fn_args.iter() {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let code = DataType::try_from(*ty.clone());
            if code.is_err() {
                return Error::new(ty.span(), code.err().unwrap().to_string())
                    .to_compile_error()
                    .into();
            }
            type_codes.push(code.unwrap());
        }
    }

    let meta_name = format_ident!("{}{}", BMVM_META_STATIV_PREFIX, fn_name.to_uppercase());
    let meta = CallMeta::new(type_codes, fn_name.as_str());
    if meta.is_err() {
        return Error::new(func.span(), meta.err().unwrap().to_string())
            .to_compile_error()
            .into();
    }

    let meta_bytes = meta.unwrap().as_bytes();
    let size = meta_bytes.len();
    quote! {
        #[used]
        #[unsafe(link_section = #BMVM_META_SECTION)]
        static #meta_name: [u8; #size] = [
            #(#meta_bytes),*
        ];
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
