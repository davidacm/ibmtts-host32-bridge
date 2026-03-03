use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, LitInt};

#[proc_macro_attribute]
pub fn api(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Expect attribute like `#[api(0x0001)]` or `#[api(1)]`
    let id_lit = parse_macro_input!(attr as LitInt);

    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let reg_name = syn::Ident::new(&format!("__register_{}", fn_name), fn_name.span());

    let expanded = quote! {
        #input_fn

        #[ctor::ctor]
        fn #reg_name() {
            // register the handler at startup
            crate::worker::register_handler(#id_lit as u16, #fn_name);
        }
    };

    TokenStream::from(expanded)
}
