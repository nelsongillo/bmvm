use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, ReturnType, Signature, parse_macro_input};

pub fn setup_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let func_name = &func.sig.ident;

    // Check function signature: fn #name()
    if !is_valid_setup_func(&func.sig) {
        return syn::Error::new_spanned(
            &func.sig,
            "The #[setup] function must have signature `fn #name()` (no args, no return)",
        )
        .to_compile_error()
        .into();
    }

    let wrapper = quote! {
        #func

        #[unsafe(no_mangle)]
        pub fn __environment_setup() {
            #func_name();
        }
    };

    wrapper.into()
}

fn is_valid_setup_func(sig: &Signature) -> bool {
    sig.inputs.is_empty()
        && matches!(sig.output, ReturnType::Default)
        && sig.constness.is_none()
        && sig.asyncness.is_none()
        && sig.unsafety.is_none()
        && sig.abi.is_none()
}
