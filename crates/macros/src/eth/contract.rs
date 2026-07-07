/* #[contract("abi/erc20.json")] on a unit struct: reads the abi at compile
time and generates typed methods — views become CallBuilders, writes become
TxBuilders, events become schema-carrying structs streamable over #[sse] */

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use serde::Deserialize;
use syn::{Error, Fields, Ident, ItemStruct, LitStr, Result};

#[derive(Deserialize)]
struct AbiEntry {
    #[serde(rename = "type")]
    kind: String,
    name: Option<String>,
    #[serde(default)]
    inputs: Vec<AbiParamDef>,
    #[serde(default)]
    outputs: Vec<AbiParamDef>,
    #[serde(rename = "stateMutability")]
    state_mutability: Option<String>,
    #[serde(default)]
    anonymous: bool,
}

#[derive(Deserialize)]
struct AbiParamDef {
    #[serde(default)]
    name: String,
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    indexed: bool,
    #[serde(default)]
    components: Vec<AbiParamDef>,
}

pub fn expand(path: LitStr, item: ItemStruct) -> Result<TokenStream> {
    if !matches!(item.fields, Fields::Unit) {
        return Err(Error::new_spanned(
            &item.ident,
            "#[contract] expects a unit struct, e.g. `struct Erc20;`",
        ));
    }
    if !item.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &item.generics,
            "#[contract] does not support generic structs",
        ));
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| Error::new(path.span(), "CARGO_MANIFEST_DIR is not set"))?;
    let abi_path = std::path::Path::new(&manifest_dir).join(path.value());
    let json = std::fs::read_to_string(&abi_path).map_err(|e| {
        Error::new(
            path.span(),
            format!("cannot read abi file {}: {e}", abi_path.display()),
        )
    })?;
    let entries: Vec<AbiEntry> = serde_json::from_str(&json).map_err(|e| {
        Error::new(
            path.span(),
            format!("invalid abi json in {}: {e}", abi_path.display()),
        )
    })?;

    let span = path.span();
    let mut methods = Vec::new();
    let mut method_names: Vec<(String, String)> = Vec::new(); // (rust name, abi name)
    let mut event_items = Vec::new();

    for entry in &entries {
        match entry.kind.as_str() {
            "function" => {
                let (name, method) = function_method(entry, span)?;
                if let Some((_, clash)) = method_names.iter().find(|(rust, _)| *rust == name) {
                    return Err(Error::new(
                        span,
                        format!(
                            "overloaded abi functions are not supported yet: {clash:?} and \
                             {:?} both map to `{name}` — trim one from the abi file",
                            entry.name.as_deref().unwrap_or_default(),
                        ),
                    ));
                }
                method_names.push((name, entry.name.clone().unwrap_or_default()));
                methods.push(method);
            }
            "event" if !entry.anonymous => event_items.push(event_struct(entry, span)?),
            // anonymous events have no topic0 to filter on; constructors,
            // fallback/receive and custom errors have no call surface
            _ => {}
        }
    }

    let vis = &item.vis;
    let name = &item.ident;
    let attrs = &item.attrs;
    let abi_path_literal = LitStr::new(&abi_path.display().to_string(), span);

    Ok(quote! {
        #(#attrs)*
        #vis struct #name {
            address: ::lib::eth::Address,
        }

        // ties recompilation of the generated bindings to abi file edits
        const _: &str = ::core::include_str!(#abi_path_literal);

        impl #name {
            /// Binds the abi to a deployed contract. Accepts an `Address` or
            /// a `"0x…"` literal; a malformed literal is a programmer error
            /// and panics — use `try_at` for runtime input.
            pub fn at(address: impl ::lib::eth::IntoAddress) -> #name {
                #name { address: ::lib::eth::IntoAddress::into_address(address) }
            }

            pub fn try_at(address: &str) -> ::std::result::Result<#name, ::lib::eth::EthError> {
                ::std::result::Result::Ok(#name { address: address.parse()? })
            }

            pub fn address(&self) -> ::lib::eth::Address {
                self.address
            }

            /// New events of type `E` as an iterator, from the chain head
            /// onward — ready to return from an `#[sse]` handler.
            pub fn events<E: ::lib::eth::ContractEvent>(
                &self,
            ) -> ::std::result::Result<::lib::eth::EventStream<E>, ::lib::eth::EthError> {
                ::lib::eth::__watch::<E>(self.address)
            }

            #(#methods)*
        }

        #(#event_items)*
    })
}

