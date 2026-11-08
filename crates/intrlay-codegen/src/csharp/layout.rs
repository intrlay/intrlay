//! CLR representation checks for native inputs and direct POD access.

use std::collections::BTreeSet;

use crate::plan::{Id, Kind, TypeGraph};

/// C# uses explicit record layout and inline fixed arrays. Only completely
/// initialized unmanaged representations qualify for direct POD access.
pub fn native_layout(plan: &TypeGraph, id: Id) -> bool {
    if !plan.writes_all_bytes(id) {
        return false;
    }
    let mut visited = BTreeSet::new();
    let mut pending = vec![id];
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        match &plan.nodes[id].kind {
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

#[cfg(test)]
mod tests {
    use intrlay::{Intrlay, schema::Registry};
    use intrlay_schema::Language;

    use super::native_layout;
    use crate::{backend, plan::TypeGraph};

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
    struct AlignedPayloads {
        records: Vec<Packed>,
        words: Vec<u32>,
    }

    fn plan<T: intrlay::schema::Describe>() -> TypeGraph {
        let mut registry = Registry::new();
        registry.export::<T>("Root");
        crate::plan::GenerationPlan::new(
            &registry.finish(),
            backend::for_language(Language::CSharp),
            None,
        )
        .unwrap_or_else(|error| panic!("valid schema: {error}"))
        .encoding
        .types
    }

    #[test]
    fn native_records_exclude_managed_fields_and_padding() {
        let packed = plan::<Packed>();
        assert!(native_layout(&packed, packed.exports["Root"]));
        let padded = plan::<Padded>();
        assert!(!native_layout(&padded, padded.exports["Root"]));
        let arrays = plan::<AlignedPayloads>();
        assert!(!native_layout(&arrays, arrays.exports["Root"]));
        let fixed = plan::<[[Packed; 3]; 2]>();
        assert!(native_layout(&fixed, fixed.exports["Root"]));
        let empty = plan::<[u32; 0]>();
        assert!(!native_layout(&empty, empty.exports["Root"]));
        let managed = plan::<[String; 2]>();
        assert!(!native_layout(&managed, managed.exports["Root"]));
    }
}
