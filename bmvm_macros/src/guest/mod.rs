mod entry;
mod guest2host;
mod host2guest;

pub use entry::*;
pub use guest2host::*;
pub use host2guest::*;

use crate::common::{MOTHER_CRATE, find_crate, is_reference_type};
use bmvm_common::vmi::FnCall;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Error, PathArguments, Type, TypePath, WherePredicate, parse_quote};

type StructFields = Vec<TokenStream>;
type WherePreds = Vec<WherePredicate>;
type ParamPackaging = Vec<TokenStream>;

static VAR_NAME_PARAM: &'static str = "params";
static VAR_NAME_TRANSPORT: &'static str = "transport";

const STATIC_META: &'static str = "BMVM_CALL_META_";
const STATIC_META_TUPLE: &'static str = "BMVM_CALL_META_TUPLE_";
const STATIC_META_SIG: &'static str = "BMVM_CALL_META_SIG_";

enum ParamType {
    Void,
    Value {
        ty: Type,
        ty_turbofish: TokenStream,
        name: Ident,
        ensure_trait: TokenStream,
    },
    MultipleValues {
        ty: Ident,
        struct_definition: TokenStream,
        packaging: ParamPackaging,
    },
}

#[derive(Debug, Clone, Copy)]
enum CallDirection {
    Host2Guest,
    Guest2Host,
}

/// gen_callmeta generates the static data to be embedded in the executable
fn gen_callmeta(
    meta: FnCall,
    params: Vec<Type>,
    return_type: Type,
    fn_name: &str,
    section_name: &str,
) -> Result<(TokenStream, Ident), Error> {
    let meta_name_tuple = format_ident!("{}{}", STATIC_META_TUPLE, fn_name.to_uppercase());
    let meta_name_sig = format_ident!("{}{}", STATIC_META_SIG, fn_name.to_uppercase());
    let meta_name = format_ident!("{}{}", STATIC_META, fn_name.to_uppercase());

    // Get the CallMeta as bytes and prefix with the size (u16)
    let bytes = meta.to_bytes();
    let meta_size = bytes.len();
    let (sig_seed_bytes, suffix) = bytes.split_at(8);
    let suffix_size = suffix.len();
    let sig_seed = u64::from_ne_bytes(sig_seed_bytes[0..8].try_into().map_err(|e| {
        Error::new(
            Span::call_site(),
            format!("Failed to convert bytes to u64: {}", e),
        )
    })?);

    // construct fully qualified name for Djb2 and TypeSignature for use in the macro output
    let crate_bmvm = find_crate(MOTHER_CRATE)?;
    let type_djb2 = quote! {#crate_bmvm::Djb2};
    let type_typehash = quote! {#crate_bmvm::TypeSignature};

    // Convert each string to a syn::Type and quote the hashing line
    let mut hash_lines = params
        .iter()
        .map(|ty| {
            quote! {
                hasher.write(&<#ty as #type_typehash>::SIGNATURE.to_ne_bytes());
            }
        })
        .collect::<Vec<_>>();
    hash_lines.push(quote! {
        hasher.write(&<#return_type as #type_typehash>::SIGNATURE.to_ne_bytes());
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
        #[unsafe(link_section = #section_name)]
        static #meta_name: [u8; #meta_size] = #meta_name_tuple.0;

        #[used]
        static #meta_name_sig: u64 = #meta_name_tuple.1;
    };

    Ok((token, meta_name_sig))
}

#[cfg(not(any(
    all(debug_assertions, not(feature = "vmi-no-debug")),
    all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
    feature = "vmi-consume",
)))]
/// Stub function which generates no output
fn gen_call_meta_debug() -> TokenStream {
    quote! {}.into()
}

#[cfg(any(
    all(debug_assertions, not(feature = "vmi-no-debug")),
    all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
    feature = "vmi-consume",
))]
/// generate the call meta debug indicator section
fn gen_call_meta_debug() -> TokenStream {
    use bmvm_common::BMVM_META_SECTION_DEBUG;

    let suffix = crate::common::suffix();
    let static_name = quote::format_ident!("BMVM_CALL_META_DEBUG_INDICATOR_{}", suffix);

    quote! {
        #[used]
        #[unsafe(link_section = #BMVM_META_SECTION_DEBUG)]
        static #static_name: [u8; 0] = [];
    }
    .into()
}

