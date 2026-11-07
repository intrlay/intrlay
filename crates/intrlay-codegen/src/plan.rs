mod cache;
mod encoding;
mod generation;
mod initialization;
mod mutation;
mod properties;
mod view_plan;
pub use cache::ViewCache;
pub use encoding::EncodingPlan;
pub use mutation::{Mutation, MutationStep};
pub use view_plan::ViewPlan;

use std::collections::{BTreeMap, BTreeSet};

use intrlay_layout::Layout;
use intrlay_schema::{
    ApiGeneration, DefinitionKind, Exposure, Language, Layouts, Scalar, Schema, TargetOptions, Type,
};

use crate::{
    GenerateError, GenerateOptions, SymbolKind, Visibility, backend::Backend, names::Names,
};

pub type Id = usize;

#[derive(Clone, Default)]
pub struct Options {
    pub exposure: Exposure,
    pub explicit_exposure: bool,
    pub generation: ApiGeneration,
}

impl Options {
    fn apply(&mut self, overrides: Option<&TargetOptions>, language: Language) {
        if let Some(overrides) = overrides {
            if let Some(exposure) = overrides.exposure {
                self.exposure = exposure;
                self.explicit_exposure = true;
            }
            if language.supports_accessor_generation() {
                if let Some(getters) = overrides.generate_getters {
                    self.generation.getters &= getters;
                }
                if let Some(setters) = overrides.generate_setters {
                    self.generation.setters &= setters;
                }
            }
            if let Some(create) = overrides.create {
                self.generation.create &= create;
            }
        }
    }
}

#[derive(Clone)]
pub struct Field {
    pub source_name: String,
    pub name: String,
    pub property_name: String,
    pub ty: Id,
    pub offset: usize,
    pub docs: String,
    pub options: Options,
}

#[derive(Clone)]
pub struct Variant {
    pub name: String,
    pub docs: String,
    pub tag: u32,
    pub fields: Vec<Field>,
    pub synthetic_payload: bool,
}

#[derive(Clone)]
pub enum Kind {
    Scalar(Scalar),
    Unit,
    String,
    Record(Vec<Field>),
    Enum(Vec<Variant>),
    Array { element: Id, length: usize },
    Sequence(Id),
    Optional { value: Id, offset: usize },
    Boxed(Id),
}

#[derive(Clone)]
pub struct Node {
    /// Encoding metadata remains canonical when accessors use another projection.
    pub codec: Id,
    pub name: String,
    pub view_name: String,
    pub docs: String,
    pub transparent: bool,
    pub kind: Kind,
    pub layout: Layout,
    pub buffer_alignment: usize,
    pub options: Options,
    properties: properties::Properties,
}

/// Canonical codecs and projected APIs are built once and shared by validation
/// and every backend. Projected nodes retain their canonical `Node::codec` ID.
pub struct GenerationPlan {
    pub encoding: EncodingPlan,
    pub views: ViewPlan,
}

impl GenerationPlan {
    pub fn new(
        schema: &Schema,
        backend: &'static Backend,
        options: GenerateOptions,
    ) -> Result<Self, GenerateError> {
        let encoding = EncodingPlan::new(TypeGraph::new(schema, backend, options)?);
        let views = ViewPlan::new(&encoding.types);
        Ok(Self { encoding, views })
    }
}

/// Resolved types and layout queries shared by both phases. Encoding and view
/// plans own separate graphs: projections never rewrite the canonical graph.
#[derive(Clone)]
pub struct TypeGraph {
    pub backend: &'static Backend,
    pub names: Names,
    pub module_name: String,
    pub cpp_namespace: Option<String>,
    pub csharp_namespace: Option<String>,
    canonical_nodes: usize,
    pub nodes: Vec<Node>,
    pub exports: BTreeMap<String, Id>,
}

