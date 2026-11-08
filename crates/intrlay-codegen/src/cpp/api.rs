use crate::{
    api::{Accessor, AccessorContext, ApiPolicy},
    plan::{Field, TypeGraph, Variant},
};

pub(super) struct Api;

impl ApiPolicy for Api {
    fn variant_accessor(&self, plan: &TypeGraph, field: &Field, variant: &Variant) -> String {
        if variant.synthetic_payload {
            plan.names.member(&format!("as_{}", variant.name))
        } else {
            plan.names
                .member(&format!("{}_{}", variant.name, field.name))
        }
    }

    fn setter_name(&self, _plan: &TypeGraph, name: &str) -> String {
        format!("set_{name}")
    }

    fn customize_accessor(
        &self,
        plan: &TypeGraph,
        accessor: &mut Accessor,
        context: AccessorContext<'_>,
    ) {
        if let Some(variant) = context.variant.filter(|variant| variant.synthetic_payload)
            && let Some(setter) = &mut accessor.setter
        {
            setter.name = plan.names.method(&format!("set_{}", variant.name), false);
        }
    }
}
