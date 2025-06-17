use bmvm_common::hash::{Djb2, Djb264};
use proc_macro::{Span, TokenStream};
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span as Span2, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    FnArg, Ident, ItemFn, Pat, PatType, Type, TypePath, WherePredicate, parse_macro_input,
    parse_quote,
};

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
    let fn_vis = &input_fn.vis;
    let fn_output = &input_fn.sig.output;

    // build struct fields and unpacking logic
    let params = extract_params(&input_fn);
    let (struct_fields, where_preds, param_unpacking) = match process_params(&params) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    // construct the function and struct names
    let suffix = suffix();
    let wrapper_fn_name = format_ident!("{}_bmvm_wrapper_{}", fn_name, suffix);
    let struct_name = format_ident!("{}BMVMWrapper{}", fn_name, suffix);

    // Generate the final token stream
    let expanded = quote! {
        // The original function will remain unchanged
        #input_fn

        // Struct containing all parameters for #fn_name
        #fn_vis struct #struct_name
        where
            #(#where_preds),*
        {
            #(#struct_fields),*
        }


        // Wrapper function for #fn_name
        #fn_vis fn #wrapper_fn_name(params: #struct_name) #fn_output {
            #fn_name(#(#param_unpacking),*)
        }
    };

    TokenStream::from(expanded)
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

/// Process the function parameters and generate the struct fields and unpacking logic
fn process_params(
    params: &Vec<(Ident, Type)>,
) -> Result<(StructFields, WherePreds, ParamUnpacking), syn::Error> {
    // Resolve the BMVM commons crate and construct trait types
    let crate_bmvm = find_crate("bmvm-common")?;
    let trait_type = quote! {#crate_bmvm::registry::Type};
    let trait_serializable = quote! {#crate_bmvm::registry::Serializable};

    // fields used in the wrapper struct
    let mut struct_fields = Vec::new();
    // where conditions for trait bounds in the struct
    let mut where_preds: Vec<WherePredicate> = Vec::new();
    // statements to unpack the parameters from struct to function all
    let mut param_unpacking = Vec::new();

    // Process each parameter
    for (name, ty) in params {
        // Check if the type is an integer type or other Type impl
        let is_type_impl = if let Type::Path(TypePath { path, .. }) = ty {
            if let Some(segment) = path.segments.last() {
                let ident = segment.ident.to_string();
                matches!(
                    ident.as_str(),
                    "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "*const u8"
                )
            } else {
                false
            }
        } else {
            false
        };

        if is_type_impl {
            // For Type-implementing parameters, add them directly
            struct_fields.push(quote! { pub #name: #ty });
            param_unpacking.push(quote! { params.#name });
            where_preds.push(parse_quote!(#ty: #trait_type));
        } else {
            // For Serializable parameters, add pointer and size fields
            let ptr_name = format_ident!("{}_ptr", name);
            let size_name = format_ident!("{}_size", name);

            // Add the pointer and size fields to the struct
            struct_fields.push(quote! { pub #ptr_name: *const u8 });
            struct_fields.push(quote! { pub #size_name: u32 });

            // Add Type trait bounds for the pointer and size
            where_preds.push(parse_quote!(*const u8: #trait_type));
            where_preds.push(parse_quote!(u32: #trait_type));

            // Generate the parameter unpacking logic - converting raw pointer back to the serializable type
            param_unpacking.push(quote! {
                {
                    // Only proceed if the pointer is not null and size > 0
                    if !params.#ptr_name.is_null() && params.#size_name > 0 {
                        // Create a slice from the pointer and size
                        let bytes = unsafe {
                            core::slice::from_raw_parts(params.#ptr_name, params.#size_name as usize)
                        };

                        // Deserialize the slice back to the original type
                        #trait_serializable::from_bytes(bytes)
                    } else {
                        // Handle null pointer or zero size case
                        panic!("Null pointer or zero size for parameter {}", stringify!(#name));
                    }
                }
            });
        }
    }

    Ok((struct_fields, where_preds, param_unpacking))
}

/// Try finding the crate by name to e.g.: generate a proper import statement.
fn find_crate(src: &str) -> Result<Ident, syn::Error> {
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
        FoundCrate::Itself => Ident::new("crate", Span2::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span2::call_site()),
    })
}

/// build the suffix for generated function and struct names based on the calling span
fn suffix() -> String {
    let span = Span::call_site();
    let mut hasher = Djb264::new();
    hasher.write(span.file().as_bytes());
    hasher.write(span.line().to_string().as_bytes());
    hasher.write(span.column().to_string().as_bytes());
    let hash = hasher.finish();
    format!("{:x}", hash)
}
