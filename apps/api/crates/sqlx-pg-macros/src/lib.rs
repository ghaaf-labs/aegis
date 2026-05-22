use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, LitStr, Type};

#[proc_macro_derive(FromRow, attributes(sqlx))]
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            _ => {
                return quote! {
                    compile_error!("sqlx-pg FromRow supports named-field structs only");
                }
                .into()
            }
        },
        _ => {
            return quote! {
                compile_error!("sqlx-pg FromRow supports structs only");
            }
            .into()
        }
    };

    let field_reads = fields.into_iter().map(|field| {
        let ident = field.ident.expect("named fields have identifiers");
        let ty = field.ty;
        let column = ident.to_string();
        let attrs = parse_sqlx_attrs(&field.attrs);

        let read_result = if attrs.json {
            quote! {
                row.try_get::<::sqlx::types::Json<#ty>, _>(#column)
                    .map(|value| value.0)
            }
        } else if let Some(source_ty) = attrs.try_from {
            quote! {
                row.try_get::<#source_ty, _>(#column)
                    .and_then(|value| {
                        <#ty as ::std::convert::TryFrom<#source_ty>>::try_from(value)
                        .map_err(|error| ::sqlx::Error::ColumnDecode {
                            index: #column.to_string(),
                            source: ::std::boxed::Box::new(::std::io::Error::new(
                                ::std::io::ErrorKind::InvalidData,
                                error.to_string(),
                            )),
                        })
                    })
            }
        } else {
            quote! {
                row.try_get::<#ty, _>(#column)
            }
        };

        let value = if attrs.default {
            quote! {
                #ident: match #read_result {
                    Ok(value) => value,
                    Err(::sqlx::Error::ColumnNotFound(_)) => ::std::default::Default::default(),
                    Err(error) => return Err(error),
                }
            }
        } else {
            quote! {
                #ident: #read_result?
            }
        };

        value
    });

    quote! {
        impl<'r> ::sqlx::FromRow<'r, ::sqlx::postgres::PgRow> for #name {
            fn from_row(row: &'r ::sqlx::postgres::PgRow) -> ::std::result::Result<Self, ::sqlx::Error> {
                use ::sqlx::Row as _;

                Ok(Self {
                    #(#field_reads,)*
                })
            }
        }
    }
    .into()
}

#[derive(Default)]
struct SqlxAttrs {
    default: bool,
    json: bool,
    try_from: Option<Type>,
}

fn parse_sqlx_attrs(attrs: &[syn::Attribute]) -> SqlxAttrs {
    let mut parsed = SqlxAttrs::default();

    for attr in attrs {
        if !attr.path().is_ident("sqlx") {
            continue;
        }

        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("default") {
                parsed.default = true;
                return Ok(());
            }

            if meta.path.is_ident("json") {
                parsed.json = true;
                return Ok(());
            }

            if meta.path.is_ident("try_from") {
                let value = meta.value()?;
                let source = value.parse::<LitStr>()?;
                parsed.try_from = Some(source.parse()?);
                return Ok(());
            }

            Ok(())
        });
    }

    parsed
}
