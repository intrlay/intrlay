use std::{
    any::{TypeId, type_name},
    collections::{BTreeMap, BTreeSet},
};

use crate::{Definition, Schema, Type};

/// Describes a concrete source type for external code generation.
///
/// Derives compose this trait through field types so Rust resolves aliases and
/// concrete generic parameters. Collection never runs on the object-exchange path.
pub trait Describe {
    /// Registers dependencies and returns this type's schema reference.
    fn describe(registry: &mut Registry) -> Type;
}

/// Collects definitions reachable from explicitly exported roots.
#[derive(Default)]
pub struct Registry {
    schema: Schema,
    visiting: BTreeSet<String>,
    identities: BTreeMap<String, TypeId>,
}

impl Registry {
    /// Creates an empty metadata collection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Exports a concrete source type under a public name.
    ///
    /// # Panics
    /// Panics if the same export name is assigned different types.
    pub fn export<T: Describe>(&mut self, name: impl Into<String>) {
        let ty = T::describe(self);
        let name = name.into();
        if let Some(previous) = self.schema.exports.insert(name, ty.clone()) {
            assert_eq!(previous, ty, "conflicting export name");
        }
    }

    /// Registers a definition once, permitting recursive references while building it.
    ///
    /// # Panics
    /// Panics if distinct concrete types share the same diagnostic type name.
    /// Rust does not guarantee uniqueness of `type_name`, but schema references
    /// must unambiguously identify a single definition.
    pub fn define<T: 'static>(&mut self, build: impl FnOnce(&mut Self) -> Definition) -> Type {
        let identity = type_name::<T>().to_owned();
        if let Some(previous) = self.identities.insert(identity.clone(), TypeId::of::<T>()) {
            assert_eq!(
                previous,
                TypeId::of::<T>(),
                "conflicting Rust type identity: {identity}"
            );
        }
        if !self.schema.definitions.contains_key(&identity)
            && self.visiting.insert(identity.clone())
        {
            let definition = build(self);
            self.schema.definitions.insert(identity.clone(), definition);
            self.visiting.remove(&identity);
        }
        Type::Named(identity)
    }

    /// Finishes metadata collection.
    #[must_use]
    pub fn finish(self) -> Schema {
        self.schema
    }
}
