use proc_macro2::TokenStream;
use quote::quote;
use syn::{GenericParam, Generics};

use crate::{attributes::with_documentation, model::Model};

pub fn arguments(generics: &Generics) -> Vec<TokenStream> {
    generics
        .params
        .iter()
        .map(|parameter| match parameter {
            GenericParam::Type(parameter) => {
                let name = &parameter.ident;
                quote!(#name)
            }
            GenericParam::Const(parameter) => {
                let name = &parameter.ident;
                // Disambiguate values from same-named types or modules.
                quote!({ #name })
            }
            GenericParam::Lifetime(parameter) => {
                let lifetime = &parameter.lifetime;
                quote!(#lifetime)
            }
        })
        .collect()
}

pub fn type_arguments(generics: &Generics) -> TokenStream {
    if generics.params.is_empty() {
        TokenStream::new()
    } else {
        let args = arguments(generics);
        quote!(<#(#args),*>)
    }
}

pub fn declaration(
    model: &Model<'_>,
    generics: &Generics,
    getters: &TokenStream,
    mutators: &TokenStream,
) -> TokenStream {
    let runtime = &model.runtime;
    let buffer_type = model.type_binding("__IntrlayBuffer", &quote!(#generics #getters #mutators));
    let buffer = model.binding("buffer");
    let offset = model.binding("offset");
    let view_value = model.binding("view");
    let value = model.binding("value");
    let ident = &model.input.ident;
    let view = &model.view;
    let source = &model.source;
    let visibility = model.visibility();
    let docs = with_documentation(
        &model.docs(),
        "A typed view over owned or borrowed encoded storage.",
    );
    let args = arguments(generics);
    let source_types = type_arguments(generics);
    let codec = quote!(#ident #source_types);

    let mut view_generics = generics.clone();
    view_generics
        .params
        .insert(0, syn::parse_quote!(#buffer_type));
    let view_where = &view_generics.where_clause;
    let mut readable = view_generics.clone();
    readable
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(#buffer_type: ::core::convert::AsRef<[::core::primitive::u8]>));
    let (read_impl, _, read_where) = readable.split_for_impl();
    let view_types = type_arguments(&readable);
    let mut writable = readable.clone();
    writable
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(#buffer_type: ::core::convert::AsMut<[::core::primitive::u8]>));
    let (write_impl, _, write_where) = writable.split_for_impl();
    let create = construction(model, generics);
    let conversion = quote! {
            #[automatically_derived]
            impl #read_impl ::core::convert::From<#view #view_types> for #source #read_where {
                #[inline]
                fn from(#view_value: #view #view_types) -> Self {
                    // SAFETY: The view's constructor and mutation API preserve codec validity.
                    unsafe { <#codec as #runtime::Intrlay>::decode(#view_value.storage.as_bytes(), #view_value.storage.byte_offset()) }
                }
            }
    };
    let mutable = quote! {
            #[automatically_derived]
            impl #write_impl #view #view_types #write_where {
                /// Exclusively reborrows this value without copying its bytes.
                #[must_use]
                #[inline]
                pub fn as_view_mut(&mut self) -> #view<&mut [::core::primitive::u8], #(#args),*> {
                    #view { storage: self.storage.as_view_mut() }
                }
                /// Replaces a fixed encoding, including optional presence and enum tags.
                #[inline]
                pub fn replace(&mut self, #value: &<#codec as #runtime::Intrlay>::Value) -> ::core::result::Result<(), #runtime::EncodeError> {
                    self.storage.replace(#value)
                }
                #mutators
            }
    };

    quote! {
        #[doc = #docs]
        #visibility struct #view #view_generics #view_where {
            storage: #runtime::ValueView<#codec, #buffer_type>,
        }
        #[automatically_derived]
        impl #read_impl #view #view_types #read_where {
            /// Fixed header layout, including trailing padding.
            pub const LAYOUT: #runtime::layout::Layout = <#codec as #runtime::Intrlay>::LAYOUT;

            /// Opens a trusted root encoding without an object-validation pass.
            ///
            /// # Safety
            /// Storage must contain a valid complete encoding of this type, and
            /// all referenced bytes must remain valid for the view's lifetime.
            #[inline]
            pub unsafe fn from_bytes_unchecked(#buffer: #buffer_type) -> Self {
                // SAFETY: The caller guarantees the complete root encoding.
                Self { storage: unsafe { #runtime::ValueView::new_unchecked(#buffer, 0) } }
            }

            /// Opens a trusted value at a byte offset within a larger buffer.
            ///
            /// # Safety
            /// The offset must identify a complete valid encoding in this storage.
            #[inline]
            pub unsafe fn from_parts_unchecked(#buffer: #buffer_type, #offset: ::core::primitive::usize) -> Self {
                // SAFETY: The caller guarantees the complete encoding at this offset.
                Self { storage: unsafe { #runtime::ValueView::new_unchecked(#buffer, #offset) } }
            }

            /// Borrows this value without copying its representation.
            #[must_use]
            #[inline]
            pub fn as_view(&self) -> #view<&[::core::primitive::u8], #(#args),*> {
                #view { storage: self.storage.as_view() }
            }

            /// Returns the backing bytes, including referenced payloads.
            #[must_use]
            #[inline]
            pub fn as_bytes(&self) -> &[::core::primitive::u8] { self.storage.as_bytes() }

            /// Consumes the view and returns ownership of its backing storage.
            #[must_use]
            pub fn into_buffer(self) -> #buffer_type { self.storage.into_buffer() }

            #getters
        }
        #mutable
        #conversion
        #create
    }
}

fn construction(model: &Model<'_>, generics: &Generics) -> TokenStream {
    if !model.options.create() {
        return TokenStream::new();
    }
    let runtime = &model.runtime;
    let value = model.binding("value");
    let buffer = model.binding("buffer");
    let ident = &model.input.ident;
    let view = &model.view;
    let source = &model.source;
    let args = arguments(generics);
    let (implementation, _, predicates) = generics.split_for_impl();
    let types = type_arguments(generics);
    quote! {
        #[automatically_derived]
        impl #implementation #view<#runtime::OwnedBuffer, #(#args),*> #predicates {
            /// Computes exact byte length and payload alignment before allocation.
            ///
            /// # Errors
            /// Returns an error when a size or offset exceeds the supported representation.
            pub fn compute_layout(#value: &#source) -> ::core::result::Result<#runtime::BufferLayout, #runtime::EncodeError> {
                #runtime::compute_layout::<#ident #types>(#value)
            }
            /// Constructs a typed view that owns its initialized buffer.
            ///
            /// # Errors
            /// Returns an error for unsupported sizes or a layout that changes during construction.
            pub fn create(#value: &#source) -> ::core::result::Result<Self, #runtime::EncodeError> {
                ::core::result::Result::Ok(Self { storage: #runtime::ValueView::create(#value)? })
            }
            /// Initializes storage owned by the caller and returns a borrowed view.
            ///
            /// # Errors
            /// Returns an error for insufficient/misaligned storage, unsupported sizes, or a
            /// layout that changes during construction. On failure, storage may be partially written.
            pub fn create_in_place<'__intrlay>(#value: &#source, #buffer: &'__intrlay mut [::core::primitive::u8]) -> ::core::result::Result<#view<&'__intrlay mut [::core::primitive::u8], #(#args),*>, #runtime::EncodeError> {
                ::core::result::Result::Ok(#view { storage: #runtime::ValueView::create_in_place(#value, #buffer)? })
            }
        }
    }
}
