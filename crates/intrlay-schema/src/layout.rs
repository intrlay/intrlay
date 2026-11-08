use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use intrlay_layout::{Layout, field_offset, record, union};

use crate::{DefinitionKind, Schema, Type};

/// A schema error detected before emitting or compiling payload access code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutError(pub String);

impl fmt::Display for LayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl Error for LayoutError {}

/// Fixed layout and field offsets for a record or an enum's payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordLayout {
    /// Size and alignment including trailing padding.
    pub layout: Layout,
    /// Byte offset of each field, in declaration order.
    pub fields: Vec<usize>,
}

/// Resolved fixed enum layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariantLayout {
    /// Size and alignment of the complete tag and union.
    pub layout: Layout,
    /// Offset of the payload from the enum header.
    pub payload_offset: usize,
    /// Layout of each variant's fields, in declaration order.
    pub variants: Vec<RecordLayout>,
}

/// Resolves schemas using the same const layout functions used by Rust derives.
/// Completed named layouts are retained for subsequent queries on this schema.
pub struct Layouts<'a> {
    schema: &'a Schema,
    headers: BTreeMap<String, Layout>,
    buffer_alignments: BTreeMap<String, usize>,
}

impl<'a> Layouts<'a> {
    /// Creates a resolver without touching application data.
    #[must_use]
    pub const fn new(schema: &'a Schema) -> Self {
        Self {
            schema,
            headers: BTreeMap::new(),
            buffer_alignments: BTreeMap::new(),
        }
    }

    /// Computes the fixed header layout of any schema type.
    ///
    /// # Errors
    /// Reports missing definitions, inline recursion, or invalid alignment.
    pub fn of(&mut self, ty: &Type) -> Result<Layout, LayoutError> {
        self.resolve(ty, &mut BTreeSet::new())
    }

    /// Computes a record's fixed layout and declaration-ordered offsets.
    ///
    /// # Errors
    /// Reports invalid field types or record alignment.
    pub fn record(
        &mut self,
        fields: &[crate::Field],
        alignment: usize,
    ) -> Result<RecordLayout, LayoutError> {
        self.record_inner(fields, alignment, &mut BTreeSet::new())
    }

    /// Computes a tagged enum's union and each variant's field offsets.
    ///
    /// # Errors
    /// Reports empty enums, duplicate tags, or invalid payload layouts.
    pub fn variants(
        &mut self,
        variants: &[crate::Variant],
        alignment: usize,
    ) -> Result<VariantLayout, LayoutError> {
        self.variants_inner(variants, alignment, &mut BTreeSet::new())
    }

    /// Computes the maximum alignment of a header and all reachable payload regions.
    ///
    /// # Errors
    /// Reports missing or invalid type definitions.
    pub fn buffer_alignment(&mut self, ty: &Type) -> Result<usize, LayoutError> {
        let alignment = self.alignment_inner(ty, &mut BTreeSet::new())?;
        if let Type::Named(identity) = ty {
            // Cache only completed root queries. A recursive payload may revisit
            // an ancestor before all of that ancestor's fields have been visited.
            self.buffer_alignments.insert(identity.clone(), alignment);
        }
        Ok(alignment)
    }

    /// Checks the entire exported graph and returns its named header layouts.
    ///
    /// # Errors
    /// Reports invalid definitions, unresolved references, and duplicate fields/tags.
    pub fn validate(&mut self) -> Result<BTreeMap<String, Layout>, LayoutError> {
        let mut layouts = BTreeMap::new();
        for (identity, definition) in &self.schema.definitions {
            if definition.transparent
                && !matches!(&definition.kind, DefinitionKind::Record(fields) if fields.len() == 1)
            {
                return Err(LayoutError(format!(
                    "transparent requires exactly one field: {}",
                    definition.name
                )));
            }
            let ty = Type::Named(identity.clone());
            layouts.insert(identity.clone(), self.of(&ty)?);
            self.buffer_alignment(&ty)?;
        }
        for ty in self.schema.exports.values() {
            self.of(ty)?;
            self.buffer_alignment(ty)?;
        }
        Ok(layouts)
    }

