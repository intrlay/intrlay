//! Member declarations shared by validation and target-specific source emission.
//!
//! These are API decisions, not a target-language syntax tree. Backends retain
//! their own method bodies and optimized load/store implementations.

mod field_helpers;
mod policy;
pub use field_helpers::FieldHelpers;
pub use policy::{AccessorContext, ApiPolicy, DefaultApi, ViewName};

use intrlay_schema::ApiGeneration;

use crate::{
    GenerateError,
    plan::{
        Field, Id, Mutation, Node, TypeGraph, Variant, ViewCache, field_generation,
        indexed_field_generation,
    },
    symbols,
};

/// Syntax supported for scalar-compatible assignments in a particular context.
#[derive(Clone, Copy)]
pub enum SetterSyntax {
    Method,
    Property,
    PropertyWithGetter,
    UntaggedProperty,
}

/// A setter's final name and the common encoding mutation it invokes.
pub struct Setter {
    pub name: String,
    pub property: bool,
    pub mutation: Mutation,
}

/// Declarations contributed by one record field, enum payload, or root value.
pub struct Accessor {
    pub name: String,
    pub generation: ApiGeneration,
    pub getter: bool,
    pub setter: Option<Setter>,
    pub mutable: Option<String>,
    pub replace: Option<String>,
    pub cache: Option<ViewCache>,
    pub loader: Option<String>,
    pub helpers: FieldHelpers,
}

impl Accessor {
    pub fn field(plan: &TypeGraph, node: &Node, field: &Field, variant: Option<&Variant>) -> Self {
        let generation = field_generation(node, field);
        let values = field
            .options
            .value_accessors
            .or(node.options.value_accessors)
            .unwrap_or(true);
        let mut accessor = Self::new(
            plan,
            field.ty,
            |readonly| {
                variant.map_or_else(
                    || plan.backend.api.field_accessor(plan, field, readonly),
                    |variant| {
                        plan.backend
                            .api
                            .variant_accessor(plan, field, variant, readonly)
                    },
                )
            },
            field_value_generation(plan, field, generation, values),
            variant.is_some(),
            false,
        );
        accessor.helpers = FieldHelpers::field(plan, node, field, &accessor.name);
        plan.backend.api.customize_accessor(
            plan,
            &mut accessor,
            AccessorContext {
                ty: field.ty,
                exposure: field.options.exposure,
                variant,
                root: false,
            },
        );
        accessor
    }

    pub fn root(plan: &TypeGraph, id: Id) -> Result<Self, GenerateError> {
        let node = &plan.nodes[id];
        let name = symbols::runtime_symbol(plan, "value")?;
        let mut accessor = Self::new(
            plan,
            id,
            |_| name,
            mask_values(node.options.generation, node.options.values()),
            false,
            true,
        );
        if let Some(setter) = &mut accessor.setter
            && !setter.property
        {
            setter.name = symbols::runtime_symbol(plan, "set")?;
        }
        plan.backend.api.customize_accessor(
            plan,
            &mut accessor,
            AccessorContext {
                ty: id,
                exposure: node.options.exposure,
                variant: None,
                root: true,
            },
        );
        Ok(accessor)
    }

    fn new(
        plan: &TypeGraph,
        id: Id,
        name: impl FnOnce(bool) -> String,
        generation: ApiGeneration,
        tagged: bool,
        root: bool,
    ) -> Self {
        let getter = if root {
            generation.getters
        } else {
            needs_navigation_getter(plan, id, generation)
        };
        let mutation = generation.setters.then(|| plan.mutation(id)).flatten();
        let syntax = if root {
            plan.backend.root_setter
        } else {
            plan.backend.field_setter
        };
        let property = mutation.as_ref().is_some_and(|mutation| {
            mutation.property
                && match syntax {
                    SetterSyntax::Method => false,
                    SetterSyntax::Property => true,
                    SetterSyntax::PropertyWithGetter => generation.getters,
                    SetterSyntax::UntaggedProperty => !tagged,
                }
        });
        // Naming modifiers describe the emitted declaration, including whether
        // a setter shares its property or is a separate method.
        let name = name(!property);
        let setter = mutation.map(|mutation| {
            let setter_name = if property {
                name.clone()
            } else if root {
                String::new() // Resolved by `root` through the runtime naming role.
            } else {
                plan.backend.api.setter_name(plan, &name)
            };
            Setter {
                name: setter_name,
                property,
                mutation,
            }
        });
        let cache = (getter && !plan.backend.language.has_value_types() && plan.caches_view(id))
            .then(|| plan.view_cache(id, &name, tagged));
        Self {
            mutable: None,
            replace: None,
            name,
            generation,
            getter,
            setter,
            cache,
            loader: None,
            helpers: FieldHelpers::default(),
        }
    }