/// One abi function → one method. Views return a CallBuilder, writes a
/// TxBuilder; both embed the canonical signature the runtime hashes.
fn function_method(entry: &AbiEntry, span: Span) -> Result<(String, TokenStream)> {
    let abi_name = entry
        .name
        .as_deref()
        .ok_or_else(|| Error::new(span, "abi function without a name"))?;
    let method_name = snake_case(abi_name);
    let method = ident(&method_name, span);

    let mut param_names = Vec::new();
    let mut param_types = Vec::new();
    let mut canonical_inputs = Vec::new();
    for (i, input) in entry.inputs.iter().enumerate() {
        let (rust_ty, canonical) = map_type(input, abi_name, span)?;
        let base = if input.name.is_empty() {
            format!("arg{i}")
        } else {
            snake_case(&input.name)
        };
        param_names.push(ident(&base, span));
        param_types.push(rust_ty);
        canonical_inputs.push(canonical);
    }
    let signature = format!("{abi_name}({})", canonical_inputs.join(","));

    let is_view = matches!(
        entry.state_mutability.as_deref(),
        Some("view") | Some("pure")
    );
    let tokens = if is_view {
        let (out_ty, out_canonical) = match entry.outputs.as_slice() {
            [] => (quote!(()), String::new()),
            [output] => map_type(output, abi_name, span)?,
            outputs => {
                return Err(Error::new(
                    span,
                    format!(
                        "abi function {abi_name:?} returns {} values; multi-value returns are \
                         not supported yet — trim it from the abi file",
                        outputs.len()
                    ),
                ));
            }
        };
        quote! {
            pub fn #method(&self, #(#param_names: #param_types),*)
                -> ::lib::eth::CallBuilder<#out_ty>
            {
                ::lib::eth::CallBuilder::new(
                    self.address,
                    #signature,
                    #out_canonical,
                    ::std::vec![#(::lib::eth::abi::AbiType::into_abi(#param_names)),*],
                )
            }
        }
    } else {
        quote! {
            pub fn #method(&self, #(#param_names: #param_types),*) -> ::lib::eth::TxBuilder {
                ::lib::eth::TxBuilder::new(
                    self.address,
                    #signature,
                    ::std::vec![#(::lib::eth::abi::AbiType::into_abi(#param_names)),*],
                )
            }
        }
    };
    Ok((method_name, tokens))
}

/// One non-anonymous abi event → one public struct with the same schema
/// impls #[derive(Schema)] would emit, plus the ContractEvent decoding hooks.
fn event_struct(entry: &AbiEntry, span: Span) -> Result<TokenStream> {
    let abi_name = entry
        .name
        .as_deref()
        .ok_or_else(|| Error::new(span, "abi event without a name"))?;
    let struct_ident = ident(abi_name, span);

    let mut field_idents = Vec::new();
    let mut field_labels = Vec::new();
    let mut field_types = Vec::new();
    let mut fields_const = Vec::new();
    let mut canonical_inputs = Vec::new();
    for (i, input) in entry.inputs.iter().enumerate() {
        let (rust_ty, canonical) = map_type(input, abi_name, span)?;
        let label = if input.name.is_empty() {
            format!("arg{i}")
        } else {
            snake_case(&input.name)
        };
        field_idents.push(ident(&label, span));
        field_labels.push(label);
        field_types.push(rust_ty);
        let indexed = input.indexed;
        fields_const.push(quote!((#canonical, #indexed)));
        canonical_inputs.push(canonical);
    }
    let signature = format!("{abi_name}({})", canonical_inputs.join(","));

    Ok(quote! {
        #[derive(Debug, Clone)]
        pub struct #struct_ident {
            #(pub #field_idents: #field_types),*
        }

        impl ::lib::schema::AsSchema for #struct_ident {
            fn schema() -> ::lib::schema::Schema {
                ::lib::schema::Schema::Record(::std::vec![
                    #((
                        #field_labels.to_string(),
                        <#field_types as ::lib::schema::AsSchema>::schema(),
                    )),*
                ])
            }
        }

        impl ::lib::schema::IntoValue for #struct_ident {
            fn into_value(self) -> ::lib::schema::Value {
                ::lib::schema::Value::Map(::std::vec![
                    #((
                        ::lib::schema::Value::Str(#field_labels.to_string()),
                        ::lib::schema::IntoValue::into_value(self.#field_idents),
                    )),*
                ])
            }
        }

        impl ::lib::server::endpoint::IntoHandlerOutput for #struct_ident {
            type Ok = Self;
            fn into_handler_output(
                self,
            ) -> ::std::result::Result<::lib::schema::Value, ::lib::server::endpoint::HandlerError>
            {
                ::std::result::Result::Ok(::lib::schema::IntoValue::into_value(self))
            }
        }

        impl ::lib::eth::ContractEvent for #struct_ident {
            const SIGNATURE: &'static str = #signature;
            const FIELDS: &'static [(&'static str, bool)] = &[#(#fields_const),*];

            fn from_params(
                params: ::std::vec::Vec<::lib::eth::abi::AbiParam>,
            ) -> ::std::result::Result<Self, ::lib::eth::EthError> {
                let mut params = params.into_iter();
                #(
                    let #field_idents: #field_types =
                        ::lib::eth::abi::AbiType::from_abi(params.next().ok_or_else(|| {
                            ::lib::eth::EthError::Abi("missing event field".to_string())
                        })?)?;
                )*
                ::std::result::Result::Ok(Self { #(#field_idents),* })
            }
        }
    })
}

