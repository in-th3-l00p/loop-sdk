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

/// `User` (or `Option<User>`) parameters come from the request context, not
/// the wire: they are resolved via `FromContext` and never appear in the
/// endpoint's schema. Detection is by type name, so `User` is a reserved
/// parameter type name in endpoint signatures.
fn is_context_param(ty: &Type) -> bool {
    fn last_segment_is_user(ty: &Type) -> bool {
        let Type::Path(path) = ty else { return false };
        path.path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "User")
    }

    if last_segment_is_user(ty) {
        return true;
    }
    // one level of Option<User>
    let Type::Path(path) = ty else { return false };
    let Some(segment) = path.path.segments.last() else {
        return false;
    };
    if segment.ident != "Option" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return false;
    };
    matches!(args.args.first(), Some(syn::GenericArgument::Type(inner))
        if args.args.len() == 1 && last_segment_is_user(inner))
}

fn expand(mut function: ItemFn, access: TokenStream, kind: Kind) -> Result<TokenStream> {
    let name = function.sig.ident.clone();
    let name_str = name.to_string();
    let factory = format_ident!("__loop_endpoint_{name}");

    let mut arg_names = Vec::new();
    let mut arg_types: Vec<Type> = Vec::new();
    let mut wire_names = Vec::new();
    let mut wire_types: Vec<Type> = Vec::new();
    let mut wire_schemas = Vec::new();
    let mut ctx_names = Vec::new();
    let mut ctx_types: Vec<Type> = Vec::new();
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
        if is_context_param(&ty) {
            if arg.attrs.iter().any(|attr| attr.path().is_ident("check")) {
                return Err(Error::new_spanned(
                    &arg.attrs[0],
                    "context parameters like `User` come from the session, \
                     not the wire — #[check] does not apply",
                ));
            }
            ctx_names.push(pat.ident.clone());
            ctx_types.push(ty.clone());
        } else {
            let constraints = check::extract(&mut arg.attrs, &ty)?;
            wire_schemas.push(check::schema(&ty, &constraints));
            wire_names.push(pat.ident.clone());
            wire_types.push(ty.clone());
        }
        arg_names.push(pat.ident.clone());
        arg_types.push(ty);
    }
    let wire_labels: Vec<String> = wire_names.iter().map(Ident::to_string).collect();

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
                            name: #wire_labels.into(),
                            schema: #wire_schemas,
                        },)*
                    ],
                    output: __output_schema(&#name),
                },
                access: #access,
                binding: #binding(::std::sync::Arc::new(
                    |__ctx: &::lib::server::endpoint::Context,
                     __args: &[::lib::schema::Value]| {
                        let _ = __ctx;
                        let [#(#wire_names),*] = __args else {
                            return ::std::result::Result::Err(
                                "wrong number of arguments".into(),
                            );
                        };
                        #(
                            let #wire_names = <#wire_types as ::lib::schema::FromValue>::from_value(
                                #wire_names.clone(),
                            )?;
                        )*
                        #(
                            let #ctx_names =
                                <#ctx_types as ::lib::server::endpoint::FromContext>::from_context(
                                    __ctx,
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
