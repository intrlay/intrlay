//! Input values share the schema's field layouts while borrowing arbitrary sources.

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
    let codecs = fields.iter().map(|field| &field.codec);
    let references = quote!(#(#codecs)*);
    (0..fields.len())
        .map(|index| model.type_binding(&format!("__IntrlayField{index}"), &references))
        .collect()
}

pub fn generics(model: &Model<'_>, fields: &[Field], parameters: &[Ident]) -> Generics {
    let mut generics = model.generics(fields);
    let runtime = &model.runtime;
    for (field, parameter) in fields.iter().zip(parameters) {
        let codec = &field.codec;
        generics.params.push(syn::parse_quote!(#parameter));
        generics
            .make_where_clause()
            .predicates
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

/// Resolve the public Input type before an impl introduces a schema parameter
/// with the same name. The private alias also works for function-local derives;
/// qualifying the public type through `self::` would incorrectly require a module item.
pub fn source_implementation(
    model: &Model<'_>,
    name: &Ident,
    parameters: &[Ident],
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
    let alias = model.type_binding("__IntrlayInput", &quote!(#name #(#parameters)*));
    let types = (!parameters.is_empty()).then(|| quote!(<#(#parameters),*>));
    let implementation = emit(&alias);
    quote! {
        const _: () = {
            type #alias #types = #name #types;
            #implementation
        };
    }
}

pub fn record(
    model: &Model<'_>,
    shape: &Fields,
    fields: &[Field],
    layouts: &[TokenStream],
) -> syn::Result<TokenStream> {
    let name = name(model)?;
    let parameters = parameters(model, fields);
    let declaration_fields = self::fields(shape, fields, &parameters);
    let vis = model.visibility();
    let runtime = &model.runtime;
    let ident = &model.input.ident;
    let types = view::type_arguments(&model.input.generics);
    let codec = quote!(#ident #types);
    let generics = generics(model, fields, &parameters);
    let (implementation, _, predicates) = generics.split_for_impl();
    let input_types = if parameters.is_empty() {
        TokenStream::new()
    } else {
        quote!(<#(#parameters),*>)
    };
    let semicolon = (!matches!(shape, Fields::Named(_))).then(|| quote!(;));
    let cursor = model.binding("cursor");
    let writer = model.binding("writer");
    let offset = model.binding("offset");
    let measure = fields.iter().map(|field| {
        let member = &field.member;
        let codec = &field.codec;
        quote!(#runtime::EncodeSource::<#codec>::measure(&self.#member, #cursor)?;)
    });
    let write = fields.iter().enumerate().map(|(index, field)| {
        let member = &field.member;
        let codec = &field.codec;
        quote!(#runtime::EncodeSource::<#codec>::write(&self.#member, #writer, #offset + const { #runtime::layout::field_offset(&[#(#layouts),*], #index) })?;)
    });
    let initialization = fields.iter().zip(&parameters).map(|(field, parameter)| {
        let codec = &field.codec;
        quote!((<#codec as #runtime::Intrlay>::LAYOUT, <#parameter as #runtime::EncodeSource<#codec>>::INITIALIZATION))
    });
    let fixed = fields.iter().zip(&parameters).map(|(field, parameter)| {
        let codec = &field.codec;
        quote!(<#parameter as #runtime::EncodeSource<#codec>>::WRITES_FIXED_LAYOUT)
    });
    let sizes = layouts.iter().map(|layout| quote!((#layout).size()));
    let owned = owned(model, &model.generics(fields));
    let implementation_tokens = source_implementation(model, &name, &parameters, |name| {
        quote! {
            // SAFETY: Uses the schema codecs' exact field offsets and source coverage proofs.
            #[automatically_derived]
            unsafe impl #implementation #runtime::EncodeSource<#codec> for #name #input_types #predicates {
                const WRITES_FIXED_LAYOUT: ::core::primitive::bool = true #(&& #fixed)*
                    && <#codec as #runtime::Intrlay>::LAYOUT.size() == 0usize #(+ #sizes)*;
                const INITIALIZATION: #runtime::Initialization = #runtime::Initialization::record(
                    <#codec as #runtime::Intrlay>::LAYOUT, &[#(#initialization),*]);
                fn measure(&self, #cursor: &mut #runtime::SizeCursor) -> ::core::result::Result<(), #runtime::EncodeError> {
                    #(#measure)*
                    ::core::result::Result::Ok(())
                }
                fn write(&self, #writer: &mut #runtime::Writer<'_>, #offset: ::core::primitive::usize) -> ::core::result::Result<(), #runtime::EncodeError> {
                    #(#write)*
                    ::core::result::Result::Ok(())
                }
            }
        }
    });
    Ok(quote! {
        /// Encoding input whose fields can borrow storage independently of the owned schema.
        /// Each type parameter selects the source representation of one declared field.
        #[derive(Clone, Copy, Debug)]
        #[allow(dead_code, non_snake_case, reason = "Generated input fields and variants preserve source names and are independently usable") ]
        #vis struct #name #input_types #declaration_fields #semicolon
        #implementation_tokens
        #owned
    })
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
