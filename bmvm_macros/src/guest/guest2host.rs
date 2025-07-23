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
