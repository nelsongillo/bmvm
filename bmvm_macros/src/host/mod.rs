use crate::common::{
    MOTHER_CRATE, ParamType, VAR_NAME_PARAM, construct_idents, create_fn_call, extract_params,
    gen_callmeta, process_params,
};
use crate::common::{find_crate, suffix};
use bmvm_common::BMVM_META_SECTION_EXPOSE;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TS};
use quote::quote;
use syn::{Ident, ItemFn, parse_macro_input};

static VAR_NAME_TRANSPORT: &'static str = "transport";

/// Procedural macro implementation:
/// * Checks that all function parameters implement TypeSignature and return type implements OwnedShareable trait
/// * Creates a C-compatible struct (with repr(C)) containing all parameters
/// * Generates a wrapper function that takes the struct, unpacks it, and calls the original function
/// * Register the wrapper function in the function inventory
pub fn expose_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the function
    let input_fn = parse_macro_input!(item as ItemFn);

    // Extract the function name and signature
    let fn_name = &input_fn.sig.ident;

    // Crate bmvm-host
    let mother = match find_crate(MOTHER_CRATE) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    // Crate inventory
    let inventory = match find_crate("inventory") {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    // construct the function and struct names
    let (wrapper_fn_name, transport_struct_name, _) = construct_idents(fn_name, suffix().as_str());

    // vmi metadata generation
    let fn_call = create_fn_call(&input_fn.attrs, &input_fn.sig);
    if fn_call.is_err() {
        return fn_call.err().unwrap().to_compile_error().into();
    }
    let (fn_call, params, return_type) = fn_call.unwrap();

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
    let param_type = match process_params(&mother, &transport_struct_name, &params, None) {
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
    // TokenStream containing static FnCall definition etc
    let meta = callmeta.token;
    let ident_meta = callmeta.meta;

    // Generate the final token stream
    quote! {
        #meta
        #transport_struct_definition
        #wrapper
        #input_fn

        #inventory::submit!(#mother::CallableFunction {
            meta: &#ident_meta,
            func: #wrapper_fn_name,
        });
    }
    .into()
}

/// Generates the upcall wrapper, which will be called by the Upcall-Handler
fn gen_wrapper(mother: &Ident, fn_name: &Ident, fn_name_wrapper: &Ident, params: &ParamType) -> TS {
    let ty_transport = quote! {#mother::Transport};
    let ty_result = quote! {#mother::HypercallResult};
    let ty_foreign = quote! {#mother::Foreign};
    let foreign_shareable = quote! {#mother::ForeignShareable};
    let owned_shareable = quote! {#mother::OwnedShareable};
    let var_params = Ident::new(VAR_NAME_PARAM, Span::call_site());
    let var_transport = Ident::new(VAR_NAME_TRANSPORT, Span::call_site());

    let func_call = match params {
        ParamType::Void => {
            quote! {
                let ret = #fn_name();
            }
        }
        ParamType::Value { ty_turbofish, .. } => {
            quote! {
                use #foreign_shareable;
                let #var_params = #ty_turbofish::from_transport(#var_transport)?;
                let ret = #fn_name(#var_params);
            }
        }
        ParamType::MultipleValues { ty, packaging, .. } => {
            // TODO: Fix moving Foreign<T> and ForeignBuf
            quote! {
                use #foreign_shareable;
                let foreign = #ty_foreign::<#ty>::from_transport(#var_transport)?;
                let #var_params = foreign.get();
                let ret = #fn_name(#(#packaging),*);
            }
        }
    };

    quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn #fn_name_wrapper(#var_transport: #ty_transport) -> #ty_result {
            #func_call
            use #owned_shareable;
            Ok(ret.into_transport())
        }
    }
}
