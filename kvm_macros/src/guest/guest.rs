use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, ReturnType, Signature, parse_macro_input};

pub fn entry_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let func_name = &func.sig.ident;

    // Check function signature: fn #name()
    if !is_valid_entrypoint(&func.sig) {
        return syn::Error::new_spanned(
            &func.sig,
            "The #[entry] function must have signature `fn #name()` (no args, no return)",
        )
        .to_compile_error()
        .into();
    }

    let wrapper = quote! {
        #func

        #[unsafe(no_mangle)]
        pub extern "C" fn __process_entry() {
            #func_name();
        }
    };

    wrapper.into()
}

fn is_valid_entrypoint(sig: &Signature) -> bool {
    sig.inputs.is_empty()
        && matches!(sig.output, ReturnType::Default)
        && sig.constness.is_none()
        && sig.asyncness.is_none()
        && sig.unsafety.is_none()
        && sig.abi.is_none()
}
