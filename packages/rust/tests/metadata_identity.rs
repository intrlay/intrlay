//! Metadata identity must distinguish even same-named block-local Rust types.

use intrlay::{Intrlay, schema::Registry};

#[test]
#[should_panic(expected = "conflicting Rust type identity")]
fn distinct_local_types_cannot_share_a_schema_identity() {
    let mut registry = Registry::new();
    {
        #[derive(Intrlay)]
        struct Packet {
            value: u32,
        }
        registry.export::<Packet>("Small");
    }
    {
        #[derive(Intrlay)]
        struct Packet {
            value: u64,
        }
        registry.export::<Packet>("Large");
    }
}

#[test]
fn identical_types_and_recursive_references_share_a_definition() {
    #[derive(Intrlay)]
    struct Link {
        next: Option<Box<Link>>,
    }
    type Alias = Link;
    let mut registry = Registry::new();
    registry.export::<Link>("Link");
    registry.export::<Alias>("Alias");
    let schema = registry.finish();
    assert_eq!(schema.definitions.len(), 1);
    assert_eq!(schema.exports["Link"], schema.exports["Alias"]);
}
