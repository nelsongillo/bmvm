use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput};

use crate::common::{MOTHER_CRATE, find_crate};

#[derive(Debug, PartialEq)]
enum Repr {
    C,
    Transparent,
    Other,
}

pub fn derive_type_signature_impl(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let repr = parse_repr(&input);

    // build the fully qualified name of the trait
    let crate_bmvm = match find_crate(MOTHER_CRATE) {
        Ok(crate_name) => crate_name,
        Err(e) => return e.into_compile_error().into(),
    };
    let type_djb2 = quote! {#crate_bmvm::SignatureHasher};
    let type_type_hash = quote! {#crate_bmvm::TypeSignature};

    // Enforce correct representation
    if repr == Repr::Other {
        return syn::Error::new_spanned(
            &input,
            "Struct deriving TypeSignature must have #[repr(C)] or #[repr(transparent)]",
        )
        .into_compile_error()
        .into();
    }

    // Extract all field types and hash the indexes and prepare the field hashes
    let mut computable_hashes = Vec::new();
    computable_hashes.push(quote! {
        let mut hasher = #type_djb2::new();
    });
    let is_primitive: proc_macro2::TokenStream;
    match &input.data {
        Data::Struct(data_struct) => {
            // if the struct is #[repr(transparent)] set the IS_PRIMITIVE based on the field type
            // otherwise it is always false
            is_primitive = match repr {
                Repr::Transparent => {
                    let inner = &data_struct.fields.iter().next().unwrap().ty;
                    quote! { <#inner as #type_type_hash>::IS_PRIMITIVE}
                }
                _ => quote! { false },
            };

            // Precompute the hash value in the macro
            data_struct
                .fields
                .iter()
                .enumerate()
                .for_each(|(index, field)| {
                    let ty = &field.ty;
                    computable_hashes.push(quote! {
                        hasher.write((#index as u64).to_le_bytes().as_slice());
                    });
                    // Assuming/Enforcing non-primitive type will itself implement TypeSignature
                    computable_hashes.push(quote! {
                        hasher.write(<#ty as #type_type_hash>::SIGNATURE.to_le_bytes().as_slice());
                    });
                })
        }
        _ => {
            return syn::Error::new_spanned(
                &input,
                "TypeSignature can only be derived for structs",
            )
            .into_compile_error()
            .into();
        }
    };
    computable_hashes.push(quote! {hasher.finish()});

    #[cfg(feature = "host")]
    let impl_name = quote! {
        fn name() -> String {
                stringify!(#name).to_string()
            }
    };

    #[cfg(feature = "guest")]
    let impl_name = quote! {};

    // Compute the expression for the final hash value
    quote! {
        impl #type_type_hash for #name {
            const SIGNATURE: u64 = {
                #(#computable_hashes)*
            };
            const IS_PRIMITIVE: bool = {
                #is_primitive
            };
            #impl_name
        }
    }
    .into()
}

/// parse the repr attribute
fn parse_repr(input: &DeriveInput) -> Repr {
    for attr in input.attrs.iter() {
        if attr.path().is_ident("repr")
            && let Ok(args) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated,
            )
        {
            if args.iter().any(|arg| arg == "C") {
                return Repr::C;
            } else if args.iter().any(|arg| arg == "transparent") {
                return Repr::Transparent;
            }
        }
    }

    Repr::Other
}