    /// Distinct declarations; a property getter/setter pair occupies one name.
    pub fn declarations(&self, read_only: bool) -> Vec<(&str, &'static str)> {
        let mut members = Vec::new();
        if self.getter || (!read_only && self.setter.as_ref().is_some_and(|setter| setter.property))
        {
            members.push((self.name.as_str(), "field accessor"));
        }
        if !read_only {
            if let Some(name) = &self.replace {
                members.push((name, "field replacement"));
            }
            if let Some(name) = &self.mutable {
                members.push((name, "field setter"));
            } else if let Some(setter) = &self.setter
                && !setter.property
            {
                members.push((&setter.name, "field setter"));
            }
        }
        if let Some(cache) = &self.cache {
            members.push((&cache.value, "cached child view"));
            if let Some(state) = &cache.state {
                members.push((state, "cached child discriminant"));
            }
        }
        if let Some(loader) = &self.loader {
            members.push((loader, "cached child loader"));
        }
        members.extend(self.helpers.declarations(read_only));
        members
    }
}

/// Nested mutable values need an access path even when value reads are omitted.
fn needs_navigation_getter(plan: &TypeGraph, id: Id, generation: ApiGeneration) -> bool {
    generation.getters || (generation.setters && plan.borrows_content(id))
}

fn mask_values(mut generation: ApiGeneration, enabled: bool) -> ApiGeneration {
    generation.getters &= enabled;
    generation.setters &= enabled;
    generation
}

/// Indexed collections need a navigation getter even when value access is
/// omitted. Keep the path to their helpers without enabling whole-value setters.
fn field_value_generation(
    plan: &TypeGraph,
    field: &Field,
    generation: ApiGeneration,
    values: bool,
) -> ApiGeneration {
    let mut selected = mask_values(generation, values);
    // Transparent reference-type wrappers expose their inner collection API.
    // Follow only that folding path; numeric projections have no indexed view.
    let mut collection = field;
    while !matches!(
        collection.options.exposure,
        intrlay_schema::Exposure::TypedArray(_)
    ) && let Some(inner) = plan.folded_field(collection.ty)
    {
        collection = inner;
    }
    let indexed = matches!(
        plan.nodes[collection.ty].kind,
        crate::plan::Kind::Array { .. } | crate::plan::Kind::Sequence(_)
    ) && matches!(
        collection.options.exposure,
        intrlay_schema::Exposure::Indexed | intrlay_schema::Exposure::IndexedOnly
    );
    selected.getters |= indexed && needs_navigation_getter(plan, field.ty, generation);
    selected
}

/// Element access shared by collection emission and target-specific name checks.
/// Backends choose named methods, operators, or language-defined special methods.
pub struct CollectionAccessors {
    pub getter: bool,
    pub setter: Option<Mutation>,
}

impl CollectionAccessors {
    pub fn new(plan: &TypeGraph, owner: &Node, element: Id) -> Self {
        Self {
            getter: owner.options.exposure != intrlay_schema::Exposure::IndexedOnly
                && owner.options.values()
                && needs_navigation_getter(plan, element, owner.options.generation),
            setter: (owner.options.generation.setters && owner.options.values())
                .then(|| plan.mutation(element))
                .flatten(),
        }
    }
}

/// Indexed field and numeric-lane methods for reference-language collections.
pub struct IndexedAccessors {
    pub getter: Option<String>,
    pub setter: Option<Setter>,
    pub helpers: FieldHelpers,
}

impl IndexedAccessors {
    pub fn new(
        plan: &TypeGraph,
        owner: &Node,
        element: &Node,
        field: &Field,
        variant: Option<&Variant>,
    ) -> Self {
        let generation = indexed_field_generation(owner, element, field);
        let name = variant.map_or_else(
            || plan.backend.api.field_accessor(plan, field, true),
            |variant| {
                plan.backend
                    .api
                    .variant_accessor(plan, field, variant, true)
            },
        );
        let method = |prefix, suffix| {
            plan.names
                .method(&format!("{prefix}_{name}{suffix}"), false)
        };
        let helpers = FieldHelpers::indexed(plan, owner, element, field, &name);
        let values = field
            .options
            .value_accessors
            .or(element.options.value_accessors)
            .or(owner.options.value_accessors)
            .unwrap_or(true);
        let generation = field_value_generation(plan, field, generation, values);
        Self {
            getter: needs_navigation_getter(plan, field.ty, generation).then(|| method("get", "")),
            setter: generation
                .setters
                .then(|| plan.mutation(field.ty))
                .flatten()
                .map(|mutation| Setter {
                    name: method("set", ""),
                    property: false,
                    mutation,
                }),
            helpers,
        }
    }

    /// Payload enums need a discriminant without constructing an element view.
    pub fn tag(plan: &TypeGraph, owner: &Node, element: &Node) -> Option<String> {
        (matches!(element.kind, crate::plan::Kind::Enum(_))
            && matches!(
                owner.options.exposure,
                intrlay_schema::Exposure::Indexed | intrlay_schema::Exposure::IndexedOnly
            )
            && owner.options.generation.getters
            && element.options.generation.getters)
            .then(|| plan.names.method("get_tag", false))
    }

    pub fn declarations(&self) -> impl Iterator<Item = (&str, &'static str)> {
        [
            (self.getter.as_deref(), "indexed field accessor"),
            (
                self.setter.as_ref().map(|setter| setter.name.as_str()),
                "indexed field accessor",
            ),
        ]
        .into_iter()
        .filter_map(|(name, role)| name.map(|name| (name, role)))
        .chain(self.helpers.declarations(false))
    }
}