impl TypeGraph {
    fn new(
        schema: &Schema,
        backend: &'static Backend,
        options: GenerateOptions,
    ) -> Result<Self, GenerateError> {
        let mut builder = Builder::new(schema, backend, options)?;
        let language = backend.language;
        // Reserve named declarations first so recursive references resolve without
        // changing public names according to the order fields happen to be visited.
        for (identity, definition) in &schema.definitions {
            let exported_name = schema
                .exports
                .iter()
                .find_map(|(name, ty)| (*ty == Type::Named(identity.clone())).then_some(name));
            let explicit_name = definition
                .targets
                .get(&language)
                .and_then(|options| options.name.as_ref());
            let name = explicit_name.cloned().unwrap_or_else(|| {
                builder
                    .plan
                    .names
                    .type_name(exported_name.unwrap_or(&definition.name))
            });
            let name = identifier(&name)?;
            if !builder.names.insert(name.clone()) {
                return Err(GenerateError(format!(
                    "duplicate generated name {name}; assign explicit names to generic specializations"
                )));
            }
            builder.reserve(Type::Named(identity.clone()), name)?;
        }
        for (identity, definition) in &schema.definitions {
            let id = builder.index(&Type::Named(identity.clone()))?;
            let kind = match &definition.kind {
                DefinitionKind::Record(fields) => {
                    Kind::Record(builder.fields(fields, definition.alignment, 0)?)
                }
                DefinitionKind::Enum(variants) => {
                    let layout = builder.layouts.variants(variants, definition.alignment)?;
                    let mut result = Vec::with_capacity(variants.len());
                    for variant in variants {
                        let variant_options = variant.targets.get(&language);
                        let mut fields =
                            builder.fields(&variant.fields, 1, layout.payload_offset)?;
                        for field in &mut fields {
                            field.options.apply(variant_options, language);
                        }
                        result.push(Variant {
                            name: identifier(
                                &variant_options
                                    .and_then(|options| options.name.clone())
                                    .unwrap_or_else(|| {
                                        builder.plan.names.symbol(
                                            &variant.name,
                                            SymbolKind::Variant,
                                            Some(Visibility::Public),
                                            &[],
                                        )
                                    }),
                            )?,
                            docs: variant.docs.clone(),
                            tag: variant.tag,
                            fields,
                            synthetic_payload: variant.synthetic_payload,
                        });
                    }
                    Kind::Enum(result)
                }
            };
            builder.plan.nodes[id].kind = kind;
            builder.plan.nodes[id].transparent = definition.transparent;
            builder.plan.nodes[id].docs.clone_from(&definition.docs);
            builder.plan.nodes[id]
                .options
                .apply(definition.targets.get(&language), language);
        }
        for (name, ty) in &schema.exports {
            let id = builder.index(ty)?;
            builder.plan.exports.insert(identifier(name)?, id);
        }
        builder.plan.classify_nodes();
        builder.plan.resolve_array_projections();
        builder.plan.check_exposures()?;
        builder.plan.check_transparent_aliases()?;
        builder.plan.canonical_nodes = builder.plan.nodes.len();
        builder.plan.resolve_synthetic_generation();
        Ok(builder.plan)
    }

    pub fn input_name(&self, id: Id) -> String {
        self.nodes[id].name.clone()
    }

