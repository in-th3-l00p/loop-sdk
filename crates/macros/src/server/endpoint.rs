use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Error, FnArg, Ident, ItemFn, LitStr, Pat, Result, Token, Type};

use crate::schema::check;

pub struct RestArgs {
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

pub fn rest(args: RestArgs, function: ItemFn) -> Result<TokenStream> {
    let RestArgs { method, url } = args;
    let access = quote! {
        ::lib::server::endpoint::Access::Rest {
            method: ::lib::server::endpoint::Method::#method,
            url: #url.into(),
        }
    };
    expand(function, access, Kind::Call)
}

pub fn sse(url: LitStr, function: ItemFn) -> Result<TokenStream> {
    let access = quote! { ::lib::server::endpoint::Access::Sse { url: #url.into() } };
    expand(function, access, Kind::Stream)
}

pub fn live(url: LitStr, function: ItemFn) -> Result<TokenStream> {
    let access = quote! { ::lib::server::endpoint::Access::Live { url: #url.into() } };
    expand(function, access, Kind::Stream)
}

enum Kind {
    Call,
    Stream,
}

fn expand(mut function: ItemFn, access: TokenStream, kind: Kind) -> Result<TokenStream> {
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
            quote! { ::lib::server::endpoint::IntoHandlerOutput },
            quote! { <<__R as ::lib::server::endpoint::IntoHandlerOutput>::Ok as ::lib::schema::AsSchema>::schema() },
            quote! { ::lib::server::endpoint::IntoHandlerOutput::into_handler_output(#name(#(#arg_names),*)) },
        ),
        Kind::Stream => (
            quote! { ::lib::server::endpoint::StreamOutput },
            quote! { <<__R as ::lib::server::endpoint::StreamOutput>::Item as ::lib::schema::AsSchema>::schema() },
            quote! { ::lib::server::endpoint::StreamOutput::into_value_stream(#name(#(#arg_names),*)) },
        ),
    };

    let binding = match kind {
        Kind::Call => quote! { ::lib::server::endpoint::Binding::Native },
        Kind::Stream => quote! { ::lib::server::endpoint::Binding::Stream },
    };

    Ok(quote! {
        #function

        #[doc(hidden)]
        fn #factory() -> ::lib::server::endpoint::Endpoint {
            fn __output_schema<__F, __R>(_: &__F) -> ::lib::schema::Schema
            where
                __F: Fn(#(#arg_types),*) -> __R,
                __R: #output_trait,
            {
                #output_schema
            }

            ::lib::server::endpoint::Endpoint {
                name: #name_str.into(),
                signature: ::lib::server::endpoint::Signature {
                    params: vec![
                        #(::lib::server::endpoint::Parameter {
                            name: #arg_labels.into(),
                            schema: #arg_schemas,
                        },)*
                    ],
                    output: __output_schema(&#name),
                },
                access: #access,
                binding: #binding(::std::sync::Arc::new(
                    |__ctx: &::lib::server::endpoint::Context,
                     __args: &[::lib::schema::Value]| {
                        let _ = __ctx;
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
            ::lib::server::endpoint::Registration(#factory)
        }
    })
}
