//! Numeric construction inputs share wire layouts with structured values.
//! Reserve each input context once so recursive schemas and field overrides keep
//! stable IDs without expanding every path through the type graph.

use std::collections::BTreeMap;

use intrlay_schema::{Exposure, Scalar};

use crate::GenerateError;

use super::{Id, Kind, Options, TypeGraph};

#[derive(Clone, Copy)]
pub struct NumericInput {
    pub scalar: Scalar,
    /// Number of scalar lanes, including padding, per fixed value or sequence element.
    pub lanes: usize,
    pub sequence: bool,
}

impl TypeGraph {
    /// A zeroed sequence reserves elements, while numeric input counts scalar lanes.
    pub fn numeric_input_zeroable(&self, node: &super::Node) -> bool {
        match node.kind {
            Kind::Sequence(element) => self.zeroable(element),
            Kind::Array { .. } => node.properties.zeroable,
            _ => false,
        }
    }
}

fn numeric(options: &Options) -> Option<Scalar> {
    match options.exposure {
        Exposure::TypedArray(scalar) if options.explicit_exposure => Some(scalar),
        _ => None,
    }
}

impl TypeGraph {
    pub(super) fn project_inputs(self) -> Result<Self, GenerateError> {
        if self.backend.language.has_value_types() {
            return Ok(self);
        }
        let mut builder = InputBuilder {
            source: &self,
            plan: self.clone(),
            contexts: self
                .nodes
                .iter()
                .enumerate()
                .map(|(id, node)| ((id, numeric(&node.options)), id))
                .collect(),
        };
        for id in 0..self.nodes.len() {
            builder.populate(id, id, numeric(&self.nodes[id].options))?;
        }
        Ok(builder.plan)
    }
}

struct InputBuilder<'a> {
    source: &'a TypeGraph,
    plan: TypeGraph,
    contexts: BTreeMap<(Id, Option<Scalar>), Id>,
}

impl InputBuilder<'_> {
    fn project(&mut self, source: Id, scalar: Option<Scalar>) -> Result<Id, GenerateError> {
        if let Some(&id) = self.contexts.get(&(source, scalar)) {
            return Ok(id);
        }
        let id = self.plan.nodes.len();
        let mut node = self.source.nodes[source].clone();
        node.codec = id;
        let suffix = if scalar.is_some() {
            "Numeric"
        } else {
            "Fields"
        };
        node.name.push_str(suffix);
        node.view_name.push_str(suffix);
        // Empty layouts admit any lane type. Nonempty layouts already determine
        // their scalar, but empty contexts need it to distinguish their helpers.
        let element = if let Kind::Sequence(element) = node.kind {
            element
        } else {
            source
        };
        if let Some(scalar) = scalar
            && matches!(
                self.source.nodes[element].properties.homogeneous,
                super::properties::Homogeneous::Empty
            )
        {
            let scalar = format!("{scalar:?}");
            node.name.push_str(&scalar);
            node.view_name.push_str(&scalar);
        }
        node.options.exposure = scalar.map_or(Exposure::Fields, Exposure::TypedArray);
        node.options.explicit_exposure = true;
        node.options.generation.create = false;
        self.plan.nodes.push(node);
        self.contexts.insert((source, scalar), id);
        self.populate(source, id, scalar)?;
        Ok(id)
    }

    fn populate(
        &mut self,
        source: Id,
        target: Id,
        scalar: Option<Scalar>,
    ) -> Result<(), GenerateError> {
        let node = &self.source.nodes[source];
        if let Some(scalar) = scalar {
            let element = if let Kind::Sequence(element) = node.kind {
                element
            } else {
                source
            };
            self.plan.nodes[target].numeric_input = Some(NumericInput {
                scalar,
                lanes: self.source.nodes[element].layout.size() / scalar.layout().size(),
                sequence: element != source,
            });
        }
        // Retain fields for read-side projection; only their encoding contexts change.
        let mut kind = node.kind.clone();
        let fields = match &mut kind {
            Kind::Record(fields) => fields.iter_mut().collect::<Vec<_>>(),
            Kind::Enum(variants) => variants
                .iter_mut()
                .flat_map(|variant| &mut variant.fields)
                .collect(),
            _ => Vec::new(),
        };
        for field in fields {
            let options = if field.options.explicit_exposure {
                &field.options
            } else {
                &self.source.nodes[field.ty].options
            };
            field.ty = self.project(field.ty, numeric(options))?;
        }
        self.plan.nodes[target].kind = kind;
        Ok(())
    }
}
