use std::collections::{BTreeMap, BTreeSet};

use intrlay_schema::Language;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Expr, LitInt, LitStr, Path, Type, WherePredicate};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Container,
    Variant,
    Field,
}

#[derive(Default)]
pub struct Attributes {
    pub name: Option<String>,
    pub remote: Option<Type>,
    pub from: Option<Path>,
    pub with: Option<Type>,
    pub getter: Option<Path>,
    pub tag: Option<Expr>,
    pub alignment: usize,
    pub languages: BTreeMap<String, ApiAttributes>,
    pub projection: Option<Exposure>,
    pub generate_getters: Option<bool>,
    pub generate_setters: Option<bool>,
    pub generate_component_accessors: Option<bool>,
    pub generate_byte_range_accessors: Option<bool>,
    pub generate_value_accessors: Option<bool>,
    pub create: Option<bool>,
    pub runtime: Option<Path>,
    pub bounds: Option<Vec<WherePredicate>>,
    pub no_field_bounds: bool,
    pub transparent: bool,
    /// The adapter author proves the source's initialized bytes match its wire layout.
    pub native_layout: Option<Expr>,
}

#[derive(Default)]
pub struct ApiAttributes {
    pub name: Option<String>,
    pub generate_getters: Option<bool>,
    pub generate_setters: Option<bool>,
    pub generate_component_accessors: Option<bool>,
    pub generate_byte_range_accessors: Option<bool>,
    pub generate_value_accessors: Option<bool>,
    pub create: Option<bool>,
}

pub enum Exposure {
    Fields,
    Flatten,
    Indexed,
    IndexedOnly,
    TypedArray(syn::Ident),
}

const LANGUAGES: [(&str, Language); 6] = [
    ("rust", Language::Rust),
    ("cpp", Language::Cpp),
    ("typescript", Language::TypeScript),
    ("csharp", Language::CSharp),
    ("python", Language::Python),
    ("dart", Language::Dart),
];