fn process_params(
    mother: &Ident,
    transport_struct: &Ident,
    params: &Vec<(Ident, Type)>,
    call_direction: CallDirection,
) -> Result<ParamType, Error> {
    // No parameters -> No transport struct needed
    if params.is_empty() {
        return Ok(ParamType::Void);
    }

    // Resolve the BMVM commons crate and construct trait types
    let trait_signature = quote! {#mother::TypeSignature};
    let trait_sharable = match call_direction {
        CallDirection::Host2Guest => quote! {#mother::ForeignShareable},
        CallDirection::Guest2Host => quote! {#mother::OwnedShareable},
    };

    // Single parameter -> make sure it implements the ForeignShareable trait
    if params.len() == 1 {
        let (name, ty) = &params.get(0).unwrap();
        return Ok(ParamType::Value {
            ty: ty.clone(),
            ty_turbofish: make_turbofish_type(ty),
            name: name.clone(),
            ensure_trait: quote! {#ty: #trait_sharable},
        });
    }

    // Multiple parameter

    let mut struct_fields: StructFields = Vec::new();
    // where conditions for trait bounds in the struct
    let mut where_preds: WherePreds = Vec::new();
    // statements to unpack the parameters from struct to function all
    let mut param_unpacking: ParamPackaging = Vec::new();
    let var_params = Ident::new(VAR_NAME_PARAM, Span::call_site());

    // Process each parameter
    for (name, ty) in params {
        if let Some(_) = is_reference_type(ty) {
            return Err(Error::new(
                Span::call_site(),
                "references are not supported.",
            ));
        }

        struct_fields.push(quote! { pub #name: #ty });
        where_preds.push(parse_quote!(#ty: #trait_signature));
        match call_direction {
            CallDirection::Host2Guest => {
                param_unpacking.push(quote! { #var_params.#name });
            }
            CallDirection::Guest2Host => {
                param_unpacking.push(quote! { #var_params.#name = #name; });
            }
        }
    }

    Ok(ParamType::MultipleValues {
        ty: transport_struct.clone(),
        struct_definition: quote! {
            #[repr(C)]
            #[allow(non_camel_case_types)]
            #[derive(#mother::TypeSignature)]
            struct #transport_struct
            where
                #(#where_preds),*
            {
                #(#struct_fields),*
            }
        },
        packaging: param_unpacking,
    })
}

pub fn make_turbofish_type(ty: &Type) -> TokenStream {
    match ty {
        Type::Path(TypePath { path, qself: None }) if path.segments.len() == 1 => {
            let segment = &path.segments[0];
            let ident = &segment.ident;

            match &segment.arguments {
                PathArguments::AngleBracketed(args) => {
                    let generic_args = &args.args;
                    quote! { #ident::<#generic_args> }
                }
                PathArguments::Parenthesized(_) | PathArguments::None => {
                    quote! { #ty }
                }
            }
        }
        Type::Path(TypePath { path, qself: None }) => {
            // Handle multi-segment paths (e.g., std::vec::Vec<T>)
            let mut segments = path.segments.iter();
            let first_segment = segments.next().unwrap();
            let mut new_path = first_segment.ident.to_string();

            for segment in segments {
                new_path.push_str("::");
                new_path.push_str(&segment.ident.to_string());
            }

            if let PathArguments::AngleBracketed(args) = &path.segments.last().unwrap().arguments {
                let generic_args = &args.args;
                let path_ident: syn::Ident = syn::parse_str(&new_path).unwrap();
                quote! { #path_ident::<#generic_args> }
            } else {
                quote! { #ty }
            }
        }
        _ => quote! { #ty },
    }
}
