//! Input values share the schema's field layouts while borrowing arbitrary sources.

use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::{Fields, Generics, Ident};

use crate::{
    model::{Field, Model},
    view,
};

pub fn name(model: &Model<'_>) -> syn::Result<Ident> {
    syn::parse_str(&format!(
        "{}Input",
        model
            .options
            .rust_name()
            .unwrap_or(&model.input.ident.to_string())
    ))
}

pub fn parameters(model: &Model<'_>, fields: &[Field]) -> Vec<Ident> {
    named_parameters(
        model,
        fields,
        fields.iter().map(|field| match &field.member {
            syn::Member::Named(name) => name.unraw().to_string(),
            syn::Member::Unnamed(index) => format!("field_{}", index.index),
        }),
    )
}

pub fn named_parameters(
    model: &Model<'_>,
    fields: &[Field],
    names: impl IntoIterator<Item = String>,
) -> Vec<Ident> {
    let codecs = fields.iter().map(|field| &field.codec);
    let mut references = quote!(#(#codecs)*);
    names
        .into_iter()
        .map(|name| {
            let parameter = model.type_binding(
                &format!("{}Source", name.to_upper_camel_case()),
                &references,
            );
            references.extend(quote!(#parameter));
            parameter
        })
        .collect()
}

pub fn generics(model: &Model<'_>, fields: &[Field], parameters: &[Ident]) -> Generics {
    let mut generics = model.generics(fields);
    let runtime = &model.runtime;
    // Input sources follow the schema arguments; required parameters cannot
    // follow the original declaration's optional defaults.
    for parameter in &mut generics.params {
        match parameter {
            syn::GenericParam::Type(parameter) => parameter.default = None,
            syn::GenericParam::Const(parameter) => parameter.default = None,
            syn::GenericParam::Lifetime(_) => {}
        }
    }
    for (field, parameter) in fields.iter().zip(parameters) {
        let codec = &field.codec;
        generics
            .params
            .push(syn::parse_quote!(#parameter: #runtime::EncodeSource<#codec>));
    }
    generics
}

pub fn fields(shape: &Fields, fields: &[Field], parameters: &[Ident]) -> TokenStream {
    let declarations =
        fields
            .iter()
            .zip(parameters)
            .zip(shape)
            .map(|((field, parameter), source)| {
                let member = &field.member;
                let vis = &source.vis;
                let docs = &field.docs;
                if let Fields::Named(_) = shape {
                    quote!(#[doc = #docs] #vis #member: #parameter)
                } else {
                    quote!(#[doc = #docs] #vis #parameter)
                }
            });
    match shape {
        Fields::Named(_) => quote!({ #(#declarations),* }),
        Fields::Unnamed(_) => quote!((#(#declarations),*)),
        Fields::Unit => TokenStream::new(),
    }
}

struct Rename<'a> {
    source: &'a Ident,
    target: &'a Ident,
}
impl syn::visit_mut::VisitMut for Rename<'_> {
    fn visit_ident_mut(&mut self, ident: &mut Ident) {
        if ident.unraw() == self.source.unraw() {
            *ident = self.target.clone();
        }
    }
}

/// Resolve the public Input type before an impl introduces a schema parameter
/// with the same name. The private alias also works for function-local derives;
/// qualifying the public type through `self::` would incorrectly require a module item.
pub fn source_implementation(
    model: &Model<'_>,
    name: &Ident,
    generics: &Generics,
    emit: impl FnOnce(&Ident) -> TokenStream,
) -> TokenStream {
    if !model
        .input
        .generics
        .type_params()
        .any(|parameter| parameter.ident.unraw() == name.unraw())
    {
        return emit(name);
    }
    let alias = model.type_binding("__IntrlayInput", &quote!(#name #generics));
    let mut alias_generics = generics.clone();
    // The schema may itself use `FooInput` as a generic parameter. Rename only
    // that alias parameter, so the RHS still resolves the public input type.
    let replacement = model.type_binding("SchemaArgument", &quote!(#generics));
    syn::visit_mut::VisitMut::visit_generics_mut(
        &mut Rename {
            source: name,
            target: &replacement,
        },
        &mut alias_generics,
    );
    let types = view::type_arguments(&alias_generics);
    let predicates = &alias_generics.where_clause;
    let implementation = emit(&alias);
    quote! {
        const _: () = {
            #[allow(type_alias_bounds, reason = "Private alias preserves constraints while resolving a shadowed public input type")]
            type #alias #alias_generics #predicates = #name #types;
            #implementation
        };
    }
}

/// Input traits depend on the actual sources, not on the phantom owned schema.
/// Emitting the impls here also avoids built-in derives resolving a schema
/// parameter named `FooInput` instead of the input declaration itself.
pub fn traits(
    model: &Model<'_>,
    name: &Ident,
    generics: &Generics,
    parameters: &[Ident],
    clone_body: &TokenStream,
    debug_body: &TokenStream,
) -> TokenStream {
    let input_types = view::type_arguments(generics);
    let formatter = model.binding("formatter");
    let mut result = TokenStream::new();
    for (bound, body) in [
        (
            quote!(::core::clone::Clone),
            quote! {
                fn clone(&self) -> Self { #clone_body }
            },
        ),
        (quote!(::core::marker::Copy), TokenStream::new()),
        (
            quote!(::core::fmt::Debug),
            quote! {
                fn fmt(&self, #formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result { #debug_body }
            },
        ),
    ] {
        let mut constrained = generics.clone();
        for parameter in parameters {
            constrained
                .make_where_clause()
                .predicates
                .push(syn::parse_quote!(#parameter: #bound));
        }
        let (implementation, _, predicates) = constrained.split_for_impl();
        result.extend(quote! {
            #[automatically_derived]
            impl #implementation #bound for #name #input_types #predicates { #body }
        });
    }
    result
}

pub fn record(
    model: &Model<'_>,
    shape: &Fields,
    fields: &[Field],
    layouts: &[TokenStream],
) -> syn::Result<TokenStream> {
    let name = name(model)?;
    let parameters = parameters(model, fields);
    let runtime = &model.runtime;
    let ident = &model.input.ident;
    let types = view::type_arguments(&model.input.generics);
    let codec = quote!(#ident #types);
    let generics = generics(model, fields, &parameters);
    let (implementation, _, predicates) = generics.split_for_impl();
    let input_types = view::type_arguments(&generics);
    let marker = model.type_binding("__intrlay_schema", &quote!(#generics));
    let needs_marker = model.input.generics.type_params().next().is_some();
    let (declaration, constructor) =
        record_declaration(model, shape, fields, &parameters, &generics)?;
    let (clone_body, debug_body) =
        record_trait_bodies(model, shape, fields, &name, &marker, needs_marker);
    let cursor = model.binding("cursor");
    let writer = model.binding("writer");
    let offset = model.binding("offset");
    let measure = crate::encoding::fallible_sequence(fields.iter().map(|field| {
        let member = &field.member;
        let codec = &field.codec;
        quote!(#runtime::EncodeSource::<#codec>::measure(&self.#member, #cursor))
    }));
    let write = crate::encoding::fallible_sequence(fields.iter().enumerate().map(|(index, field)| {
        let member = &field.member;
        let codec = &field.codec;
        quote!(#runtime::EncodeSource::<#codec>::write(&self.#member, #writer, #offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }))
    }));
    let initialization = fields.iter().zip(&parameters).map(|(field, parameter)| {
        let codec = &field.codec;
        quote!((<#codec as #runtime::Intrlay>::LAYOUT, <#parameter as #runtime::EncodeSource<#codec>>::INITIALIZATION))
    });
    let fixed = fields
        .iter()
        .zip(&parameters)
        .map(|(field, parameter)| {
            let codec = &field.codec;
            quote!(<#parameter as #runtime::EncodeSource<#codec>>::WRITES_FIXED_LAYOUT)
        })
        .collect::<Vec<_>>();
    let write = crate::encoding::write_header(
        model,
        &quote!(<#codec as #runtime::Intrlay>::LAYOUT),
        fields.iter().zip(&parameters).enumerate().map(|(index, (field, parameter))| {
            let codec = &field.codec;
            let member = &field.member;
            crate::encoding::NativeField {
                proof: quote!(<#parameter as #runtime::EncodeSource<#codec>>::NATIVE_LAYOUT),
                value_type: quote!(#parameter),
                value: quote!(&self.#member),
                offset: quote!(const { #runtime::layout::field_offset(&[#(#layouts),*], #index) }),
            }
        }),
        &write,
    );
    let sizes = layouts.iter().map(|layout| quote!((#layout).size()));
    let owned = owned(model, &model.generics(fields));
    let implementation_tokens = source_implementation(model, &name, &generics, |name| {
        let traits = traits(
            model,
            name,
            &generics,
            &parameters,
            &clone_body,
            &debug_body,
        );
        let constructor = constructor.as_ref().map(|constructor| {
            quote! {
                impl #implementation #name #input_types #predicates { #constructor }
            }
        });
        quote! {
            #constructor
            #traits
            // SAFETY: Uses the schema codecs' exact field offsets and source coverage proofs.
            #[automatically_derived]
            unsafe impl #implementation #runtime::EncodeSource<#codec> for #name #input_types #predicates {
                const WRITES_FIXED_LAYOUT: ::core::primitive::bool = true #(&& #fixed)*
                    && <#codec as #runtime::Intrlay>::LAYOUT.size() == 0usize #(+ #sizes)*;
                const INITIALIZATION: #runtime::Initialization = #runtime::Initialization::record(
                    <#codec as #runtime::Intrlay>::LAYOUT, &[#(#initialization),*]);
                #[inline]
                fn measure(&self, #cursor: &mut #runtime::SizeCursor) -> ::core::result::Result<(), #runtime::EncodeError> {
                    #measure
                }
                #[inline]
                fn write(&self, #writer: &mut #runtime::Writer<'_>, #offset: ::core::primitive::usize) -> ::core::result::Result<(), #runtime::EncodeError> {
                    #write
                }
            }
        }
    });
    Ok(quote! {
        /// Encoding input whose fields can borrow storage independently of the owned schema.
        /// Each type parameter selects the source representation of one declared field.
        #[allow(dead_code, non_snake_case, reason = "Generated input fields and variants preserve source names and are independently usable") ]
        #declaration
        #implementation_tokens
        #owned
    })
}

fn record_trait_bodies(
    model: &Model<'_>,
    shape: &Fields,
    fields: &[Field],
    name: &Ident,
    marker: &Ident,
    needs_marker: bool,
) -> (TokenStream, TokenStream) {
    let clones = fields.iter().map(|field| {
        let member = &field.member;
        let value = quote!(::core::clone::Clone::clone(&self.#member));
        if matches!(shape, Fields::Named(_)) {
            quote!(#member: #value)
        } else {
            value
        }
    });
    let clone_body = match shape {
        Fields::Named(_) => {
            let marker = needs_marker.then(|| quote!(#marker: ::core::marker::PhantomData,));
            quote!(Self { #(#clones,)* #marker })
        }
        Fields::Unnamed(_) => {
            let marker = needs_marker.then(|| quote!(::core::marker::PhantomData,));
            quote!(Self(#(#clones,)* #marker))
        }
        Fields::Unit => quote!(Self),
    };
    let formatter = model.binding("formatter");
    let debug_fields = fields.iter().map(|field| {
        let member = &field.member;
        let label = &field.name;
        if matches!(shape, Fields::Named(_)) {
            quote!(.field(#label, &self.#member))
        } else {
            quote!(.field(&self.#member))
        }
    });
    let debug_method = if matches!(shape, Fields::Named(_)) {
        quote!(debug_struct)
    } else {
        quote!(debug_tuple)
    };
    let debug_label = name.to_string();
    let debug_body = quote!(#formatter.#debug_method(#debug_label)#(#debug_fields)*.finish());
    (clone_body, debug_body)
}

fn record_declaration(
    model: &Model<'_>,
    shape: &Fields,
    fields: &[Field],
    parameters: &[Ident],
    generics: &Generics,
) -> syn::Result<(syn::ItemStruct, Option<TokenStream>)> {
    let name = name(model)?;
    let declaration_fields = self::fields(shape, fields, parameters);
    let vis = model.visibility();
    let ident = &model.input.ident;
    let types = view::type_arguments(&model.input.generics);
    let codec = quote!(#ident #types);
    let semicolon = (!matches!(shape, Fields::Named(_))).then(|| quote!(;));
    let mut declaration: syn::ItemStruct =
        syn::parse2(quote!(#vis struct #name #declaration_fields #semicolon))?;
    declaration.generics = generics.clone();
    let marker = model.type_binding("__intrlay_schema", &quote!(#generics));
    let marker_type = quote!(::core::marker::PhantomData<fn() -> #codec>);
    let needs_marker = model.input.generics.type_params().next().is_some();
    if needs_marker {
        use syn::parse::Parser;
        match &mut declaration.fields {
            Fields::Named(fields) => fields
                .named
                .push(syn::Field::parse_named.parse2(quote!(#marker: #marker_type))?),
            Fields::Unnamed(fields) => fields
                .unnamed
                .push(syn::Field::parse_unnamed.parse2(marker_type)?),
            Fields::Unit => unreachable!("a schema type parameter must occur in a field"),
        }
    }
    let constructor = needs_marker.then(|| {
        let arguments: Vec<_> = fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                model.binding(&match &field.member {
                    syn::Member::Named(name) => name.unraw().to_string(),
                    syn::Member::Unnamed(_) => format!("field_{index}"),
                })
            })
            .collect();
        let values = fields.iter().zip(&arguments).map(|(field, argument)| {
            let member = &field.member;
            if matches!(shape, Fields::Named(_)) {
                quote!(#member: #argument)
            } else {
                quote!(#argument)
            }
        });
        let value = if matches!(shape, Fields::Named(_)) {
            quote!(Self { #(#values,)* #marker: ::core::marker::PhantomData })
        } else {
            quote!(Self(#(#values,)* ::core::marker::PhantomData))
        };
        quote! {
            /// Supplies field sources while retaining the generic schema at the type level.
            #[must_use]
            #[inline]
            pub fn new(#(#arguments: #parameters),*) -> Self { #value }
        }
    });
    Ok((declaration, constructor))
}

pub fn owned(model: &Model<'_>, generics: &Generics) -> TokenStream {
    let runtime = &model.runtime;
    let source = &model.source;
    let ident = &model.input.ident;
    let types = view::type_arguments(generics);
    let codec = quote!(#ident #types);
    let (implementation, _, predicates) = generics.split_for_impl();
    let cursor = model.binding("cursor");
    let writer = model.binding("writer");
    let offset = model.binding("offset");
    quote! {
        // SAFETY: The original codec supplies the complete encoding and native-layout proof.
        #[automatically_derived]
        unsafe impl #implementation #runtime::EncodeSource<#codec> for #source #predicates {
            const NATIVE_LAYOUT: ::core::primitive::bool = <#codec as #runtime::Intrlay>::NATIVE_LAYOUT;
            const WRITES_FIXED_LAYOUT: ::core::primitive::bool = <#codec as #runtime::Intrlay>::WRITES_FIXED_LAYOUT;
            const INITIALIZATION: #runtime::Initialization = <#codec as #runtime::Intrlay>::INITIALIZATION;
            fn measure(&self, #cursor: &mut #runtime::SizeCursor) -> ::core::result::Result<(), #runtime::EncodeError> {
                <#codec as #runtime::Intrlay>::measure(self, #cursor)
            }
            fn write(&self, #writer: &mut #runtime::Writer<'_>, #offset: ::core::primitive::usize) -> ::core::result::Result<(), #runtime::EncodeError> {
                <#codec as #runtime::Intrlay>::write(self, #writer, #offset)
            }
        }
    }
}
