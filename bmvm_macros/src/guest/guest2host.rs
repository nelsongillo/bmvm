use anyhow::anyhow;
use bmvm_common::BMVM_META_SECTION;
use bmvm_common::meta::{CallMeta, DataType};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Attribute, ForeignItem, ForeignItemFn, ItemForeignMod, Meta, parse_macro_input};
use syn::{Error, Type, TypePath, TypeReference, TypeSlice};

const BMVM_META_STATIV_PREFIX: &str = "BMVM_CALL_META_";

pub fn host_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input as a foreign module (extern block)
    let foreign_mod = parse_macro_input!(item as ItemForeignMod);

    // Verify that this is an extern "C" block
    if !foreign_mod
        .abi
        .name
        .as_ref()
        .map_or(false, |abi| abi.value() == "C")
    {
        return Error::new_spanned(
            foreign_mod.abi,
            "This attribute can only be applied to extern \"C\" blocks",
        )
        .to_compile_error()
        .into();
    }

    // Process each function in the extern block
    let stubs = foreign_mod.items.iter().filter_map(|item| match item {
        ForeignItem::Fn(func) => {
            let meta = create_callmeta(func);
            if meta.is_err() {
                return Error::new(func.span(), meta.err().unwrap().to_string())
                    .to_compile_error()
                    .into();
            }

            let call = meta.unwrap();
            let meta = gen_callmeta(func, &call);
            let stub = gen_stub(func, &call);
            Some(quote! {
                #meta
                #stub
            })
        }
        _ => None,
    });

    // Combine all the stubs and generate the final output
    let expanded = quote! {
        #(#stubs)*
    };

    TokenStream::from(expanded)
}

/// gen_stub generates the call to the hypercall implementation
fn gen_stub(func: &ForeignItemFn, meta: &CallMeta) -> proc_macro2::TokenStream {
    let vis = &func.vis;
    let sig = &func.sig;
    let ident = &sig.ident;

    let call_id = meta.id();

    // Generate the stub implementation
    quote! {
        #vis #sig{
            exec_hypercall(#call_id);
        }
    }
}

/// gen_callmeta generates the static data to be embedded in the executable
fn gen_callmeta(func: &ForeignItemFn, meta: &CallMeta) -> proc_macro2::TokenStream {
    let fn_name = get_link_name(&func.attrs).unwrap_or_else(|| func.sig.ident.to_string());

    let meta_name = format_ident!("{}{}", BMVM_META_STATIV_PREFIX, fn_name.to_uppercase());

    // Get the CallMeta as bytes and prefix with the size (u16)
    let mut meta_bytes = meta.as_bytes();
    let size = meta_bytes.len();
    let size_bytes = (size as u16).to_ne_bytes();

    // Combine the size and the bytes and generate the final output
    let mut bytes = size_bytes.to_vec();
    bytes.append(&mut meta_bytes);
    let final_size = bytes.len();
    quote! {
        #[used]
        #[unsafe(link_section = #BMVM_META_SECTION)]
        static #meta_name: [u8; #final_size] = [
            #(#bytes),*
        ];
    }
}

/// create_callmeta tries to build the `CallMeta` struct from the foreign function definition
fn create_callmeta(func: &ForeignItemFn) -> anyhow::Result<CallMeta> {
    let fn_name = get_link_name(&func.attrs).unwrap_or_else(|| func.sig.ident.to_string());
    let fn_args = &func.sig.inputs;

    let mut type_codes = Vec::new();

    for arg in fn_args.iter() {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let code = try_datatype_from_type(*ty.clone());
            if code.is_err() {
                return Err(anyhow!(code.err().unwrap().to_string()));
            }
            type_codes.push(code.unwrap());
        }
    }

    CallMeta::new(type_codes, fn_name.as_str())
}

/// get_link_name either returns the function name or the name specified via a `link_name` attribute
fn get_link_name(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("link_name") {
            if let Meta::NameValue(link_name) = &attr.meta {
                Some(link_name);
            }
        }
    }
    None
}

fn try_datatype_from_type(ty: Type) -> Result<DataType, &'static str> {
    match ty {
        // Match simple types like u32, i8, etc.
        Type::Path(TypePath { path, .. }) => {
            if let Some(ident) = path.get_ident() {
                match ident.to_string().as_str() {
                    "u8" => Ok(DataType::UInt8),
                    "u16" => Ok(DataType::UInt16),
                    "u32" => Ok(DataType::UInt32),
                    "u64" => Ok(DataType::UInt64),
                    "i8" => Ok(DataType::Int8),
                    "i16" => Ok(DataType::Int16),
                    "i32" => Ok(DataType::Int32),
                    "i64" => Ok(DataType::Int64),
                    "f32" => Ok(DataType::Float32),
                    "f64" => Ok(DataType::Float64),
                    _ => Err("Unsupported type"),
                }
            } else {
                Err("Unsupported type")
            }
        }

        // Match references: &T or &mut T
        Type::Reference(TypeReference { elem, .. }) => match *elem {
            // Match &[u8]
            Type::Slice(TypeSlice { elem, .. }) => {
                if let Type::Path(TypePath { path, .. }) = *elem {
                    if let Some(ident) = path.get_ident() {
                        if ident == "u8" {
                            return Ok(DataType::Bytes);
                        }
                    }
                }
                Err("Unsupported type")
            }
            _ => Err("Unsupported type"),
        },
        _ => Err("Unsupported type"),
    }
}
