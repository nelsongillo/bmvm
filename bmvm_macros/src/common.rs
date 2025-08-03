use bmvm_common::hash::Djb2;
use bmvm_common::vmi::FnCall;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span};
use quote::{ToTokens, format_ident};
use std::ops::Deref;
use syn::spanned::Spanned;
use syn::{Attribute, Error, FnArg, Meta, Pat, PatType, Signature, Type, TypePath, parse_str};

#[cfg(feature = "guest")]
pub const MOTHER_CRATE: &'static str = "bmvm-guest";

#[cfg(feature = "host")]
pub const MOTHER_CRATE: &'static str = "bmvm-host";

/// get_link_name returns name specified via a `link_name` attribute if available
pub(crate) fn get_link_name(attrs: &[Attribute]) -> Option<Ident> {
    for attr in attrs {
        if attr.path().is_ident("link_name") {
            if let Meta::NameValue(link_name) = &attr.meta {
                let link_name_str = link_name.to_token_stream().to_string();
                return Some(Ident::new(link_name_str.as_str(), link_name.span()));
            }
        }
    }
    None
}

/// get the string representation of a type. If the type is not supported, return an error.
pub(crate) fn supported_type_string(ty: &Type) -> Result<String, syn::Error> {
    match ty {
        // Match simple types like u32, i8, etc.
        Type::Path(TypePath { path, .. }) => Ok(path.to_token_stream().to_string()),
        Type::Reference(tr) => Ok(tr.to_token_stream().to_string()),
        _ => Err(syn::Error::new_spanned(ty.clone(), "unsupported type")),
    }
}

/// Checks if the given `syn::Type` is a reference type.
pub(crate) fn is_reference_type(ty: &Type) -> Option<Type> {
    match ty {
        Type::Reference(tr) => Some(tr.elem.deref().clone()),
        _ => None,
    }
}

/// Try finding the crate by name to e.g.: generate a proper import statement.
pub(crate) fn find_crate(src: &str) -> Result<Ident, Error> {
    let found_crate = crate_name(src);
    if found_crate.is_err() {
        return Err(syn::Error::new_spanned(
            src,
            format!(
                "Crate {} could not be found. Make sure to include it in your Cargo.toml",
                src
            ),
        ));
    }

    let found_crate = found_crate.unwrap();
    Ok(match found_crate {
        FoundCrate::Itself => Ident::new("crate", Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    })
}

/// build the suffix for generated function and struct names based on the calling span
pub(crate) fn suffix() -> String {
    let span = proc_macro::Span::call_site();
    let mut hasher = Djb2::new();
    hasher.write(span.file().as_bytes());
    hasher.write(span.line().to_string().as_bytes());
    hasher.write(span.column().to_string().as_bytes());
    let hash = hasher.finish();
    format!("{:x}", hash)
}

/// Try to build the `FnCall` struct from the foreign function definition.
/// The included `sig` is only a partially computed hash, as the struct field-related type hashes
/// are not known during macro expansion and will be calculated later.
pub(crate) fn create_fn_call(
    attrs: &Vec<Attribute>,
    sig: &Signature,
) -> Result<(FnCall, Vec<Type>, Type), Error> {
    let fn_name = get_link_name(&attrs).unwrap_or_else(|| sig.ident.clone());

    // function arguments conversion
    let mut params_str = Vec::new();
    let mut params = Vec::new();
    for arg in sig.inputs.iter() {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            params_str.push(supported_type_string(&ty)?);
            params.push(*ty.clone());
        }
    }

    // return type conversion
    let (rt, _rt_str) = match &sig.output {
        syn::ReturnType::Default => (parse_str::<Type>("()")?, None),
        syn::ReturnType::Type(_, ty) => (*ty.clone(), Some(supported_type_string(&ty)?)),
    };

    // Initializing the function signature
    let init_sig = Djb2::hash(fn_name.to_string().as_bytes());

    #[cfg(any(
        all(debug_assertions, not(feature = "vmi-no-debug")),
        all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
        feature = "vmi-consume",
    ))]
    let call = FnCall::new(init_sig, fn_name.to_string(), &params_str, _rt_str)
        .map_err(|e| Error::new(sig.span(), format!("Failed to create FnCall: {}", e)))?;

    #[cfg(not(any(
        all(debug_assertions, not(feature = "vmi-no-debug")),
        all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
        feature = "vmi-consume",
    )))]
    let call = FnCall::new(init_sig, fn_name.to_string())
        .map_err(|e| Error::new(sig.span(), format!("Failed to create FnCall: {}", e)))?;

    Ok((call, params, rt))
}

/// Extract the function parameters and their types
pub(crate) fn extract_params(sig: &Signature) -> Vec<(Ident, Type)> {
    sig.inputs
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

/// Construct the names of the wrapper function, the struct and the static upcall name
pub(crate) fn construct_idents(fn_name: &Ident, suffix: &str) -> (Ident, Ident, Ident) {
    let wrapper_fn_name = format_ident!("{}_bmvm_wrapper_{}", fn_name, suffix);
    let struct_name = format_ident!("{}BMVMWrapper{}", fn_name, suffix);
    let static_upcall_name = format_ident!("UPCALL_FN_WRAPPER_{}", suffix);
    (wrapper_fn_name, struct_name, static_upcall_name)
}
