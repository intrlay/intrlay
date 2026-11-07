//! Conservative byte-coverage proofs for encoders that always receive values.
//! Count-only initialization in reference languages still requires zero filling.

use std::collections::BTreeSet;

use super::{Id, Kind, TypeGraph};

impl TypeGraph {
    pub fn writes_all_bytes(&self, root: Id) -> bool {
        let mut visited = BTreeSet::new();
        let mut pending = vec![root];
        let mut payload_alignment = 1;
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let node = &self.nodes[id];
            match &node.kind {
                Kind::Record(fields) => {
                    let mut end = 0;
                    for field in fields {
                        if field.offset != end {
                            return false;
                        }
                        end += self.nodes[field.ty].layout.size();
                        pending.push(field.ty);
                    }
                    if end != node.layout.size() {
                        return false;
                    }
                }
                Kind::Sequence(child) | Kind::Boxed(child) => {
                    payload_alignment =
                        payload_alignment.max(self.nodes[*child].layout.alignment());
                    pending.push(*child);
                }
                Kind::Array { element, .. } => pending.push(*element),
                Kind::Optional { .. } | Kind::Enum(_) => return false,
                _ => {}
            }
        }
        // Every reservation must start aligned without leaving an unwritten gap.
        // Requiring all payload sizes to be multiples of the largest alignment
        // also covers arbitrary nesting and variable collection lengths.
        self.nodes[root]
            .layout
            .size()
            .is_multiple_of(payload_alignment)
            && visited.into_iter().all(|id| match self.nodes[id].kind {
                Kind::String => payload_alignment == 1,
                Kind::Sequence(child) | Kind::Boxed(child) => self.nodes[child]
                    .layout
                    .size()
                    .is_multiple_of(payload_alignment),
                _ => true,
            })
    }

    /// C# uses explicit record layout and inline fixed arrays. Only completely
    /// initialized unmanaged representations qualify for direct POD access.
    pub fn csharp_native_layout(&self, id: Id) -> bool {
        if !self.writes_all_bytes(id) {
            return false;
        }
        let mut visited = BTreeSet::new();
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            match &self.nodes[id].kind {
                Kind::Scalar(scalar) if *scalar != intrlay_schema::Scalar::Bool => {}
                Kind::Record(fields) if !fields.is_empty() => {
                    pending.extend(fields.iter().map(|field| field.ty));
                }
                Kind::Array { element, length } if *length != 0 => pending.push(*element),
                _ => return false,
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use intrlay::{Intrlay, schema::Registry};
    use intrlay_schema::Language;

    use crate::{GenerateOptions, backend, plan::TypeGraph};

    #[derive(Intrlay)]
    struct Packed {
        id: u32,
        value: f32,
    }

    #[derive(Intrlay)]
    struct Padded {
        flag: u8,
        value: u32,
    }

    #[derive(Intrlay)]
    struct MixedPayloads {
        bytes: Vec<u8>,
        words: Vec<u64>,
    }

    #[derive(Intrlay)]
    struct AlignedPayloads {
        records: Vec<Packed>,
        words: Vec<u32>,
    }

    #[derive(Intrlay)]
    struct StringAndWords(String, Vec<u32>);

    #[derive(Intrlay)]
    struct AlignedBox(u32, Box<u64>);

    fn plan<T: intrlay::schema::Describe>() -> TypeGraph {
        let mut registry = Registry::new();
        registry.export::<T>("Root");
        TypeGraph::new(
            &registry.finish(),
            backend::for_language(Language::CSharp),
            GenerateOptions::default(),
        )
        .unwrap_or_else(|error| panic!("valid schema: {error}"))
    }

    fn fully_written<T: intrlay::schema::Describe>() -> bool {
        let plan = plan::<T>();
        plan.writes_all_bytes(plan.exports["Root"])
    }

    #[test]
    fn clearing_requires_complete_coverage_for_every_value() {
        assert!(fully_written::<Vec<Packed>>());
        assert!(fully_written::<AlignedPayloads>());
        assert!(fully_written::<Vec<Vec<u32>>>());
        assert!(fully_written::<String>());
        assert!(!fully_written::<Vec<String>>());
        assert!(fully_written::<[u8; 0]>());
        assert!(!fully_written::<Padded>());
        assert!(!fully_written::<Vec<Padded>>());
        assert!(!fully_written::<MixedPayloads>());
        assert!(!fully_written::<Vec<Option<u32>>>());
        assert!(!fully_written::<StringAndWords>());
        assert!(fully_written::<AlignedBox>());
        assert!(!fully_written::<Box<u64>>());
    }

    #[test]
    fn native_records_exclude_managed_fields_and_padding() {
        let packed = plan::<Packed>();
        assert!(packed.csharp_native_layout(packed.exports["Root"]));
        let padded = plan::<Padded>();
        assert!(!padded.csharp_native_layout(padded.exports["Root"]));
        let arrays = plan::<AlignedPayloads>();
        assert!(!arrays.csharp_native_layout(arrays.exports["Root"]));
        let fixed = plan::<[[Packed; 3]; 2]>();
        assert!(fixed.csharp_native_layout(fixed.exports["Root"]));
        let empty = plan::<[u32; 0]>();
        assert!(!empty.csharp_native_layout(empty.exports["Root"]));
        let managed = plan::<[String; 2]>();
        assert!(!managed.csharp_native_layout(managed.exports["Root"]));
    }
}
