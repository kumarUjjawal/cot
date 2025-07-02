use darling::Error;
use quote::quote;
use syn::{Data, DeriveInput};

use crate::cot_ident;

pub(super) fn impl_as_form_field_for_enum(ast: &DeriveInput) -> proc_macro2::TokenStream {
    let enum_name = &ast.ident;
    let cot = cot_ident();

    let variants = match &ast.data {
        Data::Enum(data_enum) => &data_enum.variants,
        _ => return Error::custom("`AsFormField` can only be derived for enums").write_errors(),
    };

    if variants.is_empty() {
        return Error::custom("`AsFormField` cannot be derived for empty enums").write_errors();
    }

    for variant in variants {
        if !variant.fields.is_empty() {
            return Error::custom("`AsFormField` can only be derived for enums with unit variants")
                .with_span(&variant)
                .write_errors();
        }
    }

    quote! {
        #[automatically_derived]
        impl #cot::form::AsFormField for #enum_name
        where
            Self: #cot::form::fields::SelectChoice,
        {
            type Type = #cot::form::fields::SelectField<Self>;

            fn clean_value(
                field: &Self::Type
            ) -> ::std::result::Result<Self, #cot::form::FormFieldValidationError> {
                let value = #cot::form::fields::check_required(field)?;
                <Self as #cot::form::fields::SelectChoice>::from_str(value)
            }

            fn to_field_value(&self) -> ::std::string::String {
                <Self as #cot::form::fields::SelectChoice>::to_string(self)
            }
        }
    }
}