/// abi type → (Rust type, canonical abi type for signature hashing).
fn map_type(param: &AbiParamDef, owner: &str, span: Span) -> Result<(TokenStream, String)> {
    if !param.components.is_empty() {
        return Err(Error::new(
            span,
            format!(
                "abi entry {owner:?} uses a tuple parameter; tuples are not supported yet — \
                 trim it from the abi file"
            ),
        ));
    }
    map_type_str(&param.ty, owner, span)
}

fn map_type_str(ty: &str, owner: &str, span: Span) -> Result<(TokenStream, String)> {
    // arrays recurse on the element type, keeping the (possibly fixed) suffix
    if let Some(open) = ty.rfind('[') {
        if ty.ends_with(']') {
            let (inner, canonical) = map_type_str(&ty[..open], owner, span)?;
            let suffix = &ty[open..];
            return Ok((
                quote!(::std::vec::Vec<#inner>),
                format!("{canonical}{suffix}"),
            ));
        }
    }

    let unsupported = |reason: &str| {
        Error::new(
            span,
            format!(
                "abi entry {owner:?} uses type {ty:?}: {reason} — trim it from the abi file"
            ),
        )
    };

    match ty {
        "address" => Ok((quote!(::lib::eth::Address), "address".into())),
        "bool" => Ok((quote!(bool), "bool".into())),
        "string" => Ok((quote!(::std::string::String), "string".into())),
        "bytes" => Ok((quote!(::lib::schema::Blob), "bytes".into())),
        "tuple" => Err(unsupported("tuples are not supported yet")),
        _ => {
            if let Some(size) = ty.strip_prefix("bytes") {
                size.parse::<usize>()
                    .ok()
                    .filter(|size| (1..=32).contains(size))
                    .ok_or_else(|| unsupported("not a valid abi type"))?;
                return Ok((quote!(::lib::schema::Blob), ty.into()));
            }
            if let Some(bits) = ty.strip_prefix("uint") {
                let bits: u32 = if bits.is_empty() {
                    256
                } else {
                    bits.parse().map_err(|_| unsupported("not a valid abi type"))?
                };
                let rust = match bits {
                    1..=32 => quote!(u32),
                    33..=64 => quote!(u64),
                    65..=256 => quote!(::lib::eth::U256),
                    _ => return Err(unsupported("not a valid abi type")),
                };
                return Ok((rust, format!("uint{bits}")));
            }
            if let Some(bits) = ty.strip_prefix("int") {
                let bits: u32 = if bits.is_empty() {
                    256
                } else {
                    bits.parse().map_err(|_| unsupported("not a valid abi type"))?
                };
                let rust = match bits {
                    1..=32 => quote!(i32),
                    33..=64 => quote!(i64),
                    65..=256 => {
                        return Err(unsupported(
                            "signed integers wider than 64 bits are not supported yet",
                        ));
                    }
                    _ => return Err(unsupported("not a valid abi type")),
                };
                return Ok((rust, format!("int{bits}")));
            }
            Err(unsupported("not a supported abi type"))
        }
    }
}

fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Builds an identifier, falling back to a raw identifier for abi names that
/// collide with Rust keywords (`type`, `move`, …).
fn ident(name: &str, span: Span) -> Ident {
    match syn::parse_str::<Ident>(name) {
        Ok(_) => Ident::new(name, span),
        // raw identifiers cannot spell these four; disambiguate instead
        Err(_) if matches!(name, "self" | "Self" | "super" | "crate") => {
            format_ident!("{name}_", span = span)
        }
        Err(_) => Ident::new_raw(name, span),
    }
}
