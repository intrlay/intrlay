//! Field storage ranges are independent of numeric or indexed projection.

use super::{Field, Kind, TypeGraph};

pub enum ByteRange {
    /// Inline value/header bytes, including padding but excluding indirect data.
    Inline { length: usize },
    /// Contiguous element headers reached through a sequence's relative pointer.
    Sequence { stride: usize },
}

impl TypeGraph {
    pub fn byte_range(&self, field: &Field) -> ByteRange {
        let mut id = field.ty;
        while let Some(inner) = self.folded_field(id) {
            id = inner.ty;
        }
        if let Kind::Sequence(element) = self.nodes[id].kind {
            ByteRange::Sequence {
                stride: self.nodes[element].layout.size(),
            }
        } else {
            ByteRange::Inline {
                length: self.nodes[field.ty].layout.size(),
            }
        }
    }
}