    fn resolve_array_projections(&mut self) {
        use crate::backend::FixedArrayApi;
        if self.backend.fixed_array_api == FixedArrayApi::Structural {
            return;
        }
        let projections: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(id, node)| {
                if node.options.explicit_exposure {
                    return node.options.exposure;
                }
                if matches!(node.kind, Kind::Array { .. })
                    && let Some(scalar) = self.array_scalar(id)
                {
                    return Exposure::TypedArray(scalar);
                }
                node.options.exposure
            })
            .collect();
        for (id, node) in self.nodes.iter_mut().enumerate() {
            node.options.exposure = projections[id];
            let fields: Vec<_> = match &mut node.kind {
                Kind::Record(fields) => fields.iter_mut().collect(),
                Kind::Enum(variants) => variants
                    .iter_mut()
                    .flat_map(|variant| &mut variant.fields)
                    .collect(),
                _ => Vec::new(),
            };
            for field in fields {
                if !field.options.explicit_exposure {
                    field.options.exposure = projections[field.ty];
                }
            }
        }
    }

    fn array_scalar(&self, id: Id) -> Option<Scalar> {
        match self.nodes[id].kind {
            Kind::Scalar(Scalar::Bool) => None,
            Kind::Scalar(scalar) => Some(scalar),
            Kind::Array { element, .. } => self.array_scalar(element),
            _ => None,
        }
    }
    pub fn view_name(&self, id: Id) -> String {
        self.nodes[id].view_name.clone()
    }

    /// The field exposed in place of a transparent reference-type wrapper.
    pub fn folded_field(&self, id: Id) -> Option<&Field> {
        self.folded_node(&self.nodes[id])
    }

    pub fn folded_node<'a>(&self, node: &'a Node) -> Option<&'a Field> {
        if node.transparent
            && !self.backend.language.has_value_types()
            && let Kind::Record(fields) = &node.kind
        {
            return fields.first();
        }
        None
    }

    pub fn scalar_view(&self, id: Id) -> Option<Scalar> {
        if matches!(self.nodes[id].options.exposure, Exposure::TypedArray(_)) {
            return None;
        }
        if let Some(field) = self.folded_field(id) {
            return self.scalar_view(field.ty);
        }
        if let Kind::Scalar(scalar) = self.nodes[id].kind {
            Some(scalar)
        } else {
            None
        }
    }

    /// Whether borrowing the value can expose mutable content without resizing it.
    pub fn borrows_content(&self, id: Id) -> bool {
        if matches!(self.nodes[id].options.exposure, Exposure::TypedArray(_)) {
            return true;
        }
        if let Some(field) = self.folded_field(id) {
            return self.borrows_content(field.ty);
        }
        match self.nodes[id].kind {
            Kind::Record(_) | Kind::Enum(_) | Kind::Sequence(_) | Kind::Array { .. } => true,
            Kind::Optional { value, .. } | Kind::Boxed(value) => self.borrows_content(value),
            _ => false,
        }
    }

    pub fn caches_view(&self, id: Id) -> bool {
        if matches!(self.nodes[id].options.exposure, Exposure::TypedArray(_)) {
            return true;
        }
        if let Some(field) = self.folded_field(id) {
            return self.caches_view(field.ty);
        }
        match self.nodes[id].kind {
            Kind::Record(_) | Kind::Enum(_) | Kind::Sequence(_) | Kind::Array { .. } => true,
            Kind::Boxed(value) | Kind::Optional { value, .. } => self.caches_view(value),
            _ => false,
        }
    }

    /// Numeric projection opts into lane-level access without constructing a
    /// borrowed array wrapper for each element in a record collection.
    pub fn component_scalar(&self, field: &Field) -> Option<Scalar> {
        if !self.backend.native_numeric_array_views {
            return None;
        }
        match field.options.exposure {
            Exposure::TypedArray(scalar) => Some(scalar),
            _ => None,
        }
    }

    /// Public schema types and views returned by their accessors form the API.
    /// Inline scalar, Option, and Box layouts need no wrapper of their own unless
    /// the caller explicitly exports one as a standalone buffer entry point.
    fn collect_views(&self) -> BTreeSet<Id> {
        let mut views = BTreeSet::new();
        for id in self.exports.values().copied().chain(
            self.nodes
                .iter()
                .take(self.canonical_nodes)
                .enumerate()
                .filter_map(|(id, node)| {
                    matches!(node.kind, Kind::Record(_) | Kind::Enum(_)).then_some(id)
                }),
        ) {
            self.include_view(id, &mut views);
        }
        views
    }

    fn include_view(&self, id: Id, views: &mut BTreeSet<Id>) {
        if !views.insert(id) {
            return;
        }
        self.include_child_views(id, views);
    }

    /// Views instantiated by generated accessors, rather than public factories.
    fn collect_borrowed_views(&self, emitted: &BTreeSet<Id>) -> BTreeSet<Id> {
        let mut borrowed = BTreeSet::new();
        for &id in emitted {
            self.include_child_views(id, &mut borrowed);
        }
        borrowed
    }

    fn include_child_views(&self, id: Id, views: &mut BTreeSet<Id>) {
        let node = &self.nodes[id];
        match &node.kind {
            Kind::Record(fields) => self.include_field_views(node, fields, views),
            Kind::Enum(variants) => {
                for variant in variants {
                    self.include_field_views(node, &variant.fields, views);
                }
            }
            Kind::Sequence(element) | Kind::Array { element, .. }
                if node.options.generation.getters || node.options.generation.setters =>
            {
                self.include_value_view(*element, self.nodes[*element].options.exposure, views);
            }
            _ if node.options.generation.getters || node.options.generation.setters => {
                self.include_value_view(id, node.options.exposure, views);
            }
            _ => {}
        }
    }

    fn include_field_views(&self, node: &Node, fields: &[Field], views: &mut BTreeSet<Id>) {
        for field in fields {
            if field_generation(node, field).getters || field_generation(node, field).setters {
                self.include_value_view(field.ty, field.options.exposure, views);
            }
        }
    }

    fn include_value_view(&self, id: Id, exposure: Exposure, views: &mut BTreeSet<Id>) {
        if self.backend.native_numeric_array_views && matches!(exposure, Exposure::TypedArray(_)) {
            return;
        }
        if !matches!(exposure, Exposure::TypedArray(_))
            && let Some(field) = self.folded_field(id)
        {
            self.include_value_view(field.ty, field.options.exposure, views);
            return;
        }
        match self.nodes[id].kind {
            Kind::Record(_) | Kind::Enum(_) | Kind::Sequence(_) | Kind::Array { .. } => {
                self.include_view(id, views);
            }
            Kind::Optional { value, .. } | Kind::Boxed(value) => {
                self.include_value_view(value, self.nodes[value].options.exposure, views);
            }
            Kind::Scalar(_) | Kind::String | Kind::Unit => {}
        }
    }

    /// A count-only array input is safe when zeroed element headers already form
    /// valid values and require no separately allocated payload.
    pub fn zeroable(&self, id: Id) -> bool {
        self.nodes[id].properties.zeroable
    }

    /// Only descendants stored outside their inline headers require a size walk.
    pub fn has_payload(&self, id: Id) -> bool {
        self.nodes[id].properties.has_payload
    }

    /// Whether an ordinary value can itself be represented by null in languages
    /// that erase Box and use null for Unit. An enclosing Option needs a Some
    /// wrapper in that case to retain its separate presence bit.
    pub fn nullable_value(&self, id: Id) -> bool {
        if let Some(field) = self.folded_field(id) {
            return self.nullable_value(field.ty);
        }
        match self.nodes[id].kind {
            Kind::Optional { .. } | Kind::Unit => true,
            Kind::Boxed(value) => self.nullable_value(value),
            _ => false,
        }
    }

    fn check_exposures(&self) -> Result<(), GenerateError> {
        for node in &self.nodes {
            self.check_exposure(node, node.options.exposure)?;
            match &node.kind {
                Kind::Record(fields) => {
                    for field in fields {
                        self.check_exposure(&self.nodes[field.ty], field.options.exposure)?;
                    }
                }
                Kind::Enum(variants) => {
                    for variant in variants {
                        for field in &variant.fields {
                            self.check_exposure(&self.nodes[field.ty], field.options.exposure)?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn check_transparent_aliases(&self) -> Result<(), GenerateError> {
        for (id, node) in self.nodes.iter().enumerate() {
            if self.folded_field(id).is_none() {
                continue;
            }
            let mut visited = BTreeSet::new();
            let mut current = id;
            loop {
                if !visited.insert(current) {
                    return Err(GenerateError(format!(
                        "{} forms a recursive transparent alias; retain a non-transparent record or enum in the cycle",
                        node.name
                    )));
                }
                if let Some(field) = self.folded_field(current) {
                    current = field.ty;
                } else {
                    match self.nodes[current].kind {
                        Kind::Sequence(value)
                        | Kind::Array { element: value, .. }
                        | Kind::Optional { value, .. }
                        | Kind::Boxed(value) => current = value,
                        _ => break,
                    }
                }
            }
        }
        Ok(())
    }

    fn check_exposure(&self, node: &Node, exposure: Exposure) -> Result<(), GenerateError> {
        if self.backend.language.has_value_types() && exposure == Exposure::Indexed {
            return Err(GenerateError(format!(
                "{:?} retains typed collection elements and does not support indexed projections",
                self.backend.language
            )));
        }
        match exposure {
            Exposure::TypedArray(scalar) => {
                if !self.backend.native_numeric_array_views {
                    return Err(GenerateError(format!(
                        "{:?} does not support numeric projections",
                        self.backend.language
                    )));
                }
                let element = match node.kind {
                    Kind::Sequence(element) | Kind::Array { element, .. } => &self.nodes[element],
                    _ => node,
                };
                if !element.properties.homogeneous.matches(scalar)
                    || !element.layout.size().is_multiple_of(scalar.layout().size())
                {
                    return Err(GenerateError(format!(
                        "{} cannot expose a {scalar:?} typed array: only homogeneous numeric fields and padding are permitted",
                        node.name
                    )));
                }
            }
            Exposure::Indexed if !matches!(node.kind, Kind::Sequence(_) | Kind::Array { .. }) => {
                return Err(GenerateError(format!(
                    "indexed projection requires a collection: {}",
                    node.name
                )));
            }
            _ => {}
        }
        Ok(())
    }
}

struct Builder<'a> {
    schema: &'a Schema,
    layouts: Layouts<'a>,
    language: Language,
    plan: TypeGraph,
    types: Vec<Type>,
    names: BTreeSet<String>,
}

impl<'a> Builder<'a> {
    fn new(
        schema: &'a Schema,
        backend: &'static Backend,
        options: GenerateOptions,
    ) -> Result<Self, GenerateError> {
        let mut layouts = Layouts::new(schema);
        layouts.validate()?;
        let language = backend.language;
        Ok(Self {
            schema,
            layouts,
            language,
            plan: TypeGraph {
                backend,
                names: Names {
                    policy: options.naming.unwrap_or_else(|| (backend.naming_policy)()),
                },
                module_name: identifier(options.module_name.as_deref().unwrap_or("Schema"))?,
                cpp_namespace: options.cpp_namespace,
                csharp_namespace: options.csharp_namespace,
                canonical_nodes: 0,
                nodes: Vec::new(),
                exports: BTreeMap::new(),
            },
            types: Vec::new(),
            names: BTreeSet::new(),
        })
    }

    fn reserve(&mut self, ty: Type, name: String) -> Result<Id, GenerateError> {
        let id = self.types.len();
        let view_name = if self.language == Language::Rust {
            // The derive appends View to the emitted native declaration name.
            format!("{name}View")
        } else if let Type::Named(identity) = &ty {
            let definition = &self.schema.definitions[identity];
            if let Some(explicit) = definition
                .targets
                .get(&self.language)
                .and_then(|target| target.name.as_ref())
            {
                format!("{explicit}View")
            } else {
                let source = self
                    .schema
                    .exports
                    .iter()
                    .find_map(|(alias, export)| (*export == ty).then_some(alias))
                    .unwrap_or(&definition.name);
                self.plan.names.type_name(&format!("{source}_view"))
            }
        } else {
            let prefix = if self.language == Language::CSharp {
                ""
            } else {
                "Intrlay"
            };
            self.plan
                .names
                .type_name(&format!("{prefix}{}_view", self.type_label(&ty)))
        };
        self.plan.nodes.push(Node {
            codec: id,
            name,
            view_name: identifier(&view_name)?,
            docs: String::new(),
            kind: Kind::Unit,
            transparent: false,
            layout: self.layouts.of(&ty)?,
            buffer_alignment: self.layouts.buffer_alignment(&ty)?,
            options: Options::default(),
            properties: properties::Properties::default(),
        });
        self.types.push(ty);
        Ok(id)
    }

    fn index(&mut self, ty: &Type) -> Result<Id, GenerateError> {
        if let Some(index) = self.types.iter().position(|existing| existing == ty) {
            return Ok(index);
        }
        let name = self
            .plan
            .names
            .type_name(&format!("Intrlay{}", self.type_label(ty)));
        if !self.names.insert(name.clone()) {
            return Err(GenerateError(format!(
                "generated helper name conflicts with {name}"
            )));
        }
        let id = self.reserve(ty.clone(), name)?;
        self.plan.nodes[id].kind = match ty {
            Type::Scalar(scalar) => Kind::Scalar(*scalar),
            Type::Unit => Kind::Unit,
            Type::String => Kind::String,
            Type::Sequence(element) => Kind::Sequence(self.index(element)?),
            Type::Array { element, length } => Kind::Array {
                element: self.index(element)?,
                length: *length,
            },
            Type::Optional(value) => Kind::Optional {
                value: self.index(value)?,
                offset: self.layouts.of(value)?.alignment(),
            },
            Type::Boxed(value) => Kind::Boxed(self.index(value)?),
            Type::Named(identity) => {
                return Err(GenerateError(format!("missing definition: {identity}")));
            }
        };
        Ok(id)
    }

    fn type_label(&self, ty: &Type) -> String {
        match ty {
            Type::Scalar(scalar) => format!("{scalar:?}"),
            Type::Unit => "Unit".into(),
            Type::String => "String".into(),
            Type::Named(identity) => self
                .types
                .iter()
                .position(|ty| *ty == Type::Named(identity.clone()))
                .map_or_else(
                    || self.schema.definitions[identity].name.clone(),
                    |id| self.plan.nodes[id].name.clone(),
                ),
            Type::Array { element, length } => {
                format!("Array{length}Of{}", self.type_label(element))
            }
            Type::Sequence(element) => format!("SequenceOf{}", self.type_label(element)),
            Type::Optional(value) => format!("Optional{}", self.type_label(value)),
            Type::Boxed(value) => format!("Boxed{}", self.type_label(value)),
        }
    }

    fn fields(
        &mut self,
        fields: &[intrlay_schema::Field],
        alignment: usize,
        base: usize,
    ) -> Result<Vec<Field>, GenerateError> {
        let layout = self.layouts.record(fields, alignment)?;
        let mut result = Vec::with_capacity(fields.len());
        let mut names = BTreeSet::new();
        for (field, offset) in fields.iter().zip(layout.fields) {
            let ty = self.index(&field.ty)?;
            let inherited = match &field.ty {
                Type::Named(identity) => self.schema.definitions[identity]
                    .targets
                    .get(&self.language),
                _ => None,
            };
            let mut options = Options::default();
            options.apply(inherited, self.language);
            options.apply(field.targets.get(&self.language), self.language);
            let source_name = if field.name.chars().all(|c| c.is_ascii_digit()) {
                format!("field_{}", field.name)
            } else {
                identifier(&field.name)?
            };
            let explicit_name = field
                .targets
                .get(&self.language)
                .and_then(|target| target.name.as_ref())
                .cloned();
            let name = explicit_name.clone().unwrap_or_else(|| {
                self.plan.names.symbol(
                    &source_name,
                    self.plan.backend.input_field_kind,
                    Some(Visibility::Public),
                    self.plan.backend.input_field_modifiers,
                )
            });
            let property_name = explicit_name.unwrap_or_else(|| {
                self.plan.names.property(
                    &source_name,
                    !options.generation.setters || !matches!(field.ty, Type::Scalar(_)),
                )
            });
            let name = identifier(&name)?;
            if !names.insert(name.clone()) {
                return Err(GenerateError(format!(
                    "field names collide after casing: {name}; provide an explicit target name"
                )));
            }
            result.push(Field {
                source_name,
                name,
                property_name: identifier(&property_name)?,
                ty,
                offset: base + offset,
                docs: field.docs.clone(),
                options,
            });
        }
        Ok(result)
    }
}

pub(crate) fn identifier(name: &str) -> Result<String, GenerateError> {
    if name.is_empty()
        || name.chars().next().is_some_and(|c| c.is_ascii_digit())
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        Err(GenerateError(format!(
            "invalid portable identifier: {name}"
        )))
    } else {
        Ok(name.into())
    }
}

pub fn field_generation(node: &Node, field: &Field) -> ApiGeneration {
    ApiGeneration {
        getters: node.options.generation.getters && field.options.generation.getters,
        setters: node.options.generation.setters && field.options.generation.setters,
        create: node.options.generation.create && field.options.generation.create,
    }
}

/// Indexed helpers expose an element's field on its enclosing collection.
/// Both declarations' emission hints participate; neither grants extra access.
pub fn indexed_field_generation(collection: &Node, element: &Node, field: &Field) -> ApiGeneration {
    let generation = field_generation(element, field);
    ApiGeneration {
        getters: collection.options.generation.getters && generation.getters,
        setters: collection.options.generation.setters && generation.setters,
        create: collection.options.generation.create && generation.create,
    }
}
