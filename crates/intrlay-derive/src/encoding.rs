use proc_macro2::TokenStream;
use quote::quote;

use crate::model::Model;

/// Propagates intermediate failures and returns the final result directly.
/// Keeping the last codec call in tail position avoids rebuilding a large Result
/// at every level of a recursive record, option, or enum payload.
pub fn fallible_sequence(expressions: impl IntoIterator<Item = TokenStream>) -> TokenStream {
    let expressions: Vec<_> = expressions.into_iter().collect();
    if let Some((last, preceding)) = expressions.split_last() {
        quote! { #(#preceding?;)* #last }
    } else {
        quote!(::core::result::Result::Ok(()))
    }
}

/// One field whose source may have a native-compatible representation.
pub struct NativeField {
    pub proof: TokenStream,
    /// The native type covered by the proof; getter results are coerced to it.
    pub value_type: TokenStream,
    pub value: TokenStream,
    pub offset: TokenStream,
}

/// Copies native fields through a fixed header, retaining the codec fallback.
/// Only native copies use relative offsets; custom field codecs always receive
/// the original writer and absolute buffer offsets.
pub fn write_header(
    model: &Model<'_>,
    layout: &TokenStream,
    native_fields: impl IntoIterator<Item = NativeField>,
    fields: &TokenStream,
) -> TokenStream {
    let writer = model.binding("writer");
    let offset = model.binding("offset");
    let header = model.binding("header");
    let native_fields: Vec<_> = native_fields.into_iter().collect();
    if native_fields.is_empty() {
        return fields.clone();
    }
    let proofs = native_fields.iter().map(|field| &field.proof);
    let copies = native_fields.iter().map(|field| {
        let value_type = &field.value_type;
        let value = &field.value;
        let offset = &field.offset;
        // A getter may return a reference or a Deref wrapper. Match the codec's
        // argument type instead of inferring the representation of that wrapper.
        quote!(#header.write_native::<#value_type>(#value, #offset);)
    });
    quote! {
        if true #(&& #proofs)* {
            let mut #header = #writer.fixed_header(#offset, #layout);
            // SAFETY: Every field's NATIVE_LAYOUT proof guarantees initialized
            // native bytes matching its encoding, without ownership or padding.
            unsafe { #(#copies)* }
            ::core::result::Result::Ok(())
        } else {
            #fields
        }
    }
}
