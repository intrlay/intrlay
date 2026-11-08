use crate::{
    api::{ApiPolicy, DefaultApi, ViewName},
    names::Names,
    plan::{Field, TypeGraph, Variant},
};

pub(super) struct Api;

impl ApiPolicy for Api {
    fn view_name(&self, names: &Names, native_name: &str, source: ViewName<'_>) -> String {
        match source {
            ViewName::Structural { label } => names.type_name(&format!("{label}_view")),
            ViewName::Named { .. } => DefaultApi.view_name(names, native_name, source),
        }
    }

    fn variant_accessor(&self, plan: &TypeGraph, field: &Field, variant: &Variant) -> String {
        if variant.synthetic_payload {
            plan.names.property(&variant.name, true)
        } else {
            plan.names
                .property(&format!("{}_{}", variant.name, field.name), true)
        }
    }
}
