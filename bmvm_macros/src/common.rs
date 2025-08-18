use bmvm_common::hash::SignatureHasher;
use bmvm_common::vmi::FnCall;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use std::ops::Deref;
use syn::spanned::Spanned;
use syn::{
    Attribute, Error, FnArg, Meta, Pat, PatType, PathArguments, Signature, Type, TypePath,
    WherePredicate, parse_quote, parse_str,
};

#[cfg(feature = "guest")]
pub const MOTHER_CRATE: &'static str = "bmvm-guest";

#[cfg(feature = "host")]
pub const MOTHER_CRATE: &'static str = "bmvm-host";

pub type StructFields = Vec<TokenStream>;
pub type WherePreds = Vec<WherePredicate>;
pub type ParamPackaging = Vec<TokenStream>;

pub static VAR_NAME_PARAM: &'static str = "__params";
pub static VAR_NAME_TRANSPORT: &'static str = "__transport";
pub static VAR_NAME_RETURN: &'static str = "__ret";

pub const STATIC_META: &'static str = "BMVM_CALL_META_";
pub const STATIC_META_TUPLE: &'static str = "BMVM_CALL_META_TUPLE_";
pub const STATIC_META_SIG: &'static str = "BMVM_CALL_META_SIG_";

#[derive(Debug, Clone, Copy)]
pub enum CallDirection {
    Host2Guest,
    Guest2Host,
}

