use proc_macro2::{TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::{DeriveInput, Field as SynField, Fields, Generics, Ident, Member, Type, Visibility};

use crate::attributes::{Attributes, Scope, documentation};

pub struct Model<'a> {
    pub input: &'a DeriveInput,
    pub options: Attributes,
    pub view: Ident,
    pub source: TokenStream,
    pub runtime: syn::Path,
}

impl<'a> Model<'a> {
    /// Type parameters use call-site name resolution. Avoid both declared names
    /// and referenced source/codec types, including parsed attribute values.
    pub fn type_binding(&self, name: &str, references: &TokenStream) -> Ident {
        let input = self.input;
        let source = &self.source;
        let runtime = &self.runtime;
        let view = &self.view;
        // Attribute strings are literals in `input`; include their parsed names too.
        let bounds = self.options.bounds.iter().flatten();
        let references = quote!(#input #source #runtime #view #(#bounds)* #references);
        let mut name = name.to_owned();
        while contains_ident(references.clone(), &name) {
            name.push('_');
        }
        Ident::new(&name, proc_macro2::Span::mixed_site())
    }

    /// Generated bindings cannot shadow const parameters, even with mixed-site
    /// hygiene. Keep source identifiers intact and choose fresh internal names.
    pub fn binding(&self, name: &str) -> Ident {
        let mut name = name.to_owned();
        while self
            .input
            .generics
            .params
            .iter()
            .any(|parameter| match parameter {
                syn::GenericParam::Const(parameter) => parameter.ident.unraw() == name,
                syn::GenericParam::Type(parameter) => parameter.ident.unraw() == name,
                syn::GenericParam::Lifetime(_) => false,
            })
        {
            name.push('_');
        }
        Ident::new(&name, proc_macro2::Span::mixed_site())
    }

    pub fn new(input: &'a DeriveInput) -> syn::Result<Self> {
        if input.generics.lifetimes().next().is_some() {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "schema values must own their data; use String, Vec, or Box instead of lifetime parameters",
            ));
        }
        let options = Attributes::parse(&input.attrs, Scope::Container)?;
        let runtime = crate::runtime_path::resolve(options.runtime.as_ref())?;
        let ident = &input.ident;
        let view = options
            .rust_name()
            .map_or_else(|| format!("{ident}View"), |name| format!("{name}View"));
        let view = syn::parse_str::<Ident>(&view)?;
        let types = crate::view::type_arguments(&input.generics);
        let source = options
            .remote
            .as_ref()
            .map_or_else(|| quote!(#ident #types), |remote| quote!(#remote));
        Ok(Self {
            input,
            options,
            view,
            source,
            runtime,
        })
    }

    pub fn generics(&self, fields: &[Field]) -> Generics {
        let mut generics = self.input.generics.clone();
        let runtime = &self.runtime;
        for field in fields {
            let ty = &field.ty;
            let codec = &field.codec;
            // Recursive references already resolve through this impl. Adding the whole
            // field as a predicate would make trait selection recurse through itself.
            if self.options.bounds.is_none()
                && !self.options.no_field_bounds
                && !mentions_type(ty, &self.input.ident, true)
                && self.input.generics.type_params().any(|parameter| {
                    mentions_type(ty, &parameter.ident, false)
                        || mentions_type(codec, &parameter.ident, false)
                })
            {
                generics
                    .make_where_clause()
                    .predicates
                    .push(syn::parse_quote!(#codec: #runtime::Intrlay<Value = #ty>));
            }
        }
        for parameter in generics.type_params_mut() {
            parameter.bounds.push(syn::parse_quote!('static));
        }
        if let Some(bounds) = &self.options.bounds {
            generics
                .make_where_clause()
                .predicates
                .extend(bounds.iter().cloned());
        }
        self.self_resolver().visit_generics_mut(&mut generics);
        generics
    }

    pub fn resolve_expression(&self, expression: &syn::Expr) -> syn::Expr {
        let mut expression = expression.clone();
        self.self_resolver().visit_expr_mut(&mut expression);
        expression
    }

    fn self_resolver(&self) -> ResolveSelf {
        let source = &self.source;
        ResolveSelf(syn::parse_quote!(#source))
    }

    pub fn name(&self) -> String {
        self.options
            .name
            .clone()
            .unwrap_or_else(|| self.input.ident.unraw().to_string())
    }
    pub fn source_constructor(&self) -> syn::Result<syn::Path> {
        let mut path = syn::parse2::<syn::Path>(self.source.clone())?;
        for segment in &mut path.segments {
            if let syn::PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
                arguments.colon2_token = Some(syn::token::PathSep::default());
            }
        }
        Ok(path)
    }
    pub fn docs(&self) -> String {
        documentation(&self.input.attrs)
    }
    pub fn visibility(&self) -> &Visibility {
        &self.input.vis
    }
}

fn contains_ident(tokens: TokenStream, name: &str) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Ident(ident) => ident.unraw() == name,
        TokenTree::Group(group) => contains_ident(group.stream(), name),
        TokenTree::Literal(_) | TokenTree::Punct(_) => false,
    })
}

fn mentions_type(ty: &Type, ident: &Ident, allow_self_path: bool) -> bool {
    match ty {
        Type::Path(path) => {
            let first = path.path.segments.first();
            // A matching name in another module is not a reference to this type.
            // Derives cannot resolve arbitrary qualified paths; explicit bounds
            // cover self references spelled through other modules or aliases.
            let named = path.path.leading_colon.is_none()
                && first.is_some_and(|segment| {
                    segment.ident.unraw() == ident.unraw()
                        || (allow_self_path
                            && segment.ident == "self"
                            && path
                                .path
                                .segments
                                .get(1)
                                .is_some_and(|segment| segment.ident.unraw() == ident.unraw()))
                });
            let qualified = path
                .qself
                .as_ref()
                .is_some_and(|qualified| mentions_type(&qualified.ty, ident, allow_self_path));
            named || qualified || path.path.segments.iter().any(|segment| {
                    match &segment.arguments {
                        syn::PathArguments::AngleBracketed(args) => args.args.iter().any(
                            |arg| matches!(arg, syn::GenericArgument::Type(ty) if mentions_type(ty, ident, allow_self_path)),
                        ),
                        _ => false,
                    }
                })
        }
        Type::Array(array) => mentions_type(&array.elem, ident, allow_self_path),
        Type::Tuple(tuple) => tuple
            .elems
            .iter()
            .any(|ty| mentions_type(ty, ident, allow_self_path)),
        Type::Group(group) => mentions_type(&group.elem, ident, allow_self_path),
        Type::Paren(paren) => mentions_type(&paren.elem, ident, allow_self_path),
        _ => false,
    }
}

pub struct Field {
    pub member: Member,
    pub name: String,
    pub getter: Ident,
    pub ty: Type,
    pub codec: Type,
    pub docs: String,
    pub options: Attributes,
}

impl Field {
    pub fn parse(fields: &Fields, model: &Model<'_>) -> syn::Result<Vec<Self>> {
        fields
            .iter()
            .enumerate()
            .map(|(i, field)| {
                let mut field = Self::new(field, i)?;
                let mut resolver = model.self_resolver();
                syn::visit_mut::VisitMut::visit_type_mut(&mut resolver, &mut field.ty);
                syn::visit_mut::VisitMut::visit_type_mut(&mut resolver, &mut field.codec);
                Ok(field)
            })
            .collect()
    }

    fn new(field: &SynField, index: usize) -> syn::Result<Self> {
        let options = Attributes::parse(&field.attrs, Scope::Field)?;
        let member = field
            .ident
            .clone()
            .map_or_else(|| Member::Unnamed(syn::Index::from(index)), Member::Named);
        let name = field
            .ident
            .as_ref()
            .map_or_else(|| index.to_string(), |name| name.unraw().to_string());
        let getter = if let Some(name) = options.rust_name() {
            syn::parse_str::<Ident>(name)?
        } else {
            field
                .ident
                .clone()
                .unwrap_or_else(|| format_ident!("field_{index}"))
        };
        Ok(Self {
            member,
            name,
            getter,
            ty: field.ty.clone(),
            codec: options.with.clone().unwrap_or_else(|| field.ty.clone()),
            docs: documentation(&field.attrs),
            options,
        })
    }

    pub fn metadata(&self, model: &Model<'_>) -> TokenStream {
        let runtime = &model.runtime;
        let registry = model.binding("registry");
        let name = &self.name;
        let codec = &self.codec;
        let docs = &self.docs;
        let targets = self.options.targets(runtime);
        quote!(#runtime::schema::Field { name: #name.into(), ty: <#codec as #runtime::schema::Describe>::describe(#registry), docs: #docs.into(), targets: #targets })
    }

    pub fn access(&self, model: &Model<'_>) -> TokenStream {
        let value = model.binding("value");
        if let Some(getter) = &self.options.getter {
            quote!(&#getter(#value))
        } else {
            let member = &self.member;
            quote!(&#value.#member)
        }
    }
}

/// Source syntax is reused in view and module scopes. Resolve both types and
/// constant expressions while `Self` still denotes the original declaration.
struct ResolveSelf(Type);

use syn::visit_mut::VisitMut;

impl VisitMut for ResolveSelf {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(path) = ty
            && path.qself.is_none()
            && path.path.is_ident("Self")
        {
            *ty = self.0.clone();
        } else {
            syn::visit_mut::visit_type_mut(self, ty);
        }
    }

    fn visit_type_path_mut(&mut self, path: &mut syn::TypePath) {
        syn::visit_mut::visit_type_path_mut(self, path);
        if path.qself.is_none() && path.path.segments[0].ident == "Self" {
            let source = &self.0;
            let mut tail = path.path.clone();
            tail.segments = tail.segments.into_iter().skip(1).collect();
            *path = syn::parse_quote!(<#source>::#tail);
        }
    }

    fn visit_expr_path_mut(&mut self, path: &mut syn::ExprPath) {
        syn::visit_mut::visit_expr_path_mut(self, path);
        if path.qself.is_none() && path.path.segments[0].ident == "Self" {
            let source = &self.0;
            let mut tail = path.path.clone();
            tail.segments = tail.segments.into_iter().skip(1).collect();
            if tail.segments.is_empty() {
                let Type::Path(source) = source else { return };
                path.path = source.path.clone();
                for segment in &mut path.path.segments {
                    if let syn::PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
                        arguments.colon2_token = Some(syn::token::PathSep::default());
                    }
                }
            } else {
                *path = syn::parse_quote!(<#source>::#tail);
            }
        }
    }
}
