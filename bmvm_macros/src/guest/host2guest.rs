use crate::common::{
    CallDirection, MOTHER_CRATE, VAR_NAME_PARAM, VAR_NAME_RETURN, construct_idents, create_fn_call,
    extract_params, gen_callmeta, process_params,
};
use crate::common::{find_crate, suffix};
use crate::guest::{ParamType, gen_call_meta_debug};
use bmvm_common::{BMVM_META_SECTION_EXPOSE, BMVM_META_SECTION_EXPOSE_CALLS};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TS};
use quote::quote;
use syn::{Ident, ItemFn, parse_macro_input};

/// Procedural macro implementation:
/// * Checks that all function parameters implement TypeSignature and return type implements OwnedShareable trait
/// * Creates a C-compatible struct (with repr(C)) containing all parameters
/// * Generates a wrapper function that takes the struct, unpacks it, and calls the original function
/// * Create an entry in the distributed slice of exposed function calls
pub fn expose_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the function
    let input_fn = parse_macro_input!(item as ItemFn);

    // Extract the function name and signature
    let fn_name = &input_fn.sig.ident;

    // bmvm-(guest|host) crate
    let mother = match find_crate(MOTHER_CRATE) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    // construct the function and struct names
    let (wrapper_fn_name, transport_struct_name, static_upcall) =
        construct_idents(fn_name, suffix().as_str());

    // vmi metadata generation
    let fn_call = create_fn_call(&input_fn.attrs, &input_fn.sig);
    if fn_call.is_err() {
        return fn_call.err().unwrap().to_compile_error().into();
    }
    let (fn_call, params, return_type) = fn_call.unwrap();
    let upcall_sig = fn_call.signature();

    // generate call meta static data
    let callmeta = match gen_callmeta(
        fn_call,
        params,
        return_type,
        fn_name.to_string().as_str(),
        BMVM_META_SECTION_EXPOSE,
    ) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    // build struct fields and unpacking logic
    let params = extract_params(&input_fn.sig);
    let param_type = match process_params(
        &mother,
        &transport_struct_name,
        &params,
        Some(CallDirection::Host2Guest),
    ) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    // extract optional transport struct definition
    let transport_struct_definition = if let ParamType::MultipleValues {
        struct_definition, ..
    } = &param_type
    {
        struct_definition
    } else {
        &quote! {}
    };

    // function wrapper generation
    let wrapper = gen_wrapper(&mother, fn_name, &wrapper_fn_name, &param_type);
    // section name for sorting upcalls via the linker
    let sort_section_name = format!("{}.{:016x}", BMVM_META_SECTION_EXPOSE_CALLS, upcall_sig);
    // optionally indicate debug information in the metadata
    let debug = gen_call_meta_debug();
    // TokenStream containing static defs for FnCall etc
    let meta = callmeta.token;

    // Generate the final token stream
    quote! {
        #debug
        #meta
        #transport_struct_definition
        #wrapper
        #input_fn

        #[used]
        #[allow(non_upper_case_globals)]
        #[unsafe(link_section = #sort_section_name)]
        static #static_upcall: #mother::UpcallFn = #mother::UpcallFn {
            sig: #upcall_sig,
            func: #wrapper_fn_name,
        };
    }
    .into()
}

/// Generates the upcall wrapper, which will be called by the Upcall-Handler
fn gen_wrapper(mother: &Ident, fn_name: &Ident, fn_name_wrapper: &Ident, params: &ParamType) -> TS {
    let ty_transport = quote! {#mother::Transport};
    let foreign = quote! {#mother::Foreign};
    let foreign_shareable = quote! {#mother::ForeignShareable};
    let owned_shareable = quote! {#mother::OwnedShareable};
    let exit_with_code = quote! {#mother::exit_with_code};
    let var_params = Ident::new(VAR_NAME_PARAM, Span::call_site());
    let var_return = Ident::new(VAR_NAME_RETURN, Span::call_site());

    let func_call = match params {
        ParamType::Void => {
            quote! {
                    let #var_return = #fn_name();
            }
        }
        ParamType::Value { ty_turbofish, .. } => {
            quote! {
                    let __primary: u64;
                    let __secondary: u64;
                    unsafe {
                        // Read parameters from registers
                        core::arch::asm! (
                            "mov r8, {0}",
                            "mov r9, {1}",
                            out(reg) __primary,
                            out(reg) __secondary,
                        );
                    }
                    let __input = #ty_transport::new(__primary, __secondary);
                    use #foreign_shareable;
                    let #var_params = match #ty_turbofish::from_transport(__input) {
                        Ok(x) => x,
                        Err(e) => #exit_with_code(e)
                    };

                    let #var_return = #fn_name(#var_params);
            }
        }
        ParamType::MultipleValues { ty, packaging, .. } => {
            quote! {
                    let __primary: u64;
                    let __secondary: u64;

                    unsafe {
                        // Read parameters from registers
                        core::arch::asm! (
                            "mov r8, {0}",
                            "mov r9, {1}",
                            out(reg) __primary,
                            out(reg) __secondary,
                        );
                    }
                    let __input = #ty_transport::new(__primary, __secondary);
                    use #foreign_shareable;
                    let __foreign = match #foreign::<#ty>::from_transport(__input) {
                        Ok(x) => x,
                        Err(e) => #exit_with_code(e)
                    };

                    let (#(#packaging),*) = unsafe { __foreign.unpack() };
                    let #var_return = #fn_name(#(#packaging),*);
            }
        }
    };

    quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn #fn_name_wrapper() {
            #func_call
            use #owned_shareable;
            let __output = #var_return.into_transport();
            // Halt to indicate function exit and populate return registers
            unsafe {
                core::arch::asm! (
                    "hlt",
                    "mov r8, {0}",
                    "mov r9, {1}",
                    in(reg) __output.primary(),
                    in(reg) __output.secondary(),
                );
            }
        }
    }
}
