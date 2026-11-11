use intrlay_schema::NameCase;

use crate::{
    api::{Accessor, AccessorContext, ApiPolicy, ViewName},
    names::Names,
    plan::{Field, TypeGraph, Variant},
};

pub(super) struct Api;

impl ApiPolicy for Api {
    fn view_name(&self, _names: &Names, native_name: &str, _source: ViewName<'_>) -> String {
        // The derive appends View to the emitted native declaration name.
        format!("{native_name}View")
    }

    fn field_accessor(&self, _plan: &TypeGraph, field: &Field, _readonly: bool) -> String {
        // The derive uses the name emitted on the Rust declaration.
        field.property_name.clone()
    }

    fn variant_accessor(
        &self,
        _plan: &TypeGraph,
        field: &Field,
        variant: &Variant,
        _readonly: bool,
    ) -> String {
        let name = NameCase::SnakeCase.apply(&variant.name);
        if variant.synthetic_payload {
            format!("{name}_field_0")
        } else {
            format!("{name}_{}", field.property_name)
        }
    }

    fn customize_accessor(
        &self,
        _plan: &TypeGraph,
        accessor: &mut Accessor,
        context: AccessorContext<'_>,
    ) {
        if !context.root && accessor.generation.setters {
            accessor.mutable = Some(format!("{}_mut", accessor.name));
            accessor.replace = Some(format!("replace_{}", accessor.name));
        }
        if context.variant.is_some() {
            // Derive reads enum payloads through variant(), not individual getters.
            accessor.getter = false;
        }
    }
}
