use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::{DataEnum, Fields, GenericParam, Generics};

use crate::{
    attributes::{Attributes, Scope, documentation},
    model::{Field, Model},
    view,
};

struct Variant<'a> {
    source: &'a syn::Variant,
    fields: Vec<Field>,
    tag: TokenStream,
    options: Attributes,
}

impl Variant<'_> {
    fn view_name(&self) -> syn::Ident {
        self.options
            .rust_name()
            .and_then(|name| syn::parse_str(name).ok())
            .unwrap_or_else(|| self.source.ident.clone())
    }
    fn layouts(&self, runtime: &syn::Path) -> Vec<TokenStream> {
        self.fields
            .iter()
            .map(|field| {
                let codec = &field.codec;
                quote!(<#codec as #runtime::Intrlay>::LAYOUT)
            })
            .collect()
    }
    fn layout(&self, runtime: &syn::Path) -> TokenStream {
        let fields = self.layouts(runtime);
        quote!(#runtime::layout::record(&[#(#fields),*], 1))
    }
    fn pattern(&self, model: &Model<'_>, constructor: &TokenStream) -> TokenStream {
        let name = &self.source.ident;
        let bindings = (0..self.fields.len())
            .map(|i| model.binding(&format!("__field_{i}")))
            .collect::<Vec<_>>();
        match &self.source.fields {
            Fields::Named(_) => {
                let members = self.fields.iter().map(|field| &field.member);
                quote!(#constructor::#name { #(#members: #bindings),* })
            }
            Fields::Unnamed(_) => quote!(#constructor::#name(#(#bindings),*)),
            Fields::Unit => quote!(#constructor::#name),
        }
    }
}

pub fn expand(model: &Model<'_>, data: &DataEnum) -> syn::Result<TokenStream> {
    let tag_value = model.binding("tag");
    if model.options.transparent || model.options.remote.is_some() || model.options.from.is_some() {
        return Err(syn::Error::new_spanned(
            &model.input.ident,
            "transparent and remote adapters require a struct",
        ));
    }
    let mut variants = Vec::with_capacity(data.variants.len());
    let discriminant_type = discriminant_type(model)?;
    let mut next_tag = quote!(0u32);
    for source in &data.variants {
        let options = Attributes::parse(&source.attrs, Scope::Variant)?;
        if let Some(name) = options.rust_name() {
            syn::parse_str::<syn::Ident>(name)?;
        }
        let tag = if let Some(value) = &options.tag {
            let value = model.resolve_expression(value);
            quote!(#value)
        } else if let Some((_, value)) = &source.discriminant {
            let value = model.resolve_expression(value);
            // Preserve the source enum's integer type while evaluating its expression.
            // Conversion happens in a const block so no tag checks enter the data path.
            quote!(const {
                let #tag_value: #discriminant_type = #value;
                assert!(
                    (#tag_value as ::core::primitive::u128) <= (::core::primitive::u32::MAX as ::core::primitive::u128),
                    "Intrlay enum tag must fit u32"
                );
                #tag_value as ::core::primitive::u32
            })
        } else {
            next_tag.clone()
        };
        next_tag = quote!((#tag) + 1u32);
        variants.push(Variant {
            source,
            fields: {
                let fields = Field::parse(&source.fields, model)?;
                for field in &fields {
                    if let Some(getter) = &field.options.getter {
                        return Err(syn::Error::new_spanned(
                            getter,
                            "getter requires a remote struct adapter",
                        ));
                    }
                }
                fields
            },
            tag,
            options,
        });
    }
    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            &model.input.ident,
            "uninhabited enums cannot be exchanged",
        ));
    }
    let fields = data
        .variants
        .iter()
        .map(|variant| Field::parse(&variant.fields, model))
        .collect::<syn::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let generics = model.generics(&fields);
    let path = model.source_constructor()?;
    let validation = tag_validation(model, &variants);
    // Non-generic declarations can be checked immediately. Generic checks live
    // in LAYOUT and are evaluated for each concrete codec, never per message.
    let (tag_check, layout_validation) = if generics.params.is_empty() {
        (quote!(const _: () = { #validation };), TokenStream::new())
    } else {
        let source = &model.source;
        let runtime = &model.runtime;
        for variant in &mut variants {
            let tag = &variant.tag;
            variant.tag = quote!(const {
                let _ = <#source as #runtime::Intrlay>::LAYOUT;
                #tag
            });
        }
        (TokenStream::new(), validation)
    };
    let declaration = views(model, &generics, &variants, !fields.is_empty());
    let implementation = codec(
        model,
        &generics,
        &variants,
        &quote!(#path),
        &layout_validation,
    );
    let input = input(model, &generics, &fields, &variants)?;
    Ok(quote!(#tag_check #declaration #implementation #input))
}

fn input(
    model: &Model<'_>,
    generics: &Generics,
    fields: &[Field],
    variants: &[Variant<'_>],
) -> syn::Result<TokenStream> {
    let name = crate::input::name(model)?;
    let vis = model.visibility();
    let source = &model.source;
    let owned = crate::input::owned(model, generics);
    if fields.is_empty() {
        let alias_where = &generics.where_clause;
        return Ok(quote! {
            /// Payload-free enum inputs use the schema's native enum directly.
            #[allow(type_alias_bounds, reason = "The alias preserves the schema declaration's resolved generic bounds")]
            #vis type #name #generics #alias_where = #source;
            #owned
        });
    }
    let runtime = &model.runtime;
    let parameters = crate::input::parameters(model, fields);
    let source_generics = crate::input::generics(model, fields, &parameters);
    let (implementation, _, predicates) = source_generics.split_for_impl();
    let cursor = model.binding("cursor");
    let writer = model.binding("writer");
    let offset = model.binding("offset");
    let payloads = variants
        .iter()
        .map(|variant| variant.layout(runtime))
        .collect::<Vec<_>>();
    let payload_offset = quote!(const { #runtime::layout::field_offset(&[#runtime::layout::Layout::new(4, 4), #runtime::layout::union(&[#(#payloads),*])], 1) });
    let mut first = 0;
    let declarations = variants
        .iter()
        .map(|variant| {
            let variant_name = &variant.source.ident;
            let docs = documentation(&variant.source.attrs);
            let end = first + variant.fields.len();
            let fields = crate::input::fields(
                &variant.source.fields,
                &variant.fields,
                &parameters[first..end],
            );
            first = end;
            quote!(#[doc = #docs] #variant_name #fields)
        })
        .collect::<Vec<_>>();
    let measure = variants.iter().map(|variant| {
        let pattern = variant.pattern(model, &quote!(Self));
        let fields = variant.fields.iter().enumerate().map(|(index, field)| {
            let binding = model.binding(&format!("__field_{index}"));
            let codec = &field.codec;
            quote!(#runtime::EncodeSource::<#codec>::measure(#binding, #cursor)?;)
        });
        quote!(#pattern => { #(#fields)* })
    });
    let write = variants.iter().map(|variant| {
        let pattern = variant.pattern(model, &quote!(Self));
        let tag = &variant.tag;
        let layouts = variant.layouts(runtime);
        let fields = variant.fields.iter().enumerate().map(|(index, field)| {
            let binding = model.binding(&format!("__field_{index}"));
            let codec = &field.codec;
            quote!(#runtime::EncodeSource::<#codec>::write(#binding, #writer, #offset + #payload_offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) })?;)
        });
        quote!(#pattern => {
            #writer.write(#offset, &(#tag as ::core::primitive::u32).to_le_bytes());
            #(#fields)*
        })
    });
    let implementation_tokens = crate::input::source_implementation(
        model,
        &name,
        &parameters,
        |name| {
            quote! {
                // SAFETY: Tags and payload offsets are identical to the owned codec;
                // inactive bytes are initialized by the construction path.
                #[automatically_derived]
                unsafe impl #implementation #runtime::EncodeSource<#source> for #name<#(#parameters),*> #predicates {
                    fn measure(&self, #cursor: &mut #runtime::SizeCursor) -> ::core::result::Result<(), #runtime::EncodeError> {
                        match self { #(#measure),* }
                        ::core::result::Result::Ok(())
                    }
                    fn write(&self, #writer: &mut #runtime::Writer<'_>, #offset: ::core::primitive::usize) -> ::core::result::Result<(), #runtime::EncodeError> {
                        match self { #(#write),* }
                        ::core::result::Result::Ok(())
                    }
                }
            }
        },
    );
    Ok(quote! {
        /// Encoding input whose active payload can borrow existing storage.
        /// Type parameters select each variant field's source representation.
        #[derive(Clone, Copy, Debug)]
        #[allow(dead_code, non_snake_case, reason = "Generated input fields and variants preserve source names and are independently usable") ]
        #vis enum #name<#(#parameters),*> { #(#declarations),* }
        #implementation_tokens
        #owned
    })
}

fn tag_validation(model: &Model<'_>, variants: &[Variant<'_>]) -> TokenStream {
    let tag_values = model.binding("tags");
    let i = model.binding("i");
    let j = model.binding("j");
    let tags = variants.iter().map(|variant| &variant.tag);
    quote! {
        let #tag_values: &[::core::primitive::u32] = &[#(#tags),*];
        let mut #i = 0;
        while #i < #tag_values.len() {
            let mut #j = #i + 1;
            while #j < #tag_values.len() {
                assert!(#tag_values[#i] != #tag_values[#j], "duplicate Intrlay enum tag");
                #j += 1;
            }
            #i += 1;
        }
    }
}

fn discriminant_type(model: &Model<'_>) -> syn::Result<TokenStream> {
    let mut integer = syn::parse_quote!(isize);
    for attribute in &model.input.attrs {
        if attribute.path().is_ident("repr") {
            let options = attribute.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )?;
            for option in options {
                if let syn::Meta::Path(path) = option
                    && let Some(ident) = path.get_ident()
                    && matches!(
                        ident.to_string().as_str(),
                        "u8" | "u16"
                            | "u32"
                            | "u64"
                            | "u128"
                            | "usize"
                            | "i8"
                            | "i16"
                            | "i32"
                            | "i64"
                            | "i128"
                            | "isize"
                    )
                {
                    integer = ident.clone();
                }
            }
        }
    }
    Ok(quote!(::core::primitive::#integer))
}

fn codec(
    model: &Model<'_>,
    generics: &Generics,
    variants: &[Variant<'_>],
    constructor: &TokenStream,
    validation: &TokenStream,
) -> TokenStream {
    let runtime = &model.runtime;
    let value = model.binding("value");
    let cursor = model.binding("cursor");
    let writer = model.binding("writer");
    let offset = model.binding("offset");
    let bytes = model.binding("bytes");
    let registry = model.binding("registry");
    let tag_value = model.binding("tag");
    let ident = &model.input.ident;
    let view_name = &model.view;
    let source = &model.source;
    let args = view::arguments(generics);
    let (implementation, _, predicates) = generics.split_for_impl();
    let types = view::type_arguments(generics);
    let payloads = variants
        .iter()
        .map(|variant| variant.layout(runtime))
        .collect::<Vec<_>>();
    let payload_offset = quote!(const { #runtime::layout::field_offset(&[#runtime::layout::Layout::new(4, 4), #runtime::layout::union(&[#(#payloads),*])], 1) });
    let alignment = model.options.alignment;
    let measure = variants.iter().map(|variant| {
        let pattern = variant.pattern(model, constructor);
        let fields = variant.fields.iter().enumerate().map(|(index, field)| {
            let binding = model.binding(&format!("__field_{index}"));
            let codec = &field.codec;
            quote!(<#codec as #runtime::Intrlay>::measure(#binding, #cursor)?;)
        });
        quote!(#pattern => { #(#fields)* })
    });
    let write = variants.iter().map(|variant| {
        let pattern = variant.pattern(model, constructor);
        let tag = &variant.tag;
        let layouts = variant.layouts(runtime);
        let fields = variant.fields.iter().enumerate().map(|(index, field)| {
            let binding = model.binding(&format!("__field_{index}"));
            let codec = &field.codec;
            quote!(<#codec as #runtime::Intrlay>::write(#binding, #writer, #offset + #payload_offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) })?;)
        });
        quote!(#pattern => { let #tag_value: ::core::primitive::u32 = #tag; #writer.write(#offset, &#tag_value.to_le_bytes()); #(#fields)* })
    });
    let decode = decode_arms(model, variants, &payload_offset, constructor);
    let metadata = metadata(model, variants);
    let has_payload = variants
        .iter()
        .flat_map(|variant| &variant.fields)
        .map(|field| {
            let codec = &field.codec;
            quote!(<#codec as #runtime::Intrlay>::HAS_PAYLOAD)
        });
    let in_place = variants
        .iter()
        .flat_map(|variant| &variant.fields)
        .map(|field| {
            let codec = &field.codec;
            quote!(<#codec as #runtime::Intrlay>::IN_PLACE_WRITE)
        });
    quote! {
        #[automatically_derived]
        impl #implementation #runtime::schema::Describe for #ident #types #predicates {
            fn describe(#registry: &mut #runtime::schema::Registry) -> #runtime::schema::Type { #metadata }
        }
        #[automatically_derived]
        unsafe impl #implementation #runtime::Intrlay for #ident #types #predicates {
            type Value = #source;
            type View<'__intrlay> = #view_name<&'__intrlay [::core::primitive::u8], #(#args),*>;
            type ViewMut<'__intrlay> = #view_name<&'__intrlay mut [::core::primitive::u8], #(#args),*>;
            const HAS_PAYLOAD: ::core::primitive::bool = false #(|| #has_payload)*;
            const IN_PLACE_WRITE: ::core::primitive::bool = true #(&& #in_place)*;
            const LAYOUT: #runtime::layout::Layout = {
                #validation
                #runtime::layout::record(&[
                    #runtime::layout::Layout::new(4, 4), #runtime::layout::union(&[#(#payloads),*])
                ], #alignment)
            };
            fn measure(#value: &<Self as #runtime::Intrlay>::Value, #cursor: &mut #runtime::SizeCursor) -> ::core::result::Result<(), #runtime::EncodeError> {
                match #value { #(#measure),* }
                ::core::result::Result::Ok(())
            }
            fn write(#value: &<Self as #runtime::Intrlay>::Value, #writer: &mut #runtime::Writer<'_>, #offset: ::core::primitive::usize) -> ::core::result::Result<(), #runtime::EncodeError> {
                match #value { #(#write),* }
                ::core::result::Result::Ok(())
            }
            unsafe fn view(#bytes: &[::core::primitive::u8], #offset: ::core::primitive::usize) -> <Self as #runtime::Intrlay>::View<'_> {
                // SAFETY: The caller guarantees a complete valid encoding at this offset.
                unsafe { #view_name::from_parts_unchecked(#bytes, #offset) }
            }
            unsafe fn view_mut(#bytes: &mut [::core::primitive::u8], #offset: ::core::primitive::usize) -> <Self as #runtime::Intrlay>::ViewMut<'_> {
                // SAFETY: The caller guarantees a complete valid encoding at this offset.
                unsafe { #view_name::from_parts_unchecked(#bytes, #offset) }
            }
            unsafe fn decode(#bytes: &[::core::primitive::u8], #offset: ::core::primitive::usize) -> <Self as #runtime::Intrlay>::Value {
                match <::core::primitive::u32 as #runtime::Scalar>::read(#bytes, #offset) {
                    #decode
                    _ => unreachable!("invalid enum tag in a trusted Intrlay buffer"),
                }
            }
        }
    }
}

fn decode_arms(
    model: &Model<'_>,
    variants: &[Variant<'_>],
    payload_offset: &TokenStream,
    constructor: &TokenStream,
) -> TokenStream {
    let runtime = &model.runtime;
    let bytes = model.binding("bytes");
    let offset = model.binding("offset");
    let matched_tag = model.binding("__tag");
    let arms = variants.iter().map(|variant| {
        let name = &variant.source.ident;
        let tag = &variant.tag;
        let layouts = variant.layouts(runtime);
        let values = variant.fields.iter().enumerate().map(|(index, field)| {
            let codec = &field.codec;
            quote!(unsafe { <#codec as #runtime::Intrlay>::decode(#bytes, #offset + #payload_offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }) })
        }).collect::<Vec<_>>();
        let result = match &variant.source.fields {
            Fields::Named(_) => { let members = variant.fields.iter().map(|field| &field.member); quote!(#constructor::#name { #(#members: #values),* }) }
            Fields::Unnamed(_) => quote!(#constructor::#name(#(#values),*)),
            Fields::Unit => quote!(#constructor::#name),
        };
        quote!(#matched_tag if #matched_tag == #tag => #result,)
    });
    quote!(#(#arms)*)
}

fn views(
    model: &Model<'_>,
    generics: &Generics,
    variants: &[Variant<'_>],
    has_fields: bool,
) -> TokenStream {
    let runtime = &model.runtime;
    let bytes = model.binding("bytes");
    let offset = model.binding("offset");
    let ident = &model.input.ident;
    let types = view::type_arguments(generics);
    let payloads = variants
        .iter()
        .map(|variant| variant.layout(runtime))
        .collect::<Vec<_>>();
    let payload_offset = quote!(const { #runtime::layout::field_offset(&[#runtime::layout::Layout::new(4, 4), #runtime::layout::union(&[#(#payloads),*])], 1) });
    let variant_name = format_ident!("{ident}VariantView");
    let mut variant_generics = generics.clone();
    variant_generics
        .params
        .insert(0, GenericParam::Lifetime(syn::parse_quote!('__intrlay)));
    let variant_where = &variant_generics.where_clause;
    let args = view::arguments(generics);
    let visibility = model.visibility();
    let definitions = variants.iter().map(|variant| {
        let name = variant.view_name();
        let docs = documentation(&variant.source.attrs);
        let fields = variant
            .fields
            .iter()
            .map(|field| {
                let codec = &field.codec;
                quote!(<#codec as #runtime::Intrlay>::View<'__intrlay>)
            })
            .collect::<Vec<_>>();
        match &variant.source.fields {
            Fields::Named(_) => {
                let members = variant.fields.iter().zip(&fields).map(|(field, ty)| {
                    let name = &field.member;
                    let docs = &field.docs;
                    quote!(#[doc = #docs] #name: #ty)
                });
                quote!(#[doc = #docs] #name { #(#members),* })
            }
            Fields::Unnamed(_) => {
                let members = variant.fields.iter().zip(&fields).map(|(field, ty)| {
                    let docs = &field.docs;
                    quote!(#[doc = #docs] #ty)
                });
                quote!(#[doc = #docs] #name(#(#members),*))
            }
            Fields::Unit => quote!(#[doc = #docs] #name),
        }
    });
    let variant_declaration = if has_fields {
        let docs = crate::attributes::with_documentation(
            &model.docs(),
            "Borrowed payloads of the active encoded variant.",
        );
        quote!(#[doc = #docs]
            #visibility enum #variant_name #variant_generics #variant_where { #(#definitions),* })
    } else {
        TokenStream::new()
    };
    let getters = if has_fields {
        let arms = variant_read_arms(model, variants, &variant_name, &payload_offset);
        quote! {
            /// Borrows the active variant without constructing owned payload values.
            ///
            /// # Panics
            /// Panics if an unsafe caller violated the encoded tag contract.
            #[must_use]
            pub fn variant(&self) -> #variant_name<'_, #(#args),*> {
                let #bytes = self.storage.as_bytes();
                let #offset = self.storage.byte_offset();
                match <::core::primitive::u32 as #runtime::Scalar>::read(#bytes, #offset) {
                    #arms
                    _ => unreachable!("invalid enum tag in a trusted Intrlay buffer"),
                }
            }
        }
    } else {
        let source = &model.source;
        quote! {
            /// Returns the active unit variant.
            #[must_use]
            pub fn variant(&self) -> #source {
                unsafe { <#ident #types as #runtime::Intrlay>::decode(self.storage.as_bytes(), self.storage.byte_offset()) }
            }
        }
    };
    let mutators = variant_mutators(model, variants, &payload_offset);
    let view_declaration = view::declaration(model, generics, &getters, &mutators);
    quote!(#variant_declaration #view_declaration)
}

fn variant_read_arms(
    model: &Model<'_>,
    variants: &[Variant<'_>],
    variant_name: &syn::Ident,
    payload_offset: &TokenStream,
) -> TokenStream {
    let runtime = &model.runtime;
    let bytes = model.binding("bytes");
    let offset = model.binding("offset");
    let matched_tag = model.binding("__tag");
    let arms = variants.iter().map(|variant| {
        let name = variant.view_name();
        let tag = &variant.tag;
        let layouts = variant.layouts(runtime);
        let values = variant.fields.iter().enumerate().map(|(index, field)| {
            let codec = &field.codec;
            quote!(unsafe { <#codec as #runtime::Intrlay>::view(#bytes, #offset + #payload_offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }) })
        }).collect::<Vec<_>>();
        let result = match &variant.source.fields {
            Fields::Named(_) => { let names = variant.fields.iter().map(|field| &field.member); quote!(#variant_name::#name { #(#names: #values),* }) }
            Fields::Unnamed(_) => quote!(#variant_name::#name(#(#values),*)),
            Fields::Unit => quote!(#variant_name::#name),
        };
        quote!(#matched_tag if #matched_tag == #tag => #result,)
    });
    quote!(#(#arms)*)
}

fn variant_mutators(
    model: &Model<'_>,
    variants: &[Variant<'_>],
    payload_offset: &TokenStream,
) -> TokenStream {
    let runtime = &model.runtime;
    let offset = model.binding("offset");
    let mut result = TokenStream::new();
    let value = model.binding("value");
    for variant in variants {
        let tag = &variant.tag;
        let layouts = variant.layouts(runtime);
        for (index, field) in variant.fields.iter().enumerate() {
            let codec = &field.codec;
            let setter = format_ident!(
                "replace_{}_{}",
                heck::ToSnakeCase::to_snake_case(variant.view_name().unraw().to_string().as_str()),
                field.getter.unraw()
            );
            let method = format_ident!(
                "{}_{}_mut",
                heck::ToSnakeCase::to_snake_case(variant.view_name().unraw().to_string().as_str()),
                field.getter
            );
            let docs = crate::attributes::with_documentation(
                &field.docs,
                &format!(
                    "Borrows `{}` from the `{}` variant when it is active.",
                    field.name, variant.source.ident
                ),
            );
            result.extend(quote! {
                /// Replaces a fixed-size variant field; returns false if this variant is inactive.
                pub fn #setter(&mut self, #value: &<#codec as #runtime::Intrlay>::Value) -> ::core::result::Result<::core::primitive::bool, #runtime::EncodeError> {
                    if <::core::primitive::u32 as #runtime::Scalar>::read(self.storage.as_bytes(), self.storage.byte_offset()) != #tag { return ::core::result::Result::Ok(false); }
                    let #offset = self.storage.byte_offset() + #payload_offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) };
                    // SAFETY: The active variant guarantees an initialized field at this offset.
                    unsafe { #runtime::ValueView::<#codec, _>::new_unchecked(self.storage.as_bytes_mut_unchecked(), #offset) }.replace(#value)?;
                    ::core::result::Result::Ok(true)
                }
                #[doc = #docs]
                #[must_use]
                pub fn #method(&mut self) -> ::core::option::Option<<#codec as #runtime::Intrlay>::ViewMut<'_>> {
                    if <::core::primitive::u32 as #runtime::Scalar>::read(self.storage.as_bytes(), self.storage.byte_offset()) != #tag { return ::core::option::Option::None; }
                    ::core::option::Option::Some(unsafe { let #offset = self.storage.byte_offset(); <#codec as #runtime::Intrlay>::view_mut(self.storage.as_bytes_mut_unchecked(), #offset + #payload_offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }) })
                }
            });
        }
    }
    result
}

fn metadata(model: &Model<'_>, variants: &[Variant<'_>]) -> TokenStream {
    let runtime = &model.runtime;
    let registry = model.binding("registry");
    let name = model.name();
    let docs = model.docs();
    let alignment = model.options.alignment;
    let targets = model.options.targets(runtime);
    let variants = variants.iter().map(|variant| {
        let name = variant.source.ident.unraw().to_string();
        let docs = documentation(&variant.source.attrs);
        let tag = &variant.tag;
        let fields = variant.fields.iter().map(|field| field.metadata(model));
        let targets = variant.options.targets(runtime);
        quote!(#runtime::schema::Variant { name: #name.into(), docs: #docs.into(), tag: #tag, fields: ::std::vec![#(#fields),*], targets: #targets, synthetic_payload: false })
    });
    quote!(#registry.define::<Self>(|#registry| #runtime::schema::Definition {
        name: #name.into(), docs: #docs.into(), alignment: #alignment, targets: #targets, transparent: false,
        kind: #runtime::schema::DefinitionKind::Enum(::std::vec![#(#variants),*]),
    }))
}
