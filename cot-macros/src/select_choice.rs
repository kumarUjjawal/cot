use darling::Error;
use quote::quote;
use syn::{Data, Variant};

use crate::cot_ident;

pub(super) fn impl_select_choice_for_enum(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let enum_name = &ast.ident;
    let cot = cot_ident();

    let variants = match &ast.data {
        Data::Enum(data_enum) => &data_enum.variants,
        _ => return Error::custom("SelectChoice can only be derived for enums").write_errors(),
    };

    let variant_idents = variants.iter().map(|v: &Variant| &v.ident);
    let _variant_ids = variants.iter().map(|v: &Variant| {
        let name = v.ident.to_string().to_lowercase();
        quote! { #name }
    });

    // default_choices
    let default_choices = quote! { vec![ #(Self::#variant_idents),* ] };

    // from_str
    let from_str_match_arms = variants.iter().map(|v| {
        let ident = &v.ident;
        let name = ident.to_string().to_lowercase();
        quote! { #name => Ok(Self::#ident), }
    });

    // id
    let id_match_arms = variants.iter().map(|v| {
        let ident = &v.ident;
        let name = ident.to_string().to_lowercase();
        quote! { Self::#ident => #name, }
    });

    // to_string
    let to_string_match_arms = variants.iter().map(|v| {
        let ident = &v.ident;
        let name = ident.to_string();
        quote! { Self::#ident => #name, }
    });

    quote! {
        #[automatically_derived]
        impl #cot::form::fields::SelectChoice for #enum_name {
            fn default_choices() -> Vec<Self> {
                #default_choices
            }
            fn from_str(s: &str) -> Result<Self, #cot::form::FormFieldValidationError> {
                match s {
                    #( #from_str_match_arms )*
                    _ => Err(#cot::form::FormFieldValidationError::invalid_value(s.to_owned())),
                }
            }
            fn id(&self) -> String {
                match self {
                    #( #id_match_arms )*
                }.to_string()
            }
            fn to_string(&self) -> String {
                match self {
                    #( #to_string_match_arms )*
                }.to_string()
            }
        }
    }
}
