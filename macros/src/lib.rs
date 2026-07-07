mod check;
mod derive;
mod endpoint;

use proc_macro::TokenStream;
use syn::{DeriveInput, Error, ItemFn, LitStr, parse_macro_input};

#[proc_macro_attribute]
pub fn rest(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as endpoint::RestArgs);
    let function = parse_macro_input!(item as ItemFn);
    endpoint::rest(args, function)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn sse(attr: TokenStream, item: TokenStream) -> TokenStream {
    let url = parse_macro_input!(attr as LitStr);
    let function = parse_macro_input!(item as ItemFn);
    endpoint::sse(url, function)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn live(attr: TokenStream, item: TokenStream) -> TokenStream {
    let url = parse_macro_input!(attr as LitStr);
    let function = parse_macro_input!(item as ItemFn);
    endpoint::live(url, function)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Schema, attributes(check))]
pub fn derive_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive::expand(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
