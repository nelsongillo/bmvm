use crate::util::{find_crate, get_link_name, supported_type_string};
use bmvm_common::hash::Djb2;
use bmvm_common::vmi::FnCall;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Attribute, Error, Signature, Type, parse_str};

const STATIC_META: &str = "BMVM_CALL_META_";
const STATIC_META_TUPLE: &str = "BMVM_CALL_META_TUPLE_";
const STATIC_META_SIG: &str = "BMVM_CALL_META_SIG_";

/// gen_callmeta generates the static data to be embedded in the executable
pub(crate) fn gen_callmeta(
    func_span: Span,
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
    let sig_seed =
        u64::from_ne_bytes(sig_seed_bytes[0..8].try_into().map_err(|e| {
            Error::new(func_span, format!("Failed to convert bytes to u64: {}", e))
        })?);

    // construct fully qualified name for Djb2 and TypeHash for use in the macro output
    let crate_bmvm = find_crate("bmvm-guest")?;
    let type_djb2 = quote! {#crate_bmvm::Djb2};
    let type_typehash = quote! {#crate_bmvm::TypeHash};

    // Convert each string to a syn::Type and quote the hashing line
    let mut hash_lines = params
        .iter()
        .map(|ty| {
            quote! {
                hasher.write(&<#ty as #type_typehash>::TYPE_HASH.to_ne_bytes());
            }
        })
        .collect::<Vec<_>>();
    hash_lines.push(quote! {
        hasher.write(&<#return_type as #type_typehash>::TYPE_HASH.to_ne_bytes());
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

    Ok((token, Ident::new(&fn_name, func_span)))
}

/// Try to build the `FnCall` struct from the foreign function definition.
/// The included `sig` is only a partially computed hash, as the struct field-related type hashes
/// are not known during macro expansion and will be calculated later.
pub(crate) fn create_fn_call(
    attrs: &Vec<Attribute>,
    sig: &Signature,
) -> Result<(FnCall, Vec<Type>, Type), Error> {
    let fn_name = get_link_name(&attrs).unwrap_or_else(|| sig.ident.to_string());

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
    let init_sig = Djb2::hash(fn_name.as_bytes());

    #[cfg(any(
        all(debug_assertions, not(feature = "vmi-no-debug")),
        all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
        feature = "vmi-consume",
    ))]
    let call = FnCall::new(init_sig, fn_name, &params_str, _rt_str)
        .map_err(|e| Error::new(sig.span(), format!("Failed to create FnCall: {}", e)))?;

    #[cfg(not(any(
        all(debug_assertions, not(feature = "vmi-no-debug")),
        all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
        feature = "vmi-consume",
    )))]
    let call = FnCall::new(init_sig, fn_name)
        .map_err(|e| Error::new(sig.span(), format!("Failed to create FnCall: {}", e)))?;

    Ok((call, params, rt))
}