    fn resolve(
        &mut self,
        ty: &Type,
        visiting: &mut BTreeSet<String>,
    ) -> Result<Layout, LayoutError> {
        Ok(match ty {
            Type::Scalar(scalar) => scalar.layout(),
            Type::Unit => Layout::UNIT,
            Type::String | Type::Sequence(_) => Layout::SEQUENCE,
            Type::Boxed(_) => Layout::BOX,
            Type::Array { element, length } => {
                let element = self.resolve(element, visiting)?;
                if element.size().checked_mul(*length).is_none() {
                    return Err(LayoutError("array size overflow".into()));
                }
                element.array(*length)
            }
            Type::Optional(value) => {
                Layout::tagged(Layout::new(1, 1), self.resolve(value, visiting)?)
            }
            Type::Named(identity) => {
                if let Some(layout) = self.headers.get(identity) {
                    return Ok(*layout);
                }
                if !visiting.insert(identity.clone()) {
                    return Err(LayoutError(format!("inline recursive type: {identity}")));
                }
                let definition = self
                    .schema
                    .definitions
                    .get(identity)
                    .ok_or_else(|| LayoutError(format!("undefined type: {identity}")))?;
                let layout = match &definition.kind {
                    DefinitionKind::Record(fields) => {
                        self.record_inner(fields, definition.alignment, visiting)?
                            .layout
                    }
                    DefinitionKind::Enum(variants) => {
                        self.variants_inner(variants, definition.alignment, visiting)?
                            .layout
                    }
                };
                visiting.remove(identity);
                self.headers.insert(identity.clone(), layout);
                layout
            }
        })
    }

    fn record_inner(
        &mut self,
        fields: &[crate::Field],
        alignment: usize,
        visiting: &mut BTreeSet<String>,
    ) -> Result<RecordLayout, LayoutError> {
        if !alignment.is_power_of_two() {
            return Err(LayoutError(format!("invalid alignment: {alignment}")));
        }
        let mut names = BTreeSet::new();
        let mut layouts = Vec::with_capacity(fields.len());
        for field in fields {
            if !names.insert(&field.name) {
                return Err(LayoutError(format!("duplicate field: {}", field.name)));
            }
            layouts.push(self.resolve(&field.ty, visiting)?);
        }
        Ok(RecordLayout {
            layout: record(&layouts, alignment),
            fields: (0..layouts.len())
                .map(|i| field_offset(&layouts, i))
                .collect(),
        })
    }

    fn variants_inner(
        &mut self,
        variants: &[crate::Variant],
        alignment: usize,
        visiting: &mut BTreeSet<String>,
    ) -> Result<VariantLayout, LayoutError> {
        if variants.is_empty() {
            return Err(LayoutError("uninhabited enums cannot be exchanged".into()));
        }
        if !alignment.is_power_of_two() {
            return Err(LayoutError(format!("invalid alignment: {alignment}")));
        }
        let mut tags = BTreeSet::new();
        let mut payloads = Vec::with_capacity(variants.len());
        for variant in variants {
            if variant.synthetic_payload && variant.fields.len() != 1 {
                return Err(LayoutError(format!(
                    "synthetic variant payload requires exactly one field: {}",
                    variant.name
                )));
            }
            if !tags.insert(variant.tag) {
                return Err(LayoutError(format!("duplicate enum tag: {}", variant.tag)));
            }
            payloads.push(self.record_inner(&variant.fields, 1, visiting)?);
        }
        let payload = union(
            &payloads
                .iter()
                .map(|variant| variant.layout)
                .collect::<Vec<_>>(),
        );
        let parts = [Layout::new(4, 4), payload];
        Ok(VariantLayout {
            layout: record(&parts, alignment),
            payload_offset: field_offset(&parts, 1),
            variants: payloads,
        })
    }

    fn alignment_inner(
        &mut self,
        ty: &Type,
        visiting: &mut BTreeSet<String>,
    ) -> Result<usize, LayoutError> {
        if let Type::Named(identity) = ty {
            if let Some(alignment) = self.buffer_alignments.get(identity) {
                return Ok(*alignment);
            }
            if !visiting.insert(identity.clone()) {
                return Ok(1);
            }
        }
        let mut alignment = self.of(ty)?.alignment();
        match ty {
            Type::Array { element, .. }
            | Type::Sequence(element)
            | Type::Boxed(element)
            | Type::Optional(element) => {
                alignment = alignment.max(self.alignment_inner(element, visiting)?);
            }
            Type::Named(identity) => {
                let definition = self
                    .schema
                    .definitions
                    .get(identity)
                    .ok_or_else(|| LayoutError(format!("undefined type: {identity}")))?;
                match &definition.kind {
                    DefinitionKind::Record(fields) => {
                        for field in fields {
                            alignment = alignment.max(self.alignment_inner(&field.ty, visiting)?);
                        }
                    }
                    DefinitionKind::Enum(variants) => {
                        for variant in variants {
                            for field in &variant.fields {
                                alignment =
                                    alignment.max(self.alignment_inner(&field.ty, visiting)?);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(alignment)
    }
}
