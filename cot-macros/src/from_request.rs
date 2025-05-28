use crate::cot_ident;
use darling::Error;
use quote::quote;
use syn::{Data, Field, Fields};

pub(super) fn impl_from_request_parts_for_struct(
    ast: &syn::DeriveInput,
) -> proc_macro2::TokenStream {
    let struct_name = &ast.ident;
    let cot = cot_ident();

    let fields = if let Data::Struct(data_struct) = &ast.data {
        &data_struct.fields
    } else {
        let err = Error::custom("Only structs can derive `FromRequestParts`");
        return err.write_errors();
    };

    match fields {
        Fields::Named(fields_named) => {
            let field_initializers = fields_named.named.iter().map(|field: &Field| {
                let field_name = &field.ident;
                let field_type = &field.ty;
                quote! {
                    #field_name: #field_type::from_request_parts(parts).await?,
                }
            });

            quote! {
                #[automatically_derived]
                impl
                    impl #cot::axum::extract::FromRequestParts<#cot::http::Request, #cot::anyhow::Error> for #struct_name {
                    async fn from_request_parts(
                        parts: &mut #cot::axum::extract::RequestParts,
                    ) -> ::std::result::Result<Self, #cot::anyhow::Error> {
                        Ok(Self {
                            #(#field_initializers)*
                    })
                    }
                }
            }
        }
        Fields::Unnamed(fields_unnamed) => {
            let field_initializers = fields_unnamed.unnamed.iter().map(|field: &Field| {
                let field_type = &field.ty;
                quote! {
                    #field_type::from_request_parts(parts).await?,
                }
            });

            quote! {
                #[automatically_derived]
                impl
                    impl #cot::axum::extract::FromRequestParts<#cot::http::Request, #cot::anyhow::Error> for #struct_name {
                    async fn from_request_parts(
                        parts: &mut #cot::axum::extract::RequestParts,
                    ) -> ::std::result::Result<Self, #cot::anyhow::Error> {
                        Ok(Self(
                            #(#field_initializers)*
                        ))
                    }
                }
            }
        }
        Fields::Unit => {
            quote! {
                #[automatically_derived]
                impl
                    impl #cot::axum::extract::FromRequestParts<#cot::http::Request, #cot::anyhow::Error> for #struct_name {
                    async fn from_request_parts(
                        parts: &mut #cot::axum::extract::RequestParts,
                    ) -> ::std::result::Result<Self, #cot::anyhow::Error> {
                        Ok(Self)
                    }
                }
            }
        }
    }
}
