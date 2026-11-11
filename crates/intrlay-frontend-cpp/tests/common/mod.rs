use intrlay_schema::{Definition, Schema, Type};

fn entry<'a>(schema: &'a Schema, name: &str) -> (&'a str, &'a Definition) {
    let mut matches = schema
        .definitions
        .iter()
        .filter(|(_, definition)| definition.name == name);
    let (identity, definition) = matches
        .next()
        .unwrap_or_else(|| panic!("missing definition {name}"));
    assert!(matches.next().is_none(), "ambiguous definition {name}");
    (identity, definition)
}

/// Looks up a fixture declaration without depending on the frontend's opaque IDs.
pub fn definition<'a>(schema: &'a Schema, name: &str) -> &'a Definition {
    entry(schema, name).1
}

/// Resolves a fixture type reference through the same definition identity.
pub fn named(schema: &Schema, name: &str) -> Type {
    Type::Named(entry(schema, name).0.into())
}