impl Attributes {
    pub fn parse(attributes: &[Attribute], scope: Scope) -> syn::Result<Self> {
        let mut options = Self {
            alignment: 1,
            ..Self::default()
        };
        let mut seen = BTreeSet::new();
        let mut language_seen = BTreeMap::<String, BTreeSet<String>>::new();
        for attribute in attributes {
            if attribute.path().is_ident("repr") {
                attribute.parse_nested_meta(|meta| {
                    if meta.path.is_ident("align") {
                        let content;
                        syn::parenthesized!(content in meta.input);
                        options.alignment = options
                            .alignment
                            .max(content.parse::<LitInt>()?.base10_parse()?);
                    } else if meta.path.is_ident("packed") {
                        return Err(
                            meta.error("packed records cannot provide aligned TypedArray access")
                        );
                    }
                    Ok(())
                })?;
            }
            if !attribute.path().is_ident("intrlay") {
                continue;
            }
            attribute.parse_nested_meta(|meta| {
                let key = option_name(&meta)?;
                if key == "unsafe" {
                    check_scope(&meta, &key, scope)?;
                    options.native_layout = Some(parse_native_layout(&meta, &mut seen)?);
                    return Ok(());
                }
                if let Some((language, _)) = LANGUAGES.iter().find(|(name, _)| *name == key) {
                    let api = options.languages.entry((*language).into()).or_default();
                    let seen = language_seen.entry((*language).into()).or_default();
                    return meta.parse_nested_meta(|target| api.parse(&target, scope, language, seen));
                }
                check_scope(&meta, &key, scope)?;
                unique(&meta, &key, &mut seen)?;
                match key.as_str() {
                    "name" => options.name = Some(meta.value()?.parse::<LitStr>()?.value()),
                    "remote" => options.remote = Some(meta.value()?.parse()?),
                    "from" => options.from = Some(meta.value()?.parse()?),
                    "with" => options.with = Some(meta.value()?.parse()?),
                    "getter" => options.getter = Some(meta.value()?.parse()?),
                    "tag" => options.tag = Some(meta.value()?.parse()?),
                    "project" => {
                        let value = meta.value()?.parse::<LitStr>()?;
                        options.projection = Some(match value.value().as_str() {
                            "fields" => Exposure::Fields,
                            "flatten" => Exposure::Flatten,
                            "indexed" => Exposure::Indexed,
                            "indexed_only" => Exposure::IndexedOnly,
                            _ => Exposure::TypedArray(scalar(&value)?),
                        });
                    }
                    "generate_getters" => options.generate_getters = Some(meta.value()?.parse::<syn::LitBool>()?.value),
                    "generate_setters" => options.generate_setters = Some(meta.value()?.parse::<syn::LitBool>()?.value),
                    "generate_component_accessors" => options.generate_component_accessors = Some(meta.value()?.parse::<syn::LitBool>()?.value),
                    "generate_byte_range_accessors" => options.generate_byte_range_accessors = Some(meta.value()?.parse::<syn::LitBool>()?.value),
                    "generate_value_accessors" => options.generate_value_accessors = Some(meta.value()?.parse::<syn::LitBool>()?.value),
                    "create" => options.create = Some(meta.value()?.parse::<syn::LitBool>()?.value),
                    "crate" => options.runtime = Some(meta.value()?.parse::<LitStr>()?.parse()?),
                    "bound" => {
                        let value = meta.value()?.parse::<LitStr>()?;
                        let parser = syn::punctuated::Punctuated::<WherePredicate, syn::Token![,]>::parse_terminated;
                        options.bounds = Some(value.parse_with(parser)?.into_iter().collect());
                    }
                    "no_field_bounds" => options.no_field_bounds = true,
                    "transparent" => options.transparent = true,
                    _ => return Err(meta.error("unknown intrlay attribute")),
                }
                Ok(())
            })?;
        }
        if !options.alignment.is_power_of_two() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "alignment must be a power of two",
            ));
        }
        if options.from.is_some() && options.remote.is_none() {
            return Err(syn::Error::new_spanned(
                &options.from,
                "from requires a remote type",
            ));
        }
        if options.no_field_bounds && options.bounds.is_some() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "bound replaces inferred bounds; do not combine it with no_field_bounds",
            ));
        }
        Ok(options)
    }

    pub fn targets(&self, runtime: &Path) -> TokenStream {
        let defaults = ApiAttributes::default();
        let entries = LANGUAGES.iter().filter_map(|(name, variant)| {
            let api = self.languages.get(*name).unwrap_or(&defaults);
            let reference_target = !variant.has_value_types();
            let projection = reference_target
                .then_some(self.projection.as_ref())
                .flatten();
            let getters = reference_target
                .then(|| api.generate_getters.or(self.generate_getters))
                .flatten();
            let mutators = reference_target
                .then(|| api.generate_setters.or(self.generate_setters))
                .flatten();
            let create = api.create.or(self.create);
            let components = reference_target
                .then(|| {
                    api.generate_component_accessors
                        .or(self.generate_component_accessors)
                })
                .flatten();
            let name = api.name.as_ref().or(self.name.as_ref());
            let ranges = reference_target
                .then(|| {
                    api.generate_byte_range_accessors
                        .or(self.generate_byte_range_accessors)
                })
                .flatten();
            let values = reference_target
                .then(|| {
                    api.generate_value_accessors
                        .or(self.generate_value_accessors)
                })
                .flatten();
            if name.is_none()
                && projection.is_none()
                && getters.is_none()
                && mutators.is_none()
                && components.is_none()
                && ranges.is_none()
                && values.is_none()
                && create.is_none()
            {
                return None;
            }
            let language = syn::Ident::new(&format!("{variant:?}"), proc_macro2::Span::call_site());
            let exposure = match projection {
                None => quote!(::core::option::Option::None),
                Some(Exposure::Fields) => {
                    quote!(::core::option::Option::Some(#runtime::schema::Exposure::Fields))
                }
                Some(Exposure::Flatten) => {
                    quote!(::core::option::Option::Some(#runtime::schema::Exposure::Flatten))
                }
                Some(Exposure::Indexed) => {
                    quote!(::core::option::Option::Some(#runtime::schema::Exposure::Indexed))
                }
                Some(Exposure::IndexedOnly) => {
                    quote!(::core::option::Option::Some(#runtime::schema::Exposure::IndexedOnly))
                }
                Some(Exposure::TypedArray(scalar)) => quote!(::core::option::Option::Some(
                    #runtime::schema::Exposure::TypedArray(#runtime::schema::Scalar::#scalar)
                )),
            };
            let getters = optional_bool(getters);
            let mutators = optional_bool(mutators);
            let components = optional_bool(components);
            let ranges = optional_bool(ranges);
            let values = optional_bool(values);
            let create = optional_bool(create);
            let name = name.map_or_else(
                || quote!(::core::option::Option::None),
                |name| quote!(::core::option::Option::Some(#name.into())),
            );
            Some(quote!((
                #runtime::schema::Language::#language,
                #runtime::schema::TargetOptions {
                    name: #name,
                    exposure: #exposure,
                    generate_getters: #getters,
                    generate_setters: #mutators,
                    generate_component_accessors: #components,
                    generate_byte_range_accessors: #ranges,
                    generate_value_accessors: #values,
                    create: #create,
                }
            )))
        });
        quote!(::std::collections::BTreeMap::from([#(#entries),*]))
    }

    pub fn create(&self) -> bool {
        self.languages
            .get("rust")
            .and_then(|api| api.create)
            .or(self.create)
            .unwrap_or(true)
    }
    pub fn rust_name(&self) -> Option<&str> {
        self.languages
            .get("rust")
            .and_then(|api| api.name.as_deref())
            .or(self.name.as_deref())
    }
}

impl ApiAttributes {
    fn parse(
        &mut self,
        meta: &syn::meta::ParseNestedMeta<'_>,
        scope: Scope,
        language: &str,
        seen: &mut BTreeSet<String>,
    ) -> syn::Result<()> {
        let key = option_name(meta)?;
        if key == "project" {
            return Err(
                meta.error("project is a common option; place it outside the language scope")
            );
        }
        check_scope(meta, &key, scope)?;
        if matches!(
            key.as_str(),
            "generate_getters"
                | "generate_setters"
                | "generate_component_accessors"
                | "generate_byte_range_accessors"
                | "generate_value_accessors"
        ) && matches!(language, "rust" | "cpp" | "csharp")
        {
            return Err(meta
                .error("accessor generation options apply only to typescript, python, and dart"));
        }
        unique(meta, &key, seen)?;
        match key.as_str() {
            "name" => self.name = Some(meta.value()?.parse::<LitStr>()?.value()),
            "generate_getters" => {
                self.generate_getters = Some(meta.value()?.parse::<syn::LitBool>()?.value);
            }
            "generate_setters" => {
                self.generate_setters = Some(meta.value()?.parse::<syn::LitBool>()?.value);
            }
            "generate_component_accessors" => {
                self.generate_component_accessors =
                    Some(meta.value()?.parse::<syn::LitBool>()?.value);
            }
            "generate_byte_range_accessors" => {
                self.generate_byte_range_accessors =
                    Some(meta.value()?.parse::<syn::LitBool>()?.value);
            }
            "generate_value_accessors" => {
                self.generate_value_accessors = Some(meta.value()?.parse::<syn::LitBool>()?.value);
            }
            "create" => self.create = Some(meta.value()?.parse::<syn::LitBool>()?.value),
            _ => {
                return Err(
                    meta.error("expected name, generate_getters, generate_setters, generate_component_accessors, generate_byte_range_accessors, generate_value_accessors, or create")
                );
            }
        }
        Ok(())
    }
}

fn option_name(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<String> {
    meta.path
        .get_ident()
        .map(ToString::to_string)
        .ok_or_else(|| meta.error("expected an Intrlay option name"))
}

fn unique(
    meta: &syn::meta::ParseNestedMeta<'_>,
    key: &str,
    seen: &mut BTreeSet<String>,
) -> syn::Result<()> {
    if !seen.insert(key.into()) {
        return Err(meta.error(format!("duplicate or conflicting {key} option")));
    }
    Ok(())
}

fn check_scope(meta: &syn::meta::ParseNestedMeta<'_>, key: &str, scope: Scope) -> syn::Result<()> {
    let valid = match key {
        "crate" | "bound" | "no_field_bounds" | "remote" | "from" | "transparent" | "create"
        | "unsafe" => scope == Scope::Container,
        "with" | "getter" => scope == Scope::Field,
        "tag" => scope == Scope::Variant,
        "project" | "generate_component_accessors" | "generate_byte_range_accessors" => {
            scope != Scope::Variant
        }
        _ => true,
    };
    if !valid {
        return Err(meta.error(format!(
            "{key} is not supported at this annotation location"
        )));
    }
    Ok(())
}

fn optional_bool(value: Option<bool>) -> TokenStream {
    value.map_or_else(
        || quote!(::core::option::Option::None),
        |value| quote!(::core::option::Option::Some(#value)),
    )
}

fn scalar(value: &LitStr) -> syn::Result<syn::Ident> {
    let name = match value.value().as_str() {
        "i8" => "I8",
        "u8" => "U8",
        "i16" => "I16",
        "u16" => "U16",
        "i32" => "I32",
        "u32" => "U32",
        "i64" => "I64",
        "u64" => "U64",
        "f32" => "F32",
        "f64" => "F64",
        _ => {
            return Err(syn::Error::new_spanned(
                value,
                "expected fields, flatten, indexed, indexed_only, or a numeric scalar (i8, u8, i16, u16, i32, u32, i64, u64, f32, f64)",
            ));
        }
    };
    Ok(syn::Ident::new(name, value.span()))
}

pub fn documentation(attributes: &[Attribute]) -> String {
    attributes
        .iter()
        .filter_map(|attribute| {
            if !attribute.path().is_ident("doc") {
                return None;
            }
            let syn::Meta::NameValue(value) = &attribute.meta else {
                return None;
            };
            let Expr::Lit(value) = &value.value else {
                return None;
            };
            let syn::Lit::Str(value) = &value.lit else {
                return None;
            };
            let value = value.value();
            let lines = value.lines().collect::<Vec<_>>();
            let decorated = lines.len() > 1
                && lines
                    .iter()
                    .skip(1)
                    .filter(|line| !line.trim().is_empty())
                    .all(|line| line.trim_start().starts_with('*'));
            Some(
                lines
                    .iter()
                    .enumerate()
                    .map(|(index, line)| {
                        let line = if decorated && index > 0 {
                            line.trim_start().strip_prefix('*').unwrap_or(line)
                        } else {
                            line
                        };
                        line.strip_prefix(' ').unwrap_or(line)
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn with_documentation(text: &str, note: &str) -> String {
    if text.is_empty() {
        note.into()
    } else {
        format!("{text}\n\n{note}")
    }
}

fn parse_native_layout(
    meta: &syn::meta::ParseNestedMeta<'_>,
    seen: &mut BTreeSet<String>,
) -> syn::Result<Expr> {
    let mut expression = None;
    meta.parse_nested_meta(|proof| {
        if !proof.path.is_ident("native_layout") {
            return Err(proof.error("expected native_layout"));
        }
        unique(&proof, "native_layout", seen)?;
        expression = Some(if proof.input.peek(syn::Token![=]) {
            proof.value()?.parse()?
        } else {
            syn::parse_quote!(true)
        });
        Ok(())
    })?;
    expression.ok_or_else(|| meta.error("expected native_layout"))
}

#[cfg(test)]
mod tests {
    use super::{Attributes, Scope};

    #[test]
    fn native_layout_proofs_are_explicit_container_annotations() -> syn::Result<()> {
        let proof = syn::parse_quote!(#[intrlay(unsafe(native_layout))]);
        assert!(
            Attributes::parse(std::slice::from_ref(&proof), Scope::Container)?
                .native_layout
                .is_some()
        );
        for scope in [Scope::Field, Scope::Variant] {
            assert!(Attributes::parse(std::slice::from_ref(&proof), scope).is_err());
        }
        for invalid in [
            syn::parse_quote!(#[intrlay(native_layout)]),
            syn::parse_quote!(#[intrlay(unsafe())]),
            syn::parse_quote!(#[intrlay(unsafe(other))]),
            syn::parse_quote!(#[intrlay(unsafe(native_layout, native_layout))]),
        ] {
            assert!(Attributes::parse(&[invalid], Scope::Container).is_err());
        }
        assert!(Attributes::parse(&[proof.clone(), proof], Scope::Container).is_err());
        let conditional =
            syn::parse_quote!(#[intrlay(unsafe(native_layout = Child::NATIVE_LAYOUT))]);
        assert!(
            Attributes::parse(&[conditional], Scope::Container)?
                .native_layout
                .is_some()
        );
        Ok(())
    }
}
