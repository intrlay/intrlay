//! Member declarations shared by validation and target-specific source emission.
//!
//! These are API decisions, not a target-language syntax tree. Backends retain
//! their own method bodies and optimized load/store implementations.

mod policy;
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
}

impl Accessor {
    pub fn field(plan: &TypeGraph, node: &Node, field: &Field, variant: Option<&Variant>) -> Self {
        let name = variant.map_or_else(
            || field.property_name.clone(),
            |variant| plan.backend.api.variant_accessor(plan, field, variant),
        );
        let generation = field_generation(node, field);
        let mut accessor = Self::new(plan, field.ty, name, generation, variant.is_some(), false);
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
        let mut accessor = Self::new(
            plan,
            id,
            symbols::runtime_symbol(plan, "value")?,
            node.options.generation,
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
        name: String,
        generation: ApiGeneration,
        tagged: bool,
        root: bool,
    ) -> Self {
        let getter =
            generation.getters || (!root && generation.setters && plan.borrows_content(id));
        let mutation = generation.setters.then(|| plan.mutation(id)).flatten();
        let setter = mutation.map(|mutation| {
            let syntax = if root {
                plan.backend.root_setter
            } else {
                plan.backend.field_setter
            };
            let property = mutation.property
                && match syntax {
                    SetterSyntax::Method => false,
                    SetterSyntax::Property => true,
                    SetterSyntax::PropertyWithGetter => generation.getters,
                    SetterSyntax::UntaggedProperty => !tagged,
                };
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
        members
    }
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
            getter: owner.options.generation.getters
                || (owner.options.generation.setters && plan.borrows_content(element)),
            setter: owner
                .options
                .generation
                .setters
                .then(|| plan.mutation(element))
                .flatten(),
        }
    }
}

/// Indexed field and numeric-lane methods for reference-language collections.
pub struct IndexedAccessors {
    pub getter: Option<String>,
    pub setter: Option<Setter>,
    pub component_getter: Option<String>,
    pub component_setter: Option<String>,
}

impl IndexedAccessors {
    pub fn new(plan: &TypeGraph, owner: &Node, element: &Node, field: &Field) -> Self {
        let generation = indexed_field_generation(owner, element, field);
        let method = |prefix, suffix| {
            plan.names
                .method(&format!("{prefix}_{}{suffix}", field.property_name), false)
        };
        let component = plan.component_scalar(field).is_some();
        Self {
            getter: generation.getters.then(|| method("get", "")),
            setter: generation
                .setters
                .then(|| plan.mutation(field.ty))
                .flatten()
                .map(|mutation| Setter {
                    name: method("set", ""),
                    property: false,
                    mutation,
                }),
            component_getter: (component && generation.getters)
                .then(|| method("get", "_component")),
            component_setter: (component && generation.setters)
                .then(|| method("set", "_component")),
        }
    }

    pub fn declarations(&self) -> impl Iterator<Item = (&str, &'static str)> {
        [
            (self.getter.as_deref(), "indexed field accessor"),
            (
                self.setter.as_ref().map(|setter| setter.name.as_str()),
                "indexed field accessor",
            ),
            (
                self.component_getter.as_deref(),
                "indexed numeric component accessor",
            ),
            (
                self.component_setter.as_deref(),
                "indexed numeric component accessor",
            ),
        ]
        .into_iter()
        .filter_map(|(name, role)| name.map(|name| (name, role)))
    }
}
