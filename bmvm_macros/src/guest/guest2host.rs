use crate::util::{find_crate, get_link_name};

use crate::guest::gen_call_meta_debug;
use crate::guest::util::{create_fn_call, gen_callmeta};
use bmvm_common::BMVM_META_SECTION_HOST;
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::Error;
use syn::spanned::Spanned;
use syn::{ForeignItem, ForeignItemFn, ItemForeignMod, parse_macro_input};

const STATIC_META: &str = "BMVM_CALL_META_";
const STATIC_META_TUPLE: &str = "BMVM_CALL_META_TUPLE_";
const STATIC_META_SIG: &str = "BMVM_CALL_META_SIG_";

pub fn host_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
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
            let fn_name = get_link_name(&func.attrs).unwrap_or_else(|| func.sig.ident.to_string());

            // vmi metadata generation
            let fn_call = create_fn_call(&func.attrs, &func.sig);
            if fn_call.is_err() {
                return Error::new(func.span(), fn_call.err().unwrap().to_string())
                    .to_compile_error()
                    .into();
            }
            let (fn_call, params, return_type) = fn_call.unwrap();

            // generate call meta static data
            let callmeta = gen_callmeta(
                func.span(),
                fn_call,
                params,
                return_type,
                fn_name.as_str(),
                BMVM_META_SECTION_HOST,
            );
            if callmeta.is_err() {
                return Error::new(func.span(), callmeta.unwrap_err().to_string())
                    .to_compile_error()
                    .into();
            }
            let (meta, fn_sig_ident) = callmeta.unwrap();

            // vmi hypercall stub generation
            let stub = gen_stub(func, fn_sig_ident);
            if stub.is_err() {
                return Error::new(func.span(), stub.unwrap_err().to_string())
                    .to_compile_error()
                    .into();
            }
            let stub = stub.unwrap();

            Some(quote! {
                #meta
                #stub
            })
        }
        _ => None,
    });

    let debug = gen_call_meta_debug();
    // Combine all the stubs and generate the final output
    let expanded = quote! {
        #debug
        #(#stubs)*
    };

    TokenStream::from(expanded)
}

// TODO:
//  * transport struct
//  * alloc in shared memory and populate transport struct
//  * pass transport struct to hypercall via register
//  * hypercall implementation
//  * return value handling

/// gen_stub generates the call to the hypercall implementation
fn gen_stub(func: &ForeignItemFn, fn_sig_ident: Ident) -> anyhow::Result<proc_macro2::TokenStream> {
    let vis = &func.vis;
    let sig = &func.sig;
    // let ident = &sig.ident;

    // let call_id = meta.id();
    let crate_bmvm = find_crate("bmvm-guest")?;
    let exec_hypercall = quote! {#crate_bmvm::exec_hypercall};

    // Generate the stub implementation
    Ok(quote! {
        #vis #sig{
            // #exec_hypercall(#fn_sig_ident);
        }
    })
}
/*

/// gen_callmeta generates the static data to be embedded in the executable
fn gen_callmeta(func: &ForeignItemFn) -> anyhow::Result<(proc_macro2::TokenStream, Ident)> {
    let (meta, params, return_type) = crate_vmi_call(func)?;
    let fn_name = get_link_name(&func.attrs).unwrap_or_else(|| func.sig.ident.to_string());
    let meta_name_tuple = format_ident!("{}{}", STATIC_META_TUPLE, fn_name.to_uppercase());
    let meta_name_sig = format_ident!("{}{}", STATIC_META_SIG, fn_name.to_uppercase());
    let meta_name = format_ident!("{}{}", STATIC_META, fn_name.to_uppercase());

    // Get the CallMeta as bytes and prefix with the size (u16)
    let bytes = meta.to_bytes();
    let meta_size = bytes.len();
    let (sig_seed_bytes, suffix) = bytes.split_at(8);
    let suffix_size = suffix.len();
    let sig_seed = u64::from_ne_bytes(sig_seed_bytes[0..8].try_into()?);

    // construct fully qualified name for Djb2 and TypeHash for use in the macro output
    let crate_bmvm = find_crate("bmvm-guest")?;
    let type_djb2 = quote! {#crate_bmvm::Djb2};
    let type_typehash = quote! {#crate_bmvm::TypeHash};

    // Convert each string to a syn::Type and quote the hashing line
    let mut hash_lines = params
        .iter()
        .map(|ty| {
            quote! {
                hasher.write(&<#ty as #type_typehash>::TYPE_HASH.to_ne_bytes());
            }
        })
        .collect::<Vec<_>>();
    hash_lines.push(quote! {
        hasher.write(&<#return_type as #type_typehash>::TYPE_HASH.to_ne_bytes());
    });

    // The FnCall signature is stored in the first 8 bytes of the FnCall data. At the moment it is
    // only a partial signature, as the type hashes are not yet known and cannot be included on
    // macro expansion.
    // From the initial (partial) signature, create a new hasher instance and apply the remaining
    // type hashes to it. This will produce the final signature hash.
    // To generate the final output, the signature hash is converted to bytes and replaces the
    // partial signatures bytes in the FnCall data.
    let token = quote! {
        #[used]
        static #meta_name_tuple: ([u8; #meta_size], u64) = {
            let mut hasher = #type_djb2::from_partial(#sig_seed);
            #(#hash_lines)*
            let sig = hasher.finish();
            let sig_bytes = sig.to_ne_bytes();
            let meta_suffix = [#(#suffix),*];

            let mut out = [0u8; #meta_size];
            let mut i = 0;
            while i < 8 {
                out[i] = sig_bytes[i];
                i += 1;
            }
            let mut j = 0;
            while j < #suffix_size {
                out[i + j] = meta_suffix[j];
                j += 1;
            }

            (out, sig)
        };

        #[used]
        #[unsafe(link_section = #BMVM_META_SECTION_HOST)]
        static #meta_name: [u8; #meta_size] = #meta_name_tuple.0;

        #[used]
        static #meta_name_sig: u64 = #meta_name_tuple.1;
    };

    Ok((token, Ident::new(&fn_name, func.span())))
}
*/
