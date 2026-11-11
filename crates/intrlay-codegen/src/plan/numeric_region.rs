//! Locate the contiguous numeric payload without constructing its language view.

use intrlay_schema::{Exposure, Scalar};

use super::{Field, Kind, TypeGraph};

pub enum RegionStep {
    /// Check presence, then advance to the inline payload.
    Optional(usize),
    /// Follow the relative pointer at the current location.
    Boxed,
}

pub struct NumericRegion {
    pub scalar: Scalar,
    pub steps: Vec<RegionStep>,
    pub sequence: bool,
}

impl NumericRegion {
    pub fn optional(&self) -> bool {
        self.steps
            .iter()
            .any(|step| matches!(step, RegionStep::Optional(_)))
    }
}

impl TypeGraph {
    pub fn numeric_region(&self, field: &Field) -> Option<NumericRegion> {
        if !self.backend.native_numeric_array_views {
            return None;
        }
        let mut id = field.ty;
        let mut exposure = field.options.exposure;
        let mut steps = Vec::new();
        loop {
            if let Exposure::TypedArray(scalar) = exposure {
                let sequence = matches!(self.nodes[id].kind, Kind::Sequence(_));
                return Some(NumericRegion {
                    scalar,
                    steps,
                    sequence,
                });
            }
            if let Some(field) = self.folded_field(id) {
                id = field.ty;
                exposure = field.options.exposure;
                continue;
            }
            id = match self.nodes[id].kind {
                Kind::Optional { value, offset } => {
                    steps.push(RegionStep::Optional(offset));
                    value
                }
                Kind::Boxed(value) => {
                    steps.push(RegionStep::Boxed);
                    value
                }
                _ => return None,
            };
            exposure = self.nodes[id].options.exposure;
        }
    }
}
