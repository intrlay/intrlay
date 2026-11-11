//! Target-owned API decisions used by both name validation and source emission.

use intrlay_schema::Exposure;

use crate::{
    names::Names,
    plan::{Field, Id, TypeGraph, Variant},
};

use super::Accessor;

/// The declaration spelling and source identity have already been resolved.
pub enum ViewName<'a> {
    Named { source: &'a str, explicit: bool },
    Structural { label: &'a str },
}

/// Placement and representation of the value described by an accessor.
pub struct AccessorContext<'a> {
    pub ty: Id,
    pub exposure: Exposure,
    pub variant: Option<&'a Variant>,
    pub root: bool,
}

/// Language syntax and naming choices do not change the encoded type graph.
/// Backends override only the decisions that differ from the common API.
pub trait ApiPolicy: Sync {
    fn view_name(&self, names: &Names, native_name: &str, source: ViewName<'_>) -> String {
        match source {
            ViewName::Named { explicit: true, .. } => format!("{native_name}View"),
            ViewName::Named {
                source,
                explicit: false,
            } => names.type_name(&format!("{source}_view")),
            ViewName::Structural { label } => names.type_name(&format!("Intrlay{label}_view")),
        }
    }

    fn field_accessor(&self, plan: &TypeGraph, field: &Field, readonly: bool) -> String {
        field
            .explicit_name
            .clone()
            .unwrap_or_else(|| plan.names.property(&field.source_name, readonly))
    }

    /// Names a buffer property independently of the input's field naming policy.
    fn variant_accessor(
        &self,
        plan: &TypeGraph,
        field: &Field,
        variant: &Variant,
        readonly: bool,
    ) -> String {
        if variant.synthetic_payload && !field.flattened {
            plan.names.property(variant.name_source(), readonly)
        } else {
            plan.names.property(
                &format!("{}_{}", variant.name_source(), field.name_source()),
                readonly,
            )
        }
    }

    fn setter_name(&self, plan: &TypeGraph, name: &str) -> String {
        plan.names.method(&format!("set_{name}"), false)
    }

    fn cache_field(&self, names: &Names, name: &str) -> String {
        names.symbol(
            name,
            crate::SymbolKind::Field,
            Some(crate::Visibility::Private),
            &[],
        )
    }

    /// Adds target-only declarations after the common getter/setter/cache decisions.
    fn customize_accessor(
        &self,
        _plan: &TypeGraph,
        _accessor: &mut Accessor,
        _context: AccessorContext<'_>,
    ) {
    }
}

pub struct DefaultApi;
impl ApiPolicy for DefaultApi {}
