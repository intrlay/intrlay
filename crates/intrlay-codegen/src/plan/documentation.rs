//! Preserve the meaning of declarations whose public shape is projected away.
//! This stage composes plain text; each backend only formats its documentation.

use std::{collections::BTreeSet, fmt::Write};

use intrlay_schema::{Exposure, Scalar};

use super::{Field, Id, Kind, Node, TypeGraph};
use crate::documentation::{indexed, indexed_component, with_note};

/// Descriptions shared by several flattened accessors, in source order.
#[derive(Default)]
pub(super) struct Contexts {
    entries: Vec<(String, String)>,
    seen: BTreeSet<(String, String)>,
}

impl Contexts {
    pub fn insert(&mut self, path: &str, text: &str) {
        if !text.is_empty() {
            let entry = (path.to_owned(), text.to_owned());
            if self.seen.insert(entry.clone()) {
                self.entries.push(entry);
            }
        }
    }

    pub fn document(&self, text: &str) -> String {
        self.entries
            .iter()
            .fold(text.to_owned(), |docs, (path, text)| {
                with_note(&docs, &format!("{path}:\n{text}"))
            })
    }
}

impl TypeGraph {
    /// Numeric aliases and their containing input fields must not rely on a
    /// separate record input being emitted to explain the packed representation.
    pub(super) fn document_inputs(&mut self) {
        if self.backend.language.has_value_types() {
            return;
        }
        let descriptions: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(id, node)| {
                let docs = if let Some(numeric) = node.numeric_input {
                    self.numeric_documentation(id, numeric.scalar)
                } else {
                    node.docs.clone()
                };
                let mut contexts = Contexts::default();
                let fields = node
                    .fields()
                    .map(|(field, _)| self.projected_field_docs(field, &mut contexts))
                    .collect::<Vec<_>>();
                (contexts.document(&docs), fields)
            })
            .collect();
        for (node, (docs, fields)) in self.nodes.iter_mut().zip(descriptions) {
            node.docs = docs;
            for (field, docs) in fields_mut(node).into_iter().zip(fields) {
                field.docs = docs;
            }
        }
    }

    /// Collection views carry element/variant descriptions even when indexed-only
    /// access and reader-only generation omit every declaration of the element.
    pub(super) fn document_collections(&mut self, emitted: &BTreeSet<Id>) {
        let descriptions: Vec<_> = emitted
            .iter()
            .filter_map(|&id| {
                let node = &self.nodes[id];
                if !matches!(
                    node.options.exposure,
                    Exposure::Indexed | Exposure::IndexedOnly
                ) {
                    return None;
                }
                let (Kind::Array { element, .. } | Kind::Sequence(element)) = node.kind else {
                    return None;
                };
                let child = &self.nodes[element];
                let mut contexts = Contexts::default();
                contexts.insert(&format!("Element type {}", child.source_name), &child.docs);
                if let Kind::Enum(variants) = &child.kind {
                    for variant in variants {
                        contexts.insert(
                            &format!("Variant {}.{}", child.source_name, variant.source_name),
                            &variant.docs,
                        );
                    }
                }
                Some((id, contexts.document(&node.docs)))
            })
            .collect();
        for (id, docs) in descriptions {
            self.nodes[id].docs = docs;
        }
    }

    pub(super) fn projected_field_docs(&self, field: &Field, contexts: &mut Contexts) -> String {
        let mut docs = field.docs.clone();
        if field.flattened {
            docs = source_note(&docs, field);
        }
        if let Some((id, scalar)) = self.numeric_projection(field.ty, field.options.exposure) {
            let label = format!("Numeric layout for {}", field.source_path);
            contexts.insert(&label, &self.numeric_documentation(id, scalar));
            docs = with_note(&docs, &format!("{label}; see the containing declaration."));
        }
        docs
    }

    /// Indexed field helpers retain their source path as well as the existing
    /// index/component preconditions. Flattened fields already carry that path.
    pub fn indexed_field_docs(field: &Field, component: bool) -> String {
        let docs = if field.flattened {
            field.docs.clone()
        } else {
            source_note(&field.docs, field)
        };
        if component {
            indexed_component(&docs)
        } else {
            indexed(&docs)
        }
    }

    fn numeric_projection(&self, mut id: Id, mut exposure: Exposure) -> Option<(Id, Scalar)> {
        loop {
            if let Exposure::TypedArray(scalar) = exposure {
                return Some((id, scalar));
            }
            id = if let Some(field) = self.folded_field(id) {
                exposure = field.options.exposure;
                field.ty
            } else if let Kind::Optional { value, .. } | Kind::Boxed(value) = self.nodes[id].kind {
                exposure = self.nodes[value].options.exposure;
                value
            } else {
                return None;
            };
        }
    }

    fn numeric_documentation(&self, id: Id, scalar: Scalar) -> String {
        let width = scalar.layout().size();
        let sequence = matches!(self.nodes[id].kind, Kind::Sequence(_));
        let root = if let Kind::Sequence(element) = self.nodes[id].kind {
            element
        } else {
            id
        };
        let layout = self.nodes[root].layout;
        let count = layout.size() / width;
        let unit = if sequence {
            "element stride"
        } else {
            "value length"
        };
        let mut text = with_note(
            &self.nodes[root].docs,
            &format!(
                "Numeric projection: {scalar:?} scalar lanes; {unit} {count} lanes ({} bytes).\nLane ranges below are half-open and relative to each described layout.",
                layout.size()
            ),
        );
        let mut pending = vec![root];
        let mut visited = BTreeSet::new();
        let mut index = 0;
        // Reuse named layouts and describe repeated dimensions once, regardless
        // of the array length or the number of paths through a shared subtype.
        while let Some(&id) = pending.get(index) {
            index += 1;
            if !visited.insert(id) {
                continue;
            }
            let node = &self.nodes[id];
            let mut block = format!(
                "Layout {} ({} lanes):",
                node.source_name,
                node.layout.size() / width
            );
            if id != root && !node.docs.is_empty() {
                block = with_note(&block, &node.docs);
            }
            let mut end = 0;
            match &node.kind {
                Kind::Record(fields) => {
                    for field in fields {
                        let start = field.offset / width;
                        padding(&mut block, end, start);
                        end = start + self.nodes[field.ty].layout.size() / width;
                        let _ = write!(block, "\n- {}: lanes [{start}, {end}).", field.source_path);
                        if !matches!(self.nodes[field.ty].kind, Kind::Scalar(_) | Kind::Unit) {
                            let _ = write!(block, " Layout: {}.", self.nodes[field.ty].source_name);
                            pending.push(field.ty);
                        }
                        append_description(&mut block, &field.docs);
                    }
                }
                Kind::Array { element, length } => {
                    let child = &self.nodes[*element];
                    let _ = write!(
                        block,
                        "\n{length} elements; element stride {} lanes. Element layout: {}.",
                        child.layout.size() / width,
                        child.source_name
                    );
                    end = node.layout.size() / width;
                    if *length != 0 && !matches!(child.kind, Kind::Scalar(_) | Kind::Unit) {
                        pending.push(*element);
                    }
                }
                Kind::Enum(variants) => {
                    block.push_str("\n- Tag: lane 0.");
                    end = 1;
                    for variant in variants {
                        let _ = write!(block, "\n- {} = {}.", variant.source_name, variant.tag);
                        append_description(&mut block, &variant.docs);
                    }
                }
                Kind::Scalar(_) => {
                    block.push_str("\n- Value: lane 0.");
                    end = 1;
                }
                Kind::Unit => {}
                _ => unreachable!("numeric projections contain only fixed homogeneous layouts"),
            }
            padding(&mut block, end, node.layout.size() / width);
            text = with_note(&text, &block);
        }
        text
    }
}

fn source_note(docs: &str, field: &Field) -> String {
    with_note(docs, &format!("Source: {}.", field.source_path))
}

fn fields_mut(node: &mut Node) -> Vec<&mut Field> {
    match &mut node.kind {
        Kind::Record(fields) => fields.iter_mut().collect(),
        Kind::Enum(variants) => variants
            .iter_mut()
            .flat_map(|variant| &mut variant.fields)
            .collect(),
        _ => Vec::new(),
    }
}

fn append_description(text: &mut String, docs: &str) {
    if !docs.is_empty() {
        text.push('\n');
        for line in docs.lines() {
            text.push_str("  ");
            text.push_str(line);
            text.push('\n');
        }
        text.pop();
    }
}

fn padding(text: &mut String, start: usize, end: usize) {
    if start < end {
        let _ = write!(text, "\n- Padding: lanes [{start}, {end}).");
    }
}
