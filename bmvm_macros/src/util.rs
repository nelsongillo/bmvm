use bmvm_common::hash::Djb2;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span};
use quote::ToTokens;
use std::ops::Deref;
use syn::{Attribute, Meta, Type, TypePath};

/// get_link_name returns name specified via a `link_name` attribute if available
pub(crate) fn get_link_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("link_name") {
            if let Meta::NameValue(link_name) = &attr.meta {
                Some(link_name);
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
pub(crate) fn find_crate(src: &str) -> Result<Ident, syn::Error> {
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
