use crate::guest::gen_call_meta_debug;
use crate::guest::util::{create_fn_call, gen_callmeta};
use crate::util::{find_crate, is_reference_type, suffix};
use bmvm_common::{BMVM_META_SECTION_EXPOSE, BMVM_META_SECTION_EXPOSE_CALLS};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, format_ident, quote};
use syn::spanned::Spanned;
use syn::{
    Error, FnArg, Ident, ItemFn, Pat, PatType, Type, WherePredicate, parse_macro_input, parse_quote,
};

static PARAM_VAR_NAME: &'static str = "__params";

type StructFields = Vec<TokenStream2>;
type WherePreds = Vec<WherePredicate>;
type ParamUnpacking = Vec<TokenStream2>;

/// A procedural macro that:
/// 1. Checks that all function parameters implement either Type or Serializable trait
/// 2. Creates a C-compatible struct (with repr(C)) containing all parameters
/// 3. Generates a wrapper function that takes the struct, unpacks it, and calls the original function
pub fn expose_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the function
    let input_fn = parse_macro_input!(item as ItemFn);

    // Extract the function name and signature
    let fn_name = &input_fn.sig.ident;

    // bmvm-guest crate
    let crate_guest = match find_crate("bmvm-guest") {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    // vmi metadata generation
    let fn_call = create_fn_call(&input_fn.attrs, &input_fn.sig);
    if fn_call.is_err() {
        return fn_call.err().unwrap().to_compile_error().into();
    }
    let (fn_call, params, return_type) = fn_call.unwrap();
    let upcall_sig = fn_call.signature();

    // generate call meta static data
    let (meta, _) = match gen_callmeta(
        input_fn.span(),
        fn_call,
        params,
        return_type,
        fn_name.to_string().as_str(),
        BMVM_META_SECTION_EXPOSE,
    ) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };
    let debug = gen_call_meta_debug();

    // build struct fields and unpacking logic
    let params = extract_params(&input_fn);
    let (struct_fields, param_where_preds, param_unpacking) = match process_params(&params) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    // construct the function and struct names
    let (wrapper_fn_name, struct_name, static_ptr_name, static_upcall) =
        construct_idents(fn_name, suffix().as_str());

    let params_from_ptr = if params.is_empty() {
        quote! {}.into()
    } else {
        match struct_from_pointer(&struct_name) {
            Ok(x) => x,
            Err(e) => return e.to_compile_error().into(),
        }
    };

    let sort_section_name = format!("{}.{:016x}", BMVM_META_SECTION_EXPOSE_CALLS, upcall_sig);

    // Generate the final token stream
    quote! {
        #debug

        #meta

        #input_fn

        #[repr(C)]
        #[allow(non_camel_case_types)]
        #[derive(#crate_guest::TypeHash)]
        struct #struct_name
        where
            #(#param_where_preds),*
        {
            #(#struct_fields),*
        }

        extern "C" fn #wrapper_fn_name() {
            unsafe {
                #params_from_ptr
                let __ret = #fn_name(#(#param_unpacking),*);

                use #crate_guest::OwnedShareable;
                __ret.write();
            }
        }

        #[used]
        #[allow(non_upper_case_globals)]
        #[unsafe(link_section = #sort_section_name)]
        static #static_upcall: #crate_guest::UpcallFn = #crate_guest::UpcallFn {
            sig: #upcall_sig,
            func: #wrapper_fn_name,
        };
    }
    .into()
}

fn construct_idents(fn_name: &Ident, suffix: &str) -> (Ident, Ident, Ident, Ident) {
    let wrapper_fn_name = format_ident!("{}_bmvm_wrapper_{}", fn_name, suffix);
    let struct_name = format_ident!("{}BMVMWrapper{}", fn_name, suffix);
    let static_ptr_name = format_ident!("PTR_BMVM_FN_WRAPPER_{}", suffix);
    let static_upcall_name = format_ident!("UPCALL_FN_WRAPPER_{}", suffix);
    (
        wrapper_fn_name,
        struct_name,
        static_ptr_name,
        static_upcall_name,
    )
}

/// Extract the function parameters and their types
fn extract_params(func: &ItemFn) -> Vec<(Ident, Type)> {
    func.sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(PatType { pat, ty, .. }) = arg {
                if let Pat::Ident(pat_ident) = &**pat {
                    Some((pat_ident.ident.clone(), (**ty).clone()))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

/// Generate code which reads EBX register for the offset ptr and builds the Foreign<T> for
/// the function params
fn struct_from_pointer(ty: &Ident) -> Result<TokenStream2, Error> {
    let crate_name = find_crate("bmvm-guest")?;
    let raw_offset_ptr = quote! {#crate_name::RawOffsetPtr};
    let get_foreign = quote! {#crate_name::get_foreign};
    let offset_ptr = quote! {#crate_name::OffsetPtr};
    let exit_with_code = quote! {#crate_name::exit_with_code};
    let exit_code_ptr = quote! {#crate_name::ExitCode::Ptr};
    let params = Ident::new(PARAM_VAR_NAME, ty.span());
    Ok(quote! {
        let __offset: u32;
        use core::arch::asm;
        unsafe {
            asm!("mov ebx, {0:e}", out(reg) __offset);
        }
        let __raw = #raw_offset_ptr::from(__offset);
        let __result_foreign = #get_foreign(#offset_ptr::<#ty>::from(__raw));
        let __foreign = match __result_foreign {
            Ok(f) => f,
            Err(e) => #exit_with_code ( #exit_code_ptr ( __raw ) ),
        };
        let #params = __foreign.get();
    })
}

/// Process the function parameters and generate the struct fields and unpacking logic
fn process_params(
    params: &Vec<(Ident, Type)>,
) -> Result<(StructFields, WherePreds, ParamUnpacking), Error> {
    // Resolve the BMVM commons crate and construct trait types
    let crate_bmvm = find_crate("bmvm-guest")?;
    let trait_hashable = quote! {#crate_bmvm::TypeHash};
    let trait_foreign_sharable = quote! {#crate_bmvm::ForeignShareable};
    let var_params = Ident::new(PARAM_VAR_NAME, Span::call_site());

    // fields used in the wrapper struct
    let mut struct_fields = Vec::new();
    // where conditions for trait bounds in the struct
    let mut where_preds: Vec<WherePredicate> = Vec::new();
    // statements to unpack the parameters from struct to function all
    let mut param_unpacking = Vec::new();

    // Process each parameter
    for (name, ty) in params {
        match is_reference_type(ty) {
            Some(referenced) => {
                struct_fields.push(quote! { pub #name: #referenced });
                where_preds.push(parse_quote!(#referenced: #trait_foreign_sharable));
                param_unpacking.push(quote! { &#var_params.#name });
            }
            None => {
                struct_fields.push(quote! { pub #name: #ty });
                where_preds.push(parse_quote!(#ty: #trait_hashable));
                param_unpacking.push(quote! { #var_params.#name });
            }
        }
    }

    Ok((struct_fields, where_preds, param_unpacking))
}
