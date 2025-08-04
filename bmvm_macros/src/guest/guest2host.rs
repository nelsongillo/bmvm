use crate::common::{
    CallDirection, MOTHER_CRATE, VAR_NAME_TRANSPORT, construct_idents, create_fn_call,
    extract_params, gen_callmeta, process_params, suffix,
};
use crate::common::{find_crate, get_link_name};
use crate::guest::{ParamType, VAR_NAME_PARAM, gen_call_meta_debug, make_type_turbofish};
use bmvm_common::BMVM_META_SECTION_HOST;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TS};
use quote::quote;
use syn::Error;
use syn::spanned::Spanned;
use syn::{ForeignItem, ItemForeignMod, parse_macro_input};

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

    // bmvm-(guest|host) crate
    let mother = match find_crate(MOTHER_CRATE) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    // Process each function in the extern block
    let stubs = foreign_mod.items.iter().filter_map(|item| match item {
        ForeignItem::Fn(func) => {
            let fn_name = get_link_name(&func.attrs).unwrap_or_else(|| func.sig.ident.clone());

            // vmi metadata generation
            let fn_call = create_fn_call(&func.attrs, &func.sig);
            if fn_call.is_err() {
                return Error::new(func.span(), fn_call.err().unwrap().to_string())
                    .to_compile_error()
                    .into();
            }
            let (fn_call, params, return_type) = fn_call.unwrap();

            // generate call meta static data
            let callmeta = match gen_callmeta(
                fn_call,
                params,
                return_type,
                fn_name.to_string().as_str(),
                BMVM_META_SECTION_HOST,
            ) {
                Ok(x) => x,
                Err(e) => return e.to_compile_error().into(),
            };

            let (_, transport_struct, _) = construct_idents(&fn_name, suffix().as_str());

            // Parameter processing
            let params = extract_params(&func.sig);
            let param_type = match process_params(
                &mother,
                &transport_struct,
                &params,
                Some(CallDirection::Guest2Host),
            ) {
                Ok(x) => x,
                Err(e) => return e.to_compile_error().into(),
            };

            // optional transport struct definition
            let def_transport_struct = match &param_type {
                ParamType::MultipleValues {
                    struct_definition, ..
                } => struct_definition.clone(),
                _ => quote! {},
            };

            // function construction
            let fn_vis = &func.vis;
            let fn_params = &func.sig.inputs;
            let (fn_return, turbofish_return_type, union_return) = match &func.sig.output {
                syn::ReturnType::Default => (quote! {()}, quote! {()}, true),
                syn::ReturnType::Type(_, ty) => {
                    (quote! {#ty}, make_type_turbofish(&*ty.clone()), false)
                }
            };
            let transport = gen_transport(&mother, &param_type);
            let body = gen_body(
                &mother,
                &turbofish_return_type,
                &callmeta.sig,
                transport,
                union_return,
            )
            .unwrap();
            let where_clause = if let ParamType::Value { ensure_trait, .. } = &param_type {
                quote! {where #ensure_trait}
            } else {
                quote! {}
            };
            // TokenStream containing the static defs for FnCall etc
            let meta = callmeta.token;

            Some(quote! {
                #meta
                #def_transport_struct

                #fn_vis fn #fn_name(#fn_params) -> #fn_return
                #where_clause
                {
                    #body
                }
            })
        }
        _ => None,
    });

    // optional one time debug segment for the whole block
    let debug = gen_call_meta_debug();
    // Combine all the stubs and generate the final output
    let expanded = quote! {
        #debug
        #(#stubs)*
    };

    TokenStream::from(expanded)
}

/// Generate code which reads EBX register for the offset ptr and builds the Foreign<T> for
/// the function params
fn gen_body(
    mother: &Ident,
    ty_return: &TS,
    sig: &Ident,
    transport: TS,
    uinion_return: bool,
) -> Result<TS, Error> {
    let foreign_shareable = quote! {#mother::ForeignShareable};
    let exit_with_code = quote! {#mother::exit_with_code};
    let execute = quote! {#mother::hypercall};
    let var_transport = Ident::new(VAR_NAME_TRANSPORT, Span::call_site());

    let body = if uinion_return {
        quote! {
            #transport
            unsafe { #execute(#sig, #var_transport); }
        }
    } else {
        quote! {
            #transport
            let result = unsafe { #execute(#sig, #var_transport) };
            use #foreign_shareable;
            return match #ty_return::from_transport(result) {
                Ok(ret) => ret,
                Err(e) => #exit_with_code(e),
            }
        }
    };
    Ok(body)
}

fn gen_transport(mother: &Ident, param: &ParamType) -> TS {
    let alloc_owned = quote! {#mother::alloc};
    let exit_with_code = quote! {#mother::exit_with_code};
    let exit_code_alloc = quote! {#mother::ExitCode::AllocatorFailed};
    let owned_shareable = quote! {#mother::OwnedShareable};
    let transport = Ident::new(VAR_NAME_TRANSPORT, Span::call_site());
    let params = Ident::new(VAR_NAME_PARAM, Span::call_site());

    match param {
        ParamType::Void => {
            quote! {
                use #owned_shareable;
                let #transport = ().into_transport();
            }
        }
        ParamType::Value { name, .. } => {
            quote! {
                use #owned_shareable;
                let #transport = #name.into_transport();
            }
        }
        ParamType::MultipleValues {
            ty: struct_ident,
            packaging,
            ..
        } => {
            quote! {
                let mut owned_params = match unsafe { #alloc_owned::<#struct_ident>() } {
                    Ok(m) => m,
                    Err(_) => #exit_with_code(#exit_code_alloc),
                };
                let mut #params = owned_params.as_mut();
                #(#packaging)*

                let shared_params = owned_params.into_shared();
                use #owned_shareable;
                let #transport = shared_params.into_transport();
            }
        }
    }
}
