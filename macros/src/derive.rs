use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, Result};

use crate::check;

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "derive(Schema) does not support generic structs",
        ));
    }
    let Data::Struct(data) = &input.data else {
        return Err(Error::new_spanned(
            &input.ident,
            "derive(Schema) supports structs only",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(Error::new_spanned(
            &input.ident,
            "derive(Schema) requires named fields",
        ));
    };

    let name = &input.ident;
    let mut schema_fields = Vec::new();
    let mut into_fields = Vec::new();
    let mut from_fields = Vec::new();
    let mut idents = Vec::new();

    for field in &fields.named {
        let ident = field.ident.clone().expect("named field");
        let ty = &field.ty;
        let label = ident.to_string();
        let missing = format!("missing field {label:?}");

        let constraints = check::parse(&field.attrs, ty)?;
        let schema = check::schema(ty, &constraints);

        schema_fields.push(quote! { (#label.to_string(), #schema) });
        into_fields.push(quote! {
            (
                ::lib::schema::Value::Str(#label.to_string()),
                ::lib::schema::IntoValue::into_value(self.#ident),
            )
        });
        from_fields.push(quote! {
            let #ident = {
                let index = entries
                    .iter()
                    .position(|(key, _)| {
                        matches!(key, ::lib::schema::Value::Str(name) if name.as_str() == #label)
                    })
                    .ok_or_else(|| ::lib::server::endpoint::HandlerError::from(#missing))?;
                let (_, value) = entries.swap_remove(index);
                <#ty as ::lib::schema::FromValue>::from_value(value)?
            };
        });
        idents.push(ident);
    }

    Ok(quote! {
        impl ::lib::schema::AsSchema for #name {
            fn schema() -> ::lib::schema::Schema {
                ::lib::schema::Schema::Record(vec![#(#schema_fields),*])
            }
        }

        impl ::lib::schema::IntoValue for #name {
            fn into_value(self) -> ::lib::schema::Value {
                ::lib::schema::Value::Map(vec![#(#into_fields),*])
            }
        }

        impl ::lib::schema::FromValue for #name {
            fn from_value(
                value: ::lib::schema::Value,
            ) -> ::std::result::Result<Self, ::lib::server::endpoint::HandlerError> {
                let ::lib::schema::Value::Map(mut entries) = value else {
                    return ::std::result::Result::Err("expected a record".into());
                };
                #(#from_fields)*
                ::std::result::Result::Ok(Self { #(#idents),* })
            }
        }

        impl ::lib::server::endpoint::IntoHandlerOutput for #name {
            type Ok = Self;
            fn into_handler_output(
                self,
            ) -> ::std::result::Result<::lib::schema::Value, ::lib::server::endpoint::HandlerError> {
                ::std::result::Result::Ok(::lib::schema::IntoValue::into_value(self))
            }
        }
    })
}
