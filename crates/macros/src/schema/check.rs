use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Attribute, Error, Ident, Lit, Result, Token, Type, parenthesized};

pub fn extract(attrs: &mut Vec<Attribute>, ty: &Type) -> Result<Vec<TokenStream>> {
    let mut constraints = Vec::new();
    let mut remaining = Vec::new();

    for attr in attrs.drain(..) {
        if !attr.path().is_ident("check") {
            remaining.push(attr);
            continue;
        }
        let items = attr.parse_args_with(Punctuated::<CheckItem, Token![,]>::parse_terminated)?;
        for item in items {
            constraints.push(item.constraint(ty)?);
        }
    }

    *attrs = remaining;
    Ok(constraints)
}

pub fn parse(attrs: &[Attribute], ty: &Type) -> Result<Vec<TokenStream>> {
    let mut owned = attrs.to_vec();
    extract(&mut owned, ty)
}

pub fn schema(ty: &Type, constraints: &[TokenStream]) -> TokenStream {
    let base = quote! { <#ty as ::lib::schema::AsSchema>::schema() };
    if constraints.is_empty() {
        base
    } else {
        quote! {
            ::lib::schema::Schema::Constrained(
                ::std::boxed::Box::new(#base),
                vec![#(#constraints),*],
            )
        }
    }
}

struct CheckItem {
    name: Ident,
    args: CheckArgs,
}

enum CheckArgs {
    Single(CheckLit),
    Many(Vec<CheckLit>),
}

struct CheckLit {
    negative: bool,
    lit: Lit,
}

impl Parse for CheckLit {
    fn parse(input: ParseStream) -> Result<Self> {
        let negative = input.peek(Token![-]);
        if negative {
            input.parse::<Token![-]>()?;
        }
        Ok(CheckLit {
            negative,
            lit: input.parse()?,
        })
    }
}

impl CheckLit {
    fn tokens(&self) -> TokenStream {
        let lit = &self.lit;
        if self.negative {
            quote! { -#lit }
        } else {
            quote! { #lit }
        }
    }
}

impl Parse for CheckItem {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let args = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            CheckArgs::Single(input.parse()?)
        } else if input.peek(syn::token::Paren) {
            let inner;
            parenthesized!(inner in input);
            let literals = Punctuated::<CheckLit, Token![,]>::parse_terminated(&inner)?;
            CheckArgs::Many(literals.into_iter().collect())
        } else {
            return Err(input.error("expected `name = value` or `name(values...)`"));
        };
        Ok(CheckItem { name, args })
    }
}

impl CheckItem {
    fn constraint(&self, ty: &Type) -> Result<TokenStream> {
        match (self.name.to_string().as_str(), &self.args) {
            ("min", CheckArgs::Single(value)) => {
                let value = value.tokens();
                Ok(quote! { ::lib::schema::Constraint::Min(#value as f64) })
            }
            ("max", CheckArgs::Single(value)) => {
                let value = value.tokens();
                Ok(quote! { ::lib::schema::Constraint::Max(#value as f64) })
            }
            ("min_len", CheckArgs::Single(value)) => {
                let value = value.tokens();
                Ok(quote! { ::lib::schema::Constraint::MinLen(#value as usize) })
            }
            ("max_len", CheckArgs::Single(value)) => {
                let value = value.tokens();
                Ok(quote! { ::lib::schema::Constraint::MaxLen(#value as usize) })
            }
            ("pattern", CheckArgs::Single(value)) if matches!(value.lit, Lit::Str(_)) => {
                let lit = &value.lit;
                Ok(quote! { ::lib::schema::Constraint::Pattern(#lit.to_string()) })
            }
            ("pattern", _) => Err(self.error("pattern expects a string literal")),
            ("one_of", CheckArgs::Many(values)) => {
                let values = values
                    .iter()
                    .map(|value| value_of(value, ty))
                    .collect::<Result<Vec<_>>>()?;
                Ok(quote! { ::lib::schema::Constraint::OneOf(vec![#(#values),*]) })
            }
            ("one_of", _) => Err(self.error("one_of expects a list: one_of(a, b, ...)")),
            (other, _) => Err(self.error(&format!("unknown check `{other}`"))),
        }
    }

    fn error(&self, message: &str) -> Error {
        Error::new(self.name.span(), message)
    }
}

fn value_of(value: &CheckLit, ty: &Type) -> Result<TokenStream> {
    match &value.lit {
        Lit::Str(lit) => Ok(quote! { ::lib::schema::IntoValue::into_value(#lit.to_string()) }),
        Lit::Bool(lit) => Ok(quote! { ::lib::schema::IntoValue::into_value(#lit) }),
        Lit::Int(_) | Lit::Float(_) => {
            let tokens = value.tokens();
            Ok(quote! { ::lib::schema::IntoValue::into_value(#tokens as #ty) })
        }
        other => Err(Error::new_spanned(other, "unsupported literal in one_of")),
    }
}
