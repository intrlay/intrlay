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
    let all_bits_valid = fields.iter().map(|field| {
        let codec = &field.codec;
        quote!(<#codec as #runtime::Intrlay>::ALL_BITS_VALID)
    });
    let decode_depth = fixed_decode_depth(model, &fields);
    let (payload, in_place, zeroable) = encoding_properties(model, &fields);
    let initialization = initialization(model, &fields, &layouts);
    let measure = crate::encoding::fallible_sequence(fields.iter().map(|field| {
        let codec = &field.codec;
        let access = field.access(model);
        quote!(<#codec as #runtime::Intrlay>::measure(#access, #cursor))
    }));
    let write = write_fields(model, &fields, &layouts);
    let unchecked_decode = decode(model, shape, &fields, &layouts, false)?;
    let checked_methods = checked_codec(model, shape, &fields, &layouts)?;
    let (getters, mutators) = accessors(model, &fields, &layouts);
    let declaration = view::declaration(model, &generics, &getters, &mutators);
    let metadata = metadata(model, &fields);
    let input = crate::input::record(model, shape, &fields, &layouts)?;
    Ok(quote! {
        #declaration
        #input
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
            const ALL_BITS_VALID: ::core::primitive::bool = true #(&& #all_bits_valid)*;
            #decode_depth
            const ZEROABLE: ::core::primitive::bool = #zeroable;
            const HAS_PAYLOAD: ::core::primitive::bool =  #payload;
            const IN_PLACE_WRITE: ::core::primitive::bool = <Self as #runtime::Intrlay>::NATIVE_LAYOUT || #in_place;
            #initialization
            #[inline]
            fn measure(#value: &Self::Value, #cursor: &mut #runtime::SizeCursor) -> ::core::result::Result<(), #runtime::EncodeError> {
                #measure
            }
            fn write(#value: &Self::Value, #writer: &mut #runtime::Writer<'_>, #offset: ::core::primitive::usize) -> ::core::result::Result<(), #runtime::EncodeError> {
                if <Self as #runtime::Intrlay>::NATIVE_LAYOUT {
                    // SAFETY: NATIVE_LAYOUT follows checked offsets or the adapter's explicit unsafe proof.
                    unsafe { #runtime::__private::write_native(#value, #writer, #offset); }
                    ::core::result::Result::Ok(())
                } else { #write }
            }
            #[inline]
            unsafe fn view_unchecked(#bytes: &[::core::primitive::u8], #offset: ::core::primitive::usize) -> Self::View<'_> {
                // SAFETY: The caller guarantees a complete valid encoding at this offset.
                unsafe { #view_name::from_parts_unchecked(#bytes, #offset) }
            }
            #[inline]
            unsafe fn view_mut_unchecked(#bytes: &mut [::core::primitive::u8], #offset: ::core::primitive::usize) -> Self::ViewMut<'_> {
                // SAFETY: The caller guarantees a complete valid encoding at this offset.
                unsafe { #view_name::from_parts_unchecked(#bytes, #offset) }
            }
            #checked_methods
            unsafe fn decode_unchecked(#bytes: &[::core::primitive::u8], #offset: ::core::primitive::usize) -> Self::Value {
                if <Self as #runtime::Intrlay>::NATIVE_LAYOUT {
                    // SAFETY: The caller supplies valid encoded bytes and the native layout is identical.
                    unsafe { #runtime::__private::read_native(#bytes, #offset) }
                } else { #unchecked_decode }
            }
        }
    })
}

fn write_fields(model: &Model<'_>, fields: &[Field], layouts: &[TokenStream]) -> TokenStream {
    let runtime = &model.runtime;
    let writer = model.binding("writer");
    let offset = model.binding("offset");
    let write = crate::encoding::fallible_sequence(fields.iter().enumerate().map(|(index, field)| {
        let codec = &field.codec;
        let access = field.access(model);
        quote!(<#codec as #runtime::Intrlay>::write(#access, #writer, #offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }))
    }));
    crate::encoding::write_header(
        model,
        &quote!(<Self as #runtime::Intrlay>::LAYOUT),
        fields.iter().enumerate().map(|(index, field)| {
            let codec = &field.codec;
            crate::encoding::NativeField {
                proof: quote!(<#codec as #runtime::Intrlay>::NATIVE_LAYOUT),
                value_type: quote!(<#codec as #runtime::Intrlay>::Value),
                value: field.access(model),
                offset: quote!(const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }),
            }
        }),
        &write,
    )
}

fn fixed_decode_depth(model: &Model<'_>, fields: &[Field]) -> TokenStream {
    let runtime = &model.runtime;
    let depths = fields.iter().map(|field| {
        let codec = &field.codec;
        quote!(<#codec as #runtime::Intrlay>::FIXED_DECODE_DEPTH)
    });
    quote! {
            const FIXED_DECODE_DEPTH: ::core::option::Option<::core::primitive::usize> =
                if <Self as #runtime::Intrlay>::NATIVE_LAYOUT && <Self as #runtime::Intrlay>::ALL_BITS_VALID {
                    ::core::option::Option::Some(1)
                } else if <Self as #runtime::Intrlay>::ALL_BITS_VALID {
                    #runtime::__private::fixed_decode_depth(&[#(#depths),*])
                } else {
                    ::core::option::Option::None
                };
    }
}

fn initialization(model: &Model<'_>, fields: &[Field], layouts: &[TokenStream]) -> TokenStream {
    let runtime = &model.runtime;
    let initialized_fields = model.binding("fields");
    let fixed = fields.iter().map(|field| {
        let codec = &field.codec;
        quote!(<#codec as #runtime::Intrlay>::WRITES_FIXED_LAYOUT)
    });
    let sizes = layouts.iter().map(|layout| quote!((#layout).size()));
    let initialization = fields.iter().map(|field| {
        let codec = &field.codec;
        quote!((<#codec as #runtime::Intrlay>::LAYOUT, <#codec as #runtime::Intrlay>::INITIALIZATION))
    });
    quote! {
            const WRITES_FIXED_LAYOUT: ::core::primitive::bool = <Self as #runtime::Intrlay>::NATIVE_LAYOUT
                || (true #(&& #fixed)* && <Self as #runtime::Intrlay>::LAYOUT.size() == 0usize #(+ #sizes)*);
            const INITIALIZATION: #runtime::Initialization = {
                let #initialized_fields = [#(#initialization),*];
                if <Self as #runtime::Intrlay>::NATIVE_LAYOUT {
                    #runtime::Initialization::FIXED
                } else {
                    #runtime::Initialization::record(<Self as #runtime::Intrlay>::LAYOUT, &#initialized_fields)
                }
            };
    }
}

fn checked_codec(
    model: &Model<'_>,
    shape: &Fields,
    fields: &[Field],
    layouts: &[TokenStream],
) -> syn::Result<TokenStream> {
    let runtime = &model.runtime;
    let offset = model.binding("offset");
    let decoder = model.binding("decoder");
    let validator = model.binding("validator");
    let checked_decode = decode(model, shape, fields, layouts, true)?;
    let validate = fields.iter().enumerate().map(|(index, field)| {
        let codec = &field.codec;
        quote!(#validator.field::<#codec>(#offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) })?;)
    });
    Ok(quote! {
            #[inline]
            fn validate_fields(#validator: &mut #runtime::Validator<'_>, #offset: ::core::primitive::usize) -> ::core::result::Result<(), #runtime::DecodeError> {
                #(#validate)*
                ::core::result::Result::Ok(())
            }
            #[inline]
            fn decode_from(#decoder: &mut #runtime::Decoder<'_>, #offset: ::core::primitive::usize) -> ::core::result::Result<Self::Value, #runtime::DecodeError> {
                #decoder.range(#offset, <Self as #runtime::Intrlay>::LAYOUT.size())?;
                ::core::result::Result::Ok(#checked_decode)
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

fn encoding_properties(
    model: &Model<'_>,
    fields: &[Field],
) -> (TokenStream, TokenStream, TokenStream) {
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
    let zeroable = fields.iter().map(|field| {
        let codec = &field.codec;
        quote!(<#codec as #runtime::Intrlay>::ZEROABLE)
    });
    (
        quote!(false #(|| #payloads)*),
        quote!(#direct #(&& (#in_place))*),
        quote!(true #(&& #zeroable)*),
    )
}

fn native_layout(model: &Model<'_>, fields: &[Field]) -> TokenStream {
    let runtime = &model.runtime;
    let source = &model.source;
    let compatible = quote!(cfg!(target_endian = "little")
        && !::core::mem::needs_drop::<#source>()
        && ::core::mem::size_of::<#source>() == <Self as #runtime::Intrlay>::LAYOUT.size()
        && !<Self as #runtime::Intrlay>::HAS_PAYLOAD);
    if let Some(proof) = &model.options.native_layout {
        // The unsafe annotation guarantees initialized padding, field semantics,
        // and decode validity. Mechanical checks still gate the selected target.
        return quote!(#compatible && (#proof));
    }
    if model.options.from.is_some() || fields.iter().any(|field| field.options.getter.is_some()) {
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
            && ::core::mem::offset_of!(#source, #member) == #runtime::layout::field_offset(&[#(#parts),*], #index))
    });
    let sizes = fields.iter().map(|field| {
        let codec = &field.codec;
        quote!(+ <#codec as #runtime::Intrlay>::LAYOUT.size())
    });
    quote!(#compatible && <Self as #runtime::Intrlay>::LAYOUT.size() == 0usize #(#sizes)* #(#checks)*)
}

fn decode(
    model: &Model<'_>,
    shape: &Fields,
    fields: &[Field],
    layouts: &[TokenStream],
    checked: bool,
) -> syn::Result<TokenStream> {
    let runtime = &model.runtime;
    let bytes = model.binding("bytes");
    let offset = model.binding("offset");
    let decoder = model.binding("decoder");
    let ident = &model.input.ident;
    let values = fields.iter().enumerate().map(|(index, field)| {
        let codec = &field.codec;
        let position = quote!(#offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) });
        if checked {
            quote!(#decoder.decode::<#codec>(#position)?)
        } else {
            quote!(unsafe { <#codec as #runtime::Intrlay>::decode_unchecked(#bytes, #position) })
        }
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
                    unsafe { <#codec as #runtime::Intrlay>::view_unchecked(self.storage.as_bytes(), self.storage.byte_offset() + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }) }
                }
        });
        mutators.extend(quote! {
                /// Replaces this field when its complete encoding is fixed-size.
                #[inline]
                pub fn #setter(&mut self, #value: &<#codec as #runtime::Intrlay>::Value) -> ::core::result::Result<(), #runtime::EncodeError> {
                    let #offset = self.storage.byte_offset() + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) };
                    // SAFETY: The field is a valid encoding exclusively borrowed from its parent.
                    unsafe { #runtime::ValueView::<#codec, _>::from_parts_unchecked(self.storage.as_bytes_mut_unchecked(), #offset) }.replace(#value)
                }
                #[doc = #mutable_docs]
                #[must_use]
                #[inline]
                pub fn #mutable(&mut self) -> <#codec as #runtime::Intrlay>::ViewMut<'_> {
                    // SAFETY: Reborrowing storage excludes other mutable field access for this lifetime.
                    unsafe { let #offset = self.storage.byte_offset(); <#codec as #runtime::Intrlay>::view_mut_unchecked(self.storage.as_bytes_mut_unchecked(), #offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }) }
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
