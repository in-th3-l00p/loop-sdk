mod eth;
mod schema;
mod server;

use proc_macro::TokenStream;
use syn::{DeriveInput, Error, ItemFn, ItemStruct, LitStr, parse_macro_input};

#[proc_macro_attribute]
pub fn rest(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as server::endpoint::RestArgs);
    let function = parse_macro_input!(item as ItemFn);
    server::endpoint::rest(args, function)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn sse(attr: TokenStream, item: TokenStream) -> TokenStream {
    let url = parse_macro_input!(attr as LitStr);
    let function = parse_macro_input!(item as ItemFn);
    server::endpoint::sse(url, function)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn live(attr: TokenStream, item: TokenStream) -> TokenStream {
    let url = parse_macro_input!(attr as LitStr);
    let function = parse_macro_input!(item as ItemFn);
    server::endpoint::live(url, function)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn contract(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(attr as LitStr);
    let item = parse_macro_input!(item as ItemStruct);
    eth::contract::expand(path, item)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Schema, attributes(check))]
pub fn derive_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    schema::derive::expand(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
