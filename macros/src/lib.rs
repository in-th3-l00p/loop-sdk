mod check;
mod derive;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    DeriveInput, Error, FnArg, Ident, ItemFn, LitStr, Pat, Result, Token, Type, parse_macro_input,
};

#[proc_macro_attribute]
pub fn rest(attr: TokenStream, item: TokenStream) -> TokenStream {
    let RestArgs { method, url } = parse_macro_input!(attr as RestArgs);
    let function = parse_macro_input!(item as ItemFn);

    let access = quote! {
        ::lib::endpoint::Access::Rest {
            method: ::lib::endpoint::Method::#method,
            url: #url.into(),
        }
    };
    expand(function, access, Kind::Call)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn sse(attr: TokenStream, item: TokenStream) -> TokenStream {
    let url = parse_macro_input!(attr as LitStr);
    let function = parse_macro_input!(item as ItemFn);

    let access = quote! { ::lib::endpoint::Access::Sse { url: #url.into() } };
    expand(function, access, Kind::Stream)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn live(attr: TokenStream, item: TokenStream) -> TokenStream {
    let url = parse_macro_input!(attr as LitStr);
    let function = parse_macro_input!(item as ItemFn);

    let access = quote! { ::lib::endpoint::Access::Live { url: #url.into() } };
    expand(function, access, Kind::Stream)
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

enum Kind {
    Call,
    Stream,
}

struct RestArgs {
    method: Ident,
    url: LitStr,
}

impl Parse for RestArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let method: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let url: LitStr = input.parse()?;

        let normalized = method.to_string().to_uppercase();
        match normalized.as_str() {
            "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" => Ok(RestArgs {
                method: Ident::new(&normalized, method.span()),
                url,
            }),
            other => Err(Error::new(
                method.span(),
                format!("unsupported HTTP method `{other}`"),
            )),
        }
    }
}

fn expand(mut function: ItemFn, access: TokenStream2, kind: Kind) -> Result<TokenStream2> {
    let name = function.sig.ident.clone();
    let name_str = name.to_string();
    let factory = format_ident!("__loop_endpoint_{name}");

    let mut arg_names = Vec::new();
    let mut arg_types: Vec<Type> = Vec::new();
    let mut arg_schemas = Vec::new();
    for input in function.sig.inputs.iter_mut() {
        let FnArg::Typed(arg) = input else {
            return Err(Error::new_spanned(
                input,
                "endpoint functions cannot take self",
            ));
        };
        let Pat::Ident(pat) = arg.pat.as_ref() else {
            return Err(Error::new_spanned(
                &arg.pat,
                "endpoint parameters must be plain identifiers",
            ));
        };
        let ty = arg.ty.as_ref().clone();
        let constraints = check::extract(&mut arg.attrs, &ty)?;
        arg_schemas.push(check::schema(&ty, &constraints));
        arg_names.push(pat.ident.clone());
        arg_types.push(ty);
    }
    let arg_labels: Vec<String> = arg_names.iter().map(Ident::to_string).collect();

    let (output_trait, output_schema, invoke) = match kind {
        Kind::Call => (
            quote! { ::lib::endpoint::IntoHandlerOutput },
            quote! { <<__R as ::lib::endpoint::IntoHandlerOutput>::Ok as ::lib::schema::AsSchema>::schema() },
            quote! { ::lib::endpoint::IntoHandlerOutput::into_handler_output(#name(#(#arg_names),*)) },
        ),
        Kind::Stream => (
            quote! { ::lib::endpoint::StreamOutput },
            quote! { <<__R as ::lib::endpoint::StreamOutput>::Item as ::lib::schema::AsSchema>::schema() },
            quote! { ::lib::endpoint::StreamOutput::into_value_stream(#name(#(#arg_names),*)) },
        ),
    };

    let binding = match kind {
        Kind::Call => quote! { ::lib::endpoint::Binding::Native },
        Kind::Stream => quote! { ::lib::endpoint::Binding::Stream },
    };

    Ok(quote! {
        #function

        #[doc(hidden)]
        fn #factory() -> ::lib::endpoint::Endpoint {
            fn __output_schema<__F, __R>(_: &__F) -> ::lib::schema::Schema
            where
                __F: Fn(#(#arg_types),*) -> __R,
                __R: #output_trait,
            {
                #output_schema
            }

            ::lib::endpoint::Endpoint {
                name: #name_str.into(),
                signature: ::lib::endpoint::Signature {
                    params: vec![
                        #(::lib::endpoint::Parameter {
                            name: #arg_labels.into(),
                            schema: #arg_schemas,
                        },)*
                    ],
                    output: __output_schema(&#name),
                },
                access: #access,
                binding: #binding(::std::sync::Arc::new(
                    |__args: &[::lib::schema::Value]| {
                        let [#(#arg_names),*] = __args else {
                            return ::std::result::Result::Err(
                                "wrong number of arguments".into(),
                            );
                        };
                        #(
                            let #arg_names = <#arg_types as ::lib::schema::FromValue>::from_value(
                                #arg_names.clone(),
                            )?;
                        )*
                        #invoke
                    },
                )),
            }
        }

        ::lib::inventory::submit! {
            ::lib::endpoint::Registration(#factory)
        }
    })
}
