//! Recursive source types must keep their identity inside generated view impls.

use intrlay::{
    Intrlay,
    schema::{Layouts, Registry},
};

#[derive(Debug, PartialEq, Intrlay)]
struct Recursive<T> {
    value: T,
    next: Option<Box<Self>>,
}

mod other {
    #[derive(Debug, PartialEq, intrlay::Intrlay)]
    pub struct Packet<T> {
        pub value: T,
    }
}

#[derive(Debug, PartialEq, Intrlay)]
struct Packet<T> {
    item: other::Packet<T>,
    next: Option<Box<self::Packet<T>>>,
}

#[test]
fn unrelated_qualified_types_keep_their_field_bounds() -> Result<(), intrlay::EncodeError> {
    let packet = Packet {
        item: other::Packet { value: 7u32 },
        next: Some(Box::new(Packet {
            item: other::Packet { value: 11 },
            next: None,
        })),
    };
    let view = PacketView::create(&packet)?;
    assert_eq!(view.item().value(), 7);
    let decoded: Packet<u32> = view.into();
    assert_eq!(decoded, packet);
    Ok(())
}

#[derive(Debug, PartialEq, Intrlay)]
enum Tree<T> {
    Leaf(T),
    Branch {
        left: Box<Self>,
        right: Option<Box<Self>>,
    },
}

#[derive(Debug, PartialEq, Intrlay)]
struct FixedPacket {
    values: [u8; Self::COUNT],
}

impl FixedPacket {
    const COUNT: usize = 3;
}

#[derive(Debug, PartialEq, Intrlay)]
#[repr(u8)]
enum RelatedTags {
    First = 3,
    Second = Self::First as u8 + 1,
}

trait Family {
    type Item;
}

#[derive(Intrlay)]
struct FamilyPacket
where
    Self: Family,
{
    value: <Self as Family>::Item,
}

impl Family for FamilyPacket {
    type Item = u32;
}

#[test]
fn self_in_source_expressions_and_bounds() -> Result<(), Box<dyn std::error::Error>> {
    let packet = FixedPacket { values: [1, 2, 3] };
    let view = FixedPacketView::create(&packet)?;
    assert_eq!(view.values().get(2), Some(3));
    let decoded: FixedPacket = view.into();
    assert_eq!(decoded, packet);
    let view = RelatedTagsView::create(&RelatedTags::Second)?;
    assert_eq!(view.as_bytes(), &[4, 0, 0, 0]);
    let decoded: RelatedTags = view.into();
    assert_eq!(decoded, RelatedTags::Second);
    let view = FamilyPacketView::create(&FamilyPacket { value: 7 })?;
    assert_eq!(view.value(), 7);
    let decoded: FamilyPacket = view.into();
    assert_eq!(decoded.value, 7);
    Ok(())
}

#[test]
fn self_in_generic_records_and_enums() -> Result<(), Box<dyn std::error::Error>> {
    let value = Recursive {
        value: 7u32,
        next: Some(Box::new(Recursive {
            value: 11,
            next: None,
        })),
    };
    let mut view = RecursiveView::create(&value)?;
    assert_eq!(view.next().ok_or("next")?.value(), 11);
    view.next_mut().ok_or("next")?.value_mut().set(13);
    let decoded: Recursive<u32> = view.into();
    assert_eq!(decoded.next.ok_or("next")?.value, 13);
    let tree = Tree::Branch {
        left: Box::new(Tree::Leaf(3u32)),
        right: Some(Box::new(Tree::Leaf(5))),
    };
    let decoded: Tree<u32> = TreeView::create(&tree)?.into();
    assert_eq!(decoded, tree);
    let mut registry = Registry::new();
    registry.export::<Recursive<u32>>("Recursive");
    registry.export::<Tree<u32>>("Tree");
    Layouts::new(&registry.finish()).validate()?;
    Ok(())
}