pub enum ParamType {
    Void,
    Value {
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
pub(crate) fn supported_type_string(ty: &Type) -> Result<String, Error> {
    match ty {
        Type::Tuple(tuple) => {
            if tuple.elems.len() == 0 {
                Ok("()".to_string())
            } else {
                Err(Error::new_spanned(ty.clone(), "tuples are not supported"))
            }
        }
        // Match simple types like u32, i8, etc.
        Type::Path(TypePath { path, .. }) => Ok(path.to_token_stream().to_string()),
        Type::Reference(tr) => Ok(tr.to_token_stream().to_string()),
        _ => Err(Error::new_spanned(ty.clone(), "unsupported type")),
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
    let mut hasher = SignatureHasher::new();
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
    let init_sig = SignatureHasher::hash(fn_name.to_string().as_bytes());

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
    let static_upcall_name = format_ident!("UPCALL_FN_WRAPPER_{}_{}", fn_name, suffix);
    (wrapper_fn_name, struct_name, static_upcall_name)
}

pub(crate) fn make_type_turbofish(ty: &Type) -> TokenStream {
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

pub fn process_params(
    mother: &Ident,
    transport_struct: &Ident,
    params: &Vec<(Ident, Type)>,
    call_direction: Option<CallDirection>,
) -> Result<ParamType, Error> {
    // No parameters -> No transport struct needed
    if params.is_empty() {
        return Ok(ParamType::Void);
    }

    // Resolve the BMVM commons crate and construct trait types
    let trait_signature = quote! {#mother::TypeSignature};
    let trait_sharable = match call_direction {
        Some(CallDirection::Host2Guest) => quote! {#mother::ForeignShareable},
        Some(CallDirection::Guest2Host) => quote! {#mother::OwnedShareable},
        // No direction specified, default to callee side sharing
        None => quote! {#mother::ForeignShareable},
    };

    // Single parameter -> make sure it implements the ForeignShareable trait
    if params.len() == 1 {
        let (name, ty) = &params.get(0).unwrap();
        return Ok(ParamType::Value {
            ty_turbofish: make_type_turbofish(ty),
            name: name.clone(),
            ensure_trait: quote! {#ty: #trait_sharable},
        });
    }

    // Multiple parameter

    let mut struct_fields: StructFields = Vec::new();
    // where conditions for trait bounds in the struct
    let mut where_preds: WherePreds = Vec::new();
    // statements to unpack the parameters from struct to function all
    let mut param_packaging: ParamPackaging = Vec::new();
    let mut param_types: Vec<Type> = Vec::new();
    let mut param_read = Vec::new();
    let mut param_names: Vec<Ident> = Vec::new();
    let var_params = Ident::new(VAR_NAME_PARAM, Span::call_site());
    let var_this = Ident::new("this", Span::call_site());

    // Process each parameter
    for (name, ty) in params.iter() {
        if let Some(_) = is_reference_type(ty) {
            return Err(Error::new(
                Span::call_site(),
                "references are not supported.",
            ));
        }

        param_names.push(name.clone());
        param_types.push(ty.clone());
        struct_fields.push(quote! { pub #name: #ty });
        where_preds.push(parse_quote!(#ty: #trait_signature));
        match call_direction {
            Some(CallDirection::Host2Guest) | None => {
                param_read
                    .push(quote! { let #name = unsafe { core::ptr::read(&(*#var_this).#name) }; });
                param_packaging.push(quote! { #name });
            }
            Some(CallDirection::Guest2Host) => {
                param_packaging.push(quote! { #var_params.#name = #name; })
            }
        }
    }

    let trait_transport_container = quote! {#mother::Unpackable};
    let unpack = match call_direction {
        Some(CallDirection::Host2Guest) | None => quote! {
            unsafe impl #trait_transport_container for #transport_struct {
                type Output = (#(#param_types,)*);
                unsafe fn unpack(#var_this: *const Self) -> Self::Output {
                    // Extract the fields we want to own
                    #(#param_read)*
                    return (#(#param_names,)*)
                }
            }
        },
        _ => quote! {},
    };

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

            #unpack
        },
        packaging: param_packaging,
    })
}

pub struct CallMetaResult {
    pub token: TokenStream,
    pub sig: Ident,
    pub meta: Ident,
}

/// gen_callmeta generates the static data to be embedded in the executable
pub fn gen_callmeta(
    meta: FnCall,
    params: Vec<Type>,
    return_type: Type,
    fn_name: &str,
    section_name: &str,
) -> Result<CallMetaResult, Error> {
    let meta_name_tuple = format_ident!("{}{}", STATIC_META_TUPLE, fn_name.to_uppercase());
    let meta_name_sig = format_ident!("{}{}", STATIC_META_SIG, fn_name.to_uppercase());
    let meta_name = format_ident!("{}{}", STATIC_META, fn_name.to_uppercase());
    let var_param_hash = format_ident!("param_hash");

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

    // construct fully qualified name for SignatureHasher and TypeSignature for use in the macro output
    let crate_bmvm = find_crate(MOTHER_CRATE)?;
    let ty_hash = quote! {#crate_bmvm::SignatureHasher};
    let ty_typesignature = quote! {#crate_bmvm::TypeSignature};

    // Convert each string to a syn::Type and quote the hashing line
    let param_hash = if params.len() > 0 {
        let var_hasher = format_ident!("hasher_params");

        let lines = params
            .iter()
            .enumerate()
            .map(|(idx, ty)| {
                quote! {
                    #var_hasher.write((#idx as u64).to_le_bytes().as_slice());
                    #var_hasher.write(<#ty as #ty_typesignature>::SIGNATURE.to_le_bytes().as_slice());
                }
            })
            .collect::<Vec<_>>();

        quote! {
             let mut #var_hasher = #ty_hash::new();
            #(#lines)*
            let #var_param_hash = #var_hasher.finish();
        }
    } else {
        quote! {let #var_param_hash = <() as #ty_typesignature>::SIGNATURE;}
    };

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
            #param_hash
            let mut sig_hasher = #ty_hash::new();
            sig_hasher.write(#fn_name.as_bytes());
            sig_hasher.write(#var_param_hash.to_le_bytes().as_slice());
            sig_hasher.write(<#return_type as #ty_typesignature>::SIGNATURE.to_le_bytes().as_slice());
            let sig = sig_hasher.finish();
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

    Ok(CallMetaResult {
        token,
        sig: meta_name_sig,
        meta: meta_name,
    })
}
