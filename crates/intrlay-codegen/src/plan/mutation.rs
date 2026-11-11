//! Replace inline values, or follow stable allocations to a replaceable payload.

use intrlay_schema::Exposure;

use super::{Id, Kind, TypeGraph};

pub enum MutationStep {
    /// Stop if absent, otherwise advance to the existing inline payload.
    Optional(usize),
    /// Follow a record-relative offset to its existing allocation.
    Boxed,
    /// A transparent field's offset within its containing record.
    Inline(usize),
}

pub struct Mutation {
    /// Native input type of the replacement, after any pointer traversal.
    pub value: Id,
    pub steps: Vec<MutationStep>,
    /// Property syntax is possible when reads and writes expose the same value type.
    pub property: bool,
}

impl TypeGraph {
    pub fn mutation(&self, id: Id) -> Option<Mutation> {
        let node = &self.nodes[id];
        if !node.options.generation.setters {
            return None;
        }
        if self
            .folded_field(id)
            .is_some_and(|field| !field.options.generation.setters)
        {
            return None;
        }
        if !self.has_payload(id) {
            return Some(Mutation {
                value: node.codec,
                steps: Vec::new(),
                property: self.native_value(id),
            });
        }
        if let Some(field) = self.folded_field(id) {
            let mut result = self.mutation(field.ty)?;
            if field.offset != 0 {
                result.steps.insert(0, MutationStep::Inline(field.offset));
            }
            return Some(result);
        }
        let (child, step) = match node.kind {
            Kind::Optional { value, offset } => (value, MutationStep::Optional(offset)),
            Kind::Boxed(value) => (value, MutationStep::Boxed),
            _ => return None,
        };
        let mut result = self.mutation(child)?;
        result.steps.insert(0, step);
        result.property = false;
        Some(result)
    }

    pub fn native_value(&self, id: Id) -> bool {
        // Numeric projections read borrowed lanes even when their transparent
        // construction input is a scalar. They need a distinct setter method.
        if matches!(self.nodes[id].options.exposure, Exposure::TypedArray(_)) {
            return false;
        }
        if let Some(field) = self.folded_field(id) {
            return self.native_value(field.ty);
        }
        if self.nodes[id].is_value_enum() {
            return true;
        }
        match self.nodes[id].kind {
            Kind::Scalar(_) | Kind::Unit => true,
            Kind::Optional { value, .. } => self.native_value(value),
            _ => false,
        }
    }
}
