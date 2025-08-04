use crate::common::{
    CallDirection, MOTHER_CRATE, construct_idents, create_fn_call, extract_params, gen_callmeta,
    process_params,
};
use crate::common::{find_crate, suffix};
use crate::guest::{ParamType, gen_call_meta_debug};
use bmvm_common::{BMVM_META_SECTION_EXPOSE, BMVM_META_SECTION_EXPOSE_CALLS};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TS};
use quote::quote;
use syn::{Ident, ItemFn, parse_macro_input};

static PARAM_VAR_NAME: &'static str = "params";

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
    let meta = callmeta.meta;

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
    let var_params = Ident::new(PARAM_VAR_NAME, Span::call_site());

    let func_call = match params {
        ParamType::Void => {
            quote! {
                    let ret = #fn_name();
            }
        }
        ParamType::Value { ty_turbofish, .. } => {
            quote! {
                    let primary: u64;
                    let secondary: u64;
                    unsafe {
                        // Read parameters from registers
                        core::arch::asm! (
                            "mov r8, {0}",
                            "mov r9, {1}",
                            out(reg) primary,
                            out(reg) secondary,
                        );
                    }
                    let input = #ty_transport::new(primary, secondary);
                    use #foreign_shareable;
                    let param = match #ty_turbofish::from_transport(input) {
                        Ok(x) => x,
                        Err(e) => #exit_with_code(e)
                    };

                    let ret = #fn_name(param);
            }
        }
        ParamType::MultipleValues { ty, packaging, .. } => {
            quote! {
                    let primary: u64;
                    let secondary: u64;

                    unsafe {
                        // Read parameters from registers
                        core::arch::asm! (
                            "mov r8, {0}",
                            "mov r9, {1}",
                            out(reg) primary,
                            out(reg) secondary,
                        );
                    }
                    let input = #ty_transport::new(primary, secondary);
                    use #foreign_shareable;
                    let foreign = match #foreign::<#ty>::from_transport(input) {
                        Ok(x) => x,
                        Err(e) => #exit_with_code(e)
                    };

                    let #var_params = foreign.get();
                    let ret = #fn_name(#(#packaging),*);
            }
        }
    };

    quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn #fn_name_wrapper() {
            #func_call
            use #owned_shareable;
            let output = ret.into_transport();
            // Halt to indicate function exit and populate return registers
            unsafe {
                core::arch::asm! (
                    "hlt",
                    "mov r8, {0}",
                    "mov r9, {1}",
                    in(reg) output.primary,
                    in(reg) output.secondary,
                );
            }
        }
    }
}
