//! Auxiliary field APIs share one emission policy across ordinary and indexed
//! fields. Byte ranges expose wire storage; components expose numeric lanes.

use intrlay_schema::ApiGeneration;

use crate::plan::{
    ByteRange, Field, Node, Options, TypeGraph, field_generation, indexed_field_generation,
};

#[derive(Default)]
pub struct FieldHelpers {
    pub component_getter: Option<String>,
    pub component_setter: Option<String>,
    pub byte_offset: Option<String>,
    pub byte_length: Option<String>,
}

impl FieldHelpers {
    pub fn range_docs(plan: &TypeGraph, field: &Field, indexed: bool, offset: bool) -> String {
        let docs = if indexed {
            TypeGraph::indexed_field_docs(field, false)
        } else {
            field.docs.clone()
        };
        let note = match (plan.byte_range(field), offset) {
            (ByteRange::Sequence { .. }, true) => {
                "Byte offset of the first array element, relative to the first byte of Storage. Follows the sequence pointer without creating a view. An inactive enum variant returns the absent result."
            }
            (ByteRange::Sequence { .. }, false) => {
                "Byte length of the contiguous element region: count times wire stride, including element padding. Indirect element payloads are outside this range. An inactive enum variant returns the absent result."
            }
            (ByteRange::Inline { .. }, true) => {
                "Byte offset of the field's inline wire representation, relative to the first byte of Storage. Does not follow pointers or test variant tags or optional presence."
            }
            (ByteRange::Inline { .. }, false) => {
                "Byte length of the field's inline wire representation, including padding. Referenced payloads are outside this range."
            }
        };
        crate::documentation::with_note(&docs, note)
    }

    pub fn component_docs(field: &Field, indexed: bool) -> String {
        if indexed {
            TypeGraph::indexed_field_docs(field, true)
        } else {
            crate::documentation::with_note(
                &field.docs,
                "The component index addresses scalar lanes, including padding, and must be within the numeric projection's length.",
            )
        }
    }

    pub fn field(plan: &TypeGraph, owner: &Node, field: &Field, name: &str) -> Self {
        Self::new(
            plan,
            field,
            name,
            field_generation(owner, field),
            &[&field.options, &owner.options],
            false,
        )
    }

    pub fn indexed(
        plan: &TypeGraph,
        owner: &Node,
        element: &Node,
        field: &Field,
        name: &str,
    ) -> Self {
        Self::new(
            plan,
            field,
            name,
            indexed_field_generation(owner, element, field),
            &[&field.options, &element.options, &owner.options],
            true,
        )
    }

    fn new(
        plan: &TypeGraph,
        field: &Field,
        name: &str,
        generation: ApiGeneration,
        policies: &[&Options],
        indexed: bool,
    ) -> Self {
        if !plan.backend.language.supports_accessor_generation() {
            return Self::default();
        }
        let components = policies
            .iter()
            .find_map(|policy| policy.component_accessors)
            .unwrap_or(false)
            && plan.component_scalar(field).is_some();
        let ranges = policies
            .iter()
            .find_map(|policy| policy.byte_range_accessors)
            .unwrap_or(false);
        let range_name = |suffix| {
            if indexed {
                plan.names.method(&format!("get_{name}_{suffix}"), false)
            } else {
                plan.names.property(&format!("{name}_{suffix}"), true)
            }
        };
        Self {
            component_getter: (components && generation.getters)
                .then(|| plan.names.method(&format!("get_{name}_component"), false)),
            component_setter: (components && generation.setters)
                .then(|| plan.names.method(&format!("set_{name}_component"), false)),
            byte_offset: (ranges && generation.getters).then(|| range_name("byte_offset")),
            byte_length: (ranges && generation.getters).then(|| range_name("byte_length")),
        }
    }

    pub fn declarations(&self, read_only: bool) -> impl Iterator<Item = (&str, &'static str)> {
        [
            (self.component_getter.as_deref(), "numeric component getter"),
            (
                self.component_setter.as_deref().filter(|_| !read_only),
                "numeric component setter",
            ),
            (self.byte_offset.as_deref(), "field byte offset"),
            (self.byte_length.as_deref(), "field byte length"),
        ]
        .into_iter()
        .filter_map(|(name, role)| name.map(|name| (name, role)))
    }
}
