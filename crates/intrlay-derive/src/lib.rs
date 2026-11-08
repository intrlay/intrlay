//! Derives typed views, native conversions, and compiler-resolved schema metadata.
//!
//! This crate only emits Rust items. Foreign-language files are written by the
//! explicit code-generation tool, never as a procedural-macro side effect.

#![forbid(unsafe_code)]

mod attributes;
mod enumeration;
mod input;
mod model;
mod record;
mod runtime_path;
mod view;

use proc_macro::TokenStream;

/// Generates an Intrlay codec, `TypeInput` encoding sources, and `TypeView<Buffer>`.
#[proc_macro_derive(Intrlay, attributes(intrlay))]
pub fn derive_intrlay(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(input: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let model = model::Model::new(input)?;
    match &input.data {
        syn::Data::Struct(data) => record::expand(&model, &data.fields),
        syn::Data::Enum(data) => enumeration::expand(&model, data),
        syn::Data::Union(data) => Err(syn::Error::new_spanned(
            data.union_token,
            "unions require an explicit tagged representation",
        )),
    }
}
