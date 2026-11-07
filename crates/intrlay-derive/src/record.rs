use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Fields;
use syn::ext::IdentExt;

use crate::{
    model::{Field, Model},
    view,
};

pub fn expand(model: &Model<'_>, shape: &Fields) -> syn::Result<TokenStream> {
    let runtime = &model.runtime;
    let value = model.binding("value");
    let cursor = model.binding("cursor");
    let writer = model.binding("writer");
    let offset = model.binding("offset");
    let bytes = model.binding("bytes");
    let registry = model.binding("registry");
    let initialized_fields = model.binding("fields");
    let fields = Field::parse(shape, model)?;
    validate_fields(model, shape, &fields)?;
    let generics = model.generics(&fields);
    let (implementation, _, predicates) = generics.split_for_impl();
    let types = view::type_arguments(&generics);
    let ident = &model.input.ident;
    let view_name = &model.view;
    let source = &model.source;
    let args = view::arguments(&generics);
    let layouts: Vec<_> = fields
        .iter()
        .map(|field| {
            let codec = &field.codec;
            quote!(<#codec as #runtime::Intrlay>::LAYOUT)
        })
        .collect();
    let alignment = model.options.alignment;
    let native = native_layout(model, &fields);
    let (payload, in_place) = replacement_properties(model, &fields);
    let fixed = fields.iter().map(|field| {
        let codec = &field.codec;
        quote!(<#codec as #runtime::Intrlay>::WRITES_FIXED_LAYOUT)
    });
    let sizes = layouts.iter().map(|layout| quote!((#layout).size()));
    let initialization = fields.iter().map(|field| {
        let codec = &field.codec;
        quote!((<#codec as #runtime::Intrlay>::LAYOUT, <#codec as #runtime::Intrlay>::INITIALIZATION))
    });
    let measure = fields.iter().map(|field| {
        let codec = &field.codec;
        let access = field.access(model);
        quote!(<#codec as #runtime::Intrlay>::measure(#access, #cursor)?;)
    });
    let write = fields.iter().enumerate().map(|(index, field)| {
        let codec = &field.codec;
        let access = field.access(model);
        quote!(<#codec as #runtime::Intrlay>::write(#access, #writer, #offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) })?;)
    });
    let decode = decode(model, shape, &fields, &layouts)?;
    let (getters, mutators) = accessors(model, &fields, &layouts);
    let declaration = view::declaration(model, &generics, &getters, &mutators);
    let metadata = metadata(model, &fields);
    Ok(quote! {
        #declaration
        #[automatically_derived]
        impl #implementation #runtime::schema::Describe for #ident #types #predicates {
            fn describe(#registry: &mut #runtime::schema::Registry) -> #runtime::schema::Type { #metadata }
        }
        // SAFETY: Field codecs determine every offset and access. Structural metadata is immutable.
        #[automatically_derived]
        unsafe impl #implementation #runtime::Intrlay for #ident #types #predicates {
            type Value = #source;
            type View<'__intrlay> = #view_name<&'__intrlay [::core::primitive::u8], #(#args),*>;
            type ViewMut<'__intrlay> = #view_name<&'__intrlay mut [::core::primitive::u8], #(#args),*>;
            const LAYOUT: #runtime::layout::Layout = #runtime::layout::record(&[#(#layouts),*], #alignment);
            const NATIVE_LAYOUT: ::core::primitive::bool = #native;
            const HAS_PAYLOAD: ::core::primitive::bool =  #payload;
            const IN_PLACE_WRITE: ::core::primitive::bool = #in_place;
            const WRITES_FIXED_LAYOUT: ::core::primitive::bool = true #(&& #fixed)*
                && <Self as #runtime::Intrlay>::LAYOUT.size() == 0usize #(+ #sizes)*;
            const INITIALIZATION: #runtime::Initialization = {
                let #initialized_fields = [#(#initialization),*];
                #runtime::Initialization::record(<Self as #runtime::Intrlay>::LAYOUT, &#initialized_fields)
            };
            fn measure(#value: &Self::Value, #cursor: &mut #runtime::SizeCursor) -> ::core::result::Result<(), #runtime::EncodeError> {
                #(#measure)*
                ::core::result::Result::Ok(())
            }
            fn write(#value: &Self::Value, #writer: &mut #runtime::Writer<'_>, #offset: ::core::primitive::usize) -> ::core::result::Result<(), #runtime::EncodeError> {
                if <Self as #runtime::Intrlay>::NATIVE_LAYOUT {
                    // SAFETY: The const proof excludes padding and checks every native field offset.
                    unsafe { #runtime::__private::write_native(#value, #writer, #offset); }
                } else { #(#write)* }
                ::core::result::Result::Ok(())
            }
            #[inline]
            unsafe fn view(#bytes: &[::core::primitive::u8], #offset: ::core::primitive::usize) -> Self::View<'_> {
                // SAFETY: The caller guarantees a complete valid encoding at this offset.
                unsafe { #view_name::from_parts_unchecked(#bytes, #offset) }
            }
            #[inline]
            unsafe fn view_mut(#bytes: &mut [::core::primitive::u8], #offset: ::core::primitive::usize) -> Self::ViewMut<'_> {
                // SAFETY: The caller guarantees a complete valid encoding at this offset.
                unsafe { #view_name::from_parts_unchecked(#bytes, #offset) }
            }
            unsafe fn decode(#bytes: &[::core::primitive::u8], #offset: ::core::primitive::usize) -> Self::Value {
                if <Self as #runtime::Intrlay>::NATIVE_LAYOUT {
                    // SAFETY: The caller supplies valid encoded bytes and the native layout is identical.
                    unsafe { #runtime::__private::read_native(#bytes, #offset) }
                } else { #decode }
            }
        }
    })
}

fn validate_fields(model: &Model<'_>, shape: &Fields, fields: &[Field]) -> syn::Result<()> {
    if model.options.transparent && fields.len() != 1 {
        return Err(syn::Error::new_spanned(
            shape,
            "transparent requires exactly one field",
        ));
    }
    if model.options.remote.is_none() && fields.iter().any(|field| field.options.getter.is_some()) {
        return Err(syn::Error::new_spanned(
            shape,
            "getter requires a remote container",
        ));
    }
    Ok(())
}

fn replacement_properties(model: &Model<'_>, fields: &[Field]) -> (TokenStream, TokenStream) {
    let runtime = &model.runtime;
    let direct = model.options.from.is_none();
    let payloads = fields.iter().map(|field| {
        let codec = &field.codec;
        quote!(<#codec as #runtime::Intrlay>::HAS_PAYLOAD)
    });
    let in_place = fields.iter().map(|field| {
        let codec = &field.codec;
        let direct = field.options.getter.is_none();
        quote!(#direct && <#codec as #runtime::Intrlay>::IN_PLACE_WRITE)
    });
    (
        quote!(false #(|| #payloads)*),
        quote!(#direct #(&& (#in_place))*),
    )
}

fn native_layout(model: &Model<'_>, fields: &[Field]) -> TokenStream {
    let runtime = &model.runtime;
    if model.options.remote.is_some() {
        return quote!(false);
    }
    let parts = fields
        .iter()
        .map(|field| {
            let codec = &field.codec;
            quote!(<#codec as #runtime::Intrlay>::LAYOUT)
        })
        .collect::<Vec<_>>();
    let checks = fields.iter().enumerate().map(|(index, field)| {
        let codec = &field.codec;
        let member = &field.member;
        quote!( && <#codec as #runtime::Intrlay>::NATIVE_LAYOUT
            && ::core::mem::offset_of!(Self, #member) == #runtime::layout::field_offset(&[#(#parts),*], #index))
    });
    let sizes = fields.iter().map(|field| {
        let codec = &field.codec;
        quote!(+ <#codec as #runtime::Intrlay>::LAYOUT.size())
    });
    quote!(cfg!(target_endian = "little") && !::core::mem::needs_drop::<Self>()
        && ::core::mem::size_of::<Self>() == <Self as #runtime::Intrlay>::LAYOUT.size()
        && <Self as #runtime::Intrlay>::LAYOUT.size() == 0usize #(#sizes)* #(#checks)*)
}

fn decode(
    model: &Model<'_>,
    shape: &Fields,
    fields: &[Field],
    layouts: &[TokenStream],
) -> syn::Result<TokenStream> {
    let runtime = &model.runtime;
    let bytes = model.binding("bytes");
    let offset = model.binding("offset");
    let ident = &model.input.ident;
    let values = fields.iter().enumerate().map(|(index, field)| {
        let codec = &field.codec;
        quote!(unsafe { <#codec as #runtime::Intrlay>::decode(#bytes, #offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }) })
    }).collect::<Vec<_>>();
    let constructor = if model.options.from.is_some() {
        quote!(#ident)
    } else if model.options.remote.is_none() {
        quote!(Self)
    } else {
        let source = model.source_constructor()?;
        quote!(#source)
    };
    let result = match shape {
        Fields::Named(_) => {
            let members = fields.iter().map(|field| &field.member);
            quote!(#constructor { #(#members: #values),* })
        }
        Fields::Unnamed(_) => quote!(#constructor(#(#values),*)),
        Fields::Unit => quote!(#constructor),
    };
    Ok(if let Some(from) = &model.options.from {
        quote!(#from(#result))
    } else {
        result
    })
}

fn accessors(
    model: &Model<'_>,
    fields: &[Field],
    layouts: &[TokenStream],
) -> (TokenStream, TokenStream) {
    let runtime = &model.runtime;
    let offset = model.binding("offset");
    let mut getters = TokenStream::new();
    let mut mutators = TokenStream::new();
    for (index, field) in fields.iter().enumerate() {
        let codec = &field.codec;
        let getter = &field.getter;
        let mutable = format_ident!("{getter}_mut");
        let setter = format_ident!("replace_{}", getter.unraw());
        let value = model.binding("value");
        let docs = if field.docs.is_empty() {
            format!(
                "Reads the `{}` field without reconstructing its owned value.",
                field.name
            )
        } else {
            field.docs.clone()
        };
        let mutable_docs = crate::attributes::with_documentation(
            &field.docs,
            &format!(
                "Borrows the `{}` field for mutation without changing its layout.",
                field.name
            ),
        );
        getters.extend(quote! {
                #[doc = #docs]
                #[must_use]
                #[inline]
                pub fn #getter(&self) -> <#codec as #runtime::Intrlay>::View<'_> {
                    // SAFETY: This view retains a valid parent encoding and fields have constant offsets.
                    unsafe { <#codec as #runtime::Intrlay>::view(self.storage.as_bytes(), self.storage.byte_offset() + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }) }
                }
        });
        mutators.extend(quote! {
                /// Replaces this field when its complete encoding is fixed-size.
                #[inline]
                pub fn #setter(&mut self, #value: &<#codec as #runtime::Intrlay>::Value) -> ::core::result::Result<(), #runtime::EncodeError> {
                    let #offset = self.storage.byte_offset() + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) };
                    // SAFETY: The field is a valid encoding exclusively borrowed from its parent.
                    unsafe { #runtime::ValueView::<#codec, _>::new_unchecked(self.storage.as_bytes_mut_unchecked(), #offset) }.replace(#value)
                }
                #[doc = #mutable_docs]
                #[must_use]
                #[inline]
                pub fn #mutable(&mut self) -> <#codec as #runtime::Intrlay>::ViewMut<'_> {
                    // SAFETY: Reborrowing storage excludes other mutable field access for this lifetime.
                    unsafe { let #offset = self.storage.byte_offset(); <#codec as #runtime::Intrlay>::view_mut(self.storage.as_bytes_mut_unchecked(), #offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }) }
                }
        });
    }
    (getters, mutators)
}

fn metadata(model: &Model<'_>, fields: &[Field]) -> TokenStream {
    let runtime = &model.runtime;
    let registry = model.binding("registry");
    let transparent = model.options.transparent;
    let name = model.name();
    let docs = model.docs();
    let alignment = model.options.alignment;
    let targets = model.options.targets(runtime);
    let metadata = fields.iter().map(|field| field.metadata(model));
    quote!(#registry.define::<Self>(|#registry| #runtime::schema::Definition {
        name: #name.into(), docs: #docs.into(), alignment: #alignment, targets: #targets, transparent: #transparent,
        kind: #runtime::schema::DefinitionKind::Record(::std::vec![#(#metadata),*]),
    }))
}
