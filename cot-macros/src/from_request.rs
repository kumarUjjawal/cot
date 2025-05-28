use crate::cot_ident;
use darling::Error;
use quote::quote;
use syn::{Data, Field, Fields};

pub(super) fn impl_from_request_parts_for_struct(
    ast: &syn::DeriveInput,
) -> proc_macro2::TokenStream {
    let struct_name = &ast.ident;
    let cot = cot_ident();

    let constructor = match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => {
                let initializers = fields_named.named.iter().map(|field: &Field| {
                    let field_name = &field.ident;
                    let field_type = &field.ty;
                    quote! {
                        #field_name: <#field_type as #cot::extract::FromRequestParts>::from_request_parts(parts).await?,
                    }
                });
                quote! { Self { #(#initializers)* } }
            }

            Fields::Unnamed(fields_unnamed) => {
                let initializers = fields_unnamed.unnamed.iter().map(|field: &Field| {
                    let field_type = &field.ty;
                    quote! {
                        <#field_type as #cot::extract::FromRequestParts>::from_request_parts(parts).await?,
                    }
                });
                quote! { Self(#(#initializers)*) }
            }

            Fields::Unit => {
                return quote! {
                    #[automatically_derived]
                    impl #cot::extract::FromRequestParts for #struct_name {
                        async fn from_request_parts(
                            _parts: &mut #cot::http::request::Parts,
                        ) -> #cot::Result<Self> {
                            Ok(Self)
                        }
                    }
                };
            }
        },
        _ => return Error::custom("Only structs can derive `FromRequestParts`").write_errors(),
    };

    quote! {
        #[automatically_derived]
        impl #cot::extract::FromRequestParts for #struct_name {
            async fn from_request_parts(
                parts: &mut #cot::http::request::Parts,
            ) -> #cot::Result<Self> {
                Ok(#constructor)
            }
        }
    }
}
