extern crate proc_macro;
use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn api(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // No modificamos el código original de la función.
    // Solo sirve como marcador para que build.rs encuentre la función.
    item
}