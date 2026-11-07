//! Rust source syntax must retain its meaning in views and exported metadata.

use intrlay::{
    Intrlay,
    schema::{DefinitionKind, Language, Registry},
};

#[derive(Debug, PartialEq, Intrlay)]
struct r#RawRecord {
    #[intrlay(name = "kind")]
    r#type: u32,
    r#value: u32,
}

#[derive(Debug, PartialEq, Intrlay)]
enum r#RawChoice {
    r#RawVariant {
        #[intrlay(name = "item")]
        r#match: u32,
    },
}

intrlay::export_types! {
    r#RawRecord,
    RawChoice,
    r#Alias = Option<RawRecord>,
    Scalar = u32,
}

#[test]
fn raw_export_identifiers_omit_only_the_syntactic_prefix() {
    let schema = __intrlay_export_schema();
    assert_eq!(
        schema
            .exports
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["Alias", "RawChoice", "RawRecord", "Scalar"]
    );
    let mut registry = Registry::new();
    registry.export::<RawRecord>("RawRecord");
    registry.export::<RawChoice>("RawChoice");
    registry.export::<Option<RawRecord>>("Alias");
    registry.export::<u32>("Scalar");
    assert_eq!(schema, registry.finish());
}

#[test]
fn raw_identifiers_preserve_access_and_export_plain_names() -> Result<(), Box<dyn std::error::Error>>
{
    let source = RawRecord {
        r#type: 7,
        value: 11,
    };
    let mut view = RawRecordView::create(&source)?;
    assert_eq!(view.kind(), 7);
    view.value_mut().set(13);
    let decoded: RawRecord = view.into();
    assert_eq!(decoded.r#type, 7);
    assert_eq!(decoded.value, 13);

    let mut choice = RawChoiceView::create(&RawChoice::RawVariant { r#match: 17 })?;
    choice
        .raw_variant_item_mut()
        .ok_or("active raw variant")?
        .set(19);
    assert!(matches!(
        choice.variant(),
        RawChoiceVariantView::RawVariant { r#match: 19 }
    ));
    assert_eq!(
        RawChoice::from(choice),
        RawChoice::RawVariant { r#match: 19 }
    );

    let mut registry = Registry::new();
    registry.export::<RawRecord>("Record");
    registry.export::<RawChoice>("Choice");
    let schema = registry.finish();
    let record = &schema.definitions[std::any::type_name::<RawRecord>()];
    assert_eq!(record.name, "RawRecord");
    let DefinitionKind::Record(fields) = &record.kind else {
        return Err("record metadata".into());
    };
    assert_eq!(fields[0].name, "type");
    assert_eq!(fields[1].name, "value");
    for target in fields[0].targets.values() {
        assert_eq!(target.name.as_deref(), Some("kind"));
    }
    let choice = &schema.definitions[std::any::type_name::<RawChoice>()];
    assert_eq!(choice.name, "RawChoice");
    let DefinitionKind::Enum(variants) = &choice.kind else {
        return Err("enum metadata".into());
    };
    assert_eq!(variants[0].name, "RawVariant");
    assert_eq!(variants[0].fields[0].name, "match");
    assert_eq!(
        variants[0].fields[0].targets[&Language::Rust]
            .name
            .as_deref(),
        Some("item")
    );
    Ok(())
}

const BYTE_START: u8 = 7;
const DEFAULT_START: isize = 21;

#[derive(Debug, PartialEq, Intrlay)]
enum ScopedTags<const N: u32> {
    #[intrlay(tag = Self::BASE)]
    First(u32),
    Next,
    #[intrlay(tag = N + 10)]
    Last,
}

impl<const N: u32> ScopedTags<N> {
    const BASE: u32 = N;
}

#[test]
fn explicit_tags_keep_source_and_const_generic_scope() -> Result<(), Box<dyn std::error::Error>> {
    let mut view = ScopedTagsView::create(&ScopedTags::<7>::First(11))?;
    assert_eq!(&view.as_bytes()[..4], &7u32.to_le_bytes());
    view.first_field_0_mut().ok_or("active payload")?.set(13);
    assert!(matches!(view.variant(), ScopedTagsVariantView::First(13)));
    assert_eq!(ScopedTags::from(view), ScopedTags::<7>::First(13));
    assert_eq!(
        &ScopedTagsView::create(&ScopedTags::<23>::Next)?.as_bytes()[..4],
        &24u32.to_le_bytes()
    );
    let mut registry = Registry::new();
    registry.export::<ScopedTags<23>>("ScopedTags");
    let schema = registry.finish();
    let DefinitionKind::Enum(variants) =
        &schema.definitions[std::any::type_name::<ScopedTags<23>>()].kind
    else {
        return Err("enum metadata".into());
    };
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.tag)
            .collect::<Vec<_>>(),
        [23, 24, 33]
    );
    Ok(())
}

#[derive(Debug, PartialEq, Intrlay)]
#[repr(u8)]
enum ByteTags {
    First = BYTE_START,
    Second,
}

#[derive(Debug, PartialEq, Intrlay)]
#[repr(i16)]
enum SignedTags {
    First = 300i16,
    Second,
}

#[derive(Debug, PartialEq, Intrlay)]
enum DefaultTags {
    First = DEFAULT_START,
    Second,
}

#[derive(Debug, PartialEq, Intrlay)]
#[repr(u64)]
enum WideTags {
    Large = 4_000_000_000,
    Last = u32::MAX as u64,
}

#[derive(Debug, PartialEq, Intrlay)]
#[repr(u8)]
enum PayloadTags {
    First {
        value: u32,
    } = BYTE_START,
    Second(u32),
    #[intrlay(tag = 40)]
    Override(u32) = 10,
    Next,
}

#[test]
fn source_discriminants_keep_their_integer_type() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        ByteTagsView::create(&ByteTags::First)?.as_bytes(),
        &[7, 0, 0, 0]
    );
    assert_eq!(
        ByteTagsView::create(&ByteTags::Second)?.as_bytes(),
        &[8, 0, 0, 0]
    );
    assert_eq!(
        SignedTagsView::create(&SignedTags::Second)?.as_bytes(),
        &301u32.to_le_bytes()
    );
    assert_eq!(
        DefaultTagsView::create(&DefaultTags::Second)?.as_bytes(),
        &22u32.to_le_bytes()
    );
    assert_eq!(
        WideTagsView::create(&WideTags::Large)?.as_bytes(),
        &4_000_000_000u32.to_le_bytes()
    );
    let last = WideTagsView::create(&WideTags::Last)?;
    assert_eq!(last.as_bytes(), &u32::MAX.to_le_bytes());
    assert_eq!(WideTags::from(last), WideTags::Last);

    let mut view = PayloadTagsView::create(&PayloadTags::First { value: 5 })?;
    view.first_value_mut().ok_or("active payload")?.set(9);
    assert!(matches!(
        view.variant(),
        PayloadTagsVariantView::First { value: 9 }
    ));
    assert_eq!(PayloadTags::from(view), PayloadTags::First { value: 9 });
    assert_eq!(
        &PayloadTagsView::create(&PayloadTags::Next)?.as_bytes()[..4],
        &41u32.to_le_bytes()
    );

    let mut registry = Registry::new();
    registry.export::<PayloadTags>("PayloadTags");
    let schema = registry.finish();
    let DefinitionKind::Enum(variants) =
        &schema.definitions[std::any::type_name::<PayloadTags>()].kind
    else {
        return Err("enum metadata".into());
    };
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.tag)
            .collect::<Vec<_>>(),
        [7, 8, 40, 41]
    );
    Ok(())
}

mod external {
    #[derive(Debug, PartialEq)]
    pub struct Record<T> {
        pub value: T,
    }

    #[derive(Debug, PartialEq)]
    pub struct Tuple<T, const N: usize>(pub [T; N]);
}

#[allow(dead_code, reason = "Adapter fields describe the external type")]
#[derive(Intrlay)]
#[intrlay(remote = external::Record<T>)]
struct RecordAdapter<T> {
    value: T,
}

#[allow(dead_code, reason = "Adapter fields describe the external type")]
#[derive(Intrlay)]
#[intrlay(remote = external::Tuple<T, N>)]
struct TupleAdapter<T, const N: usize>([T; N]);

#[test]
fn generic_remote_paths_work_as_record_and_tuple_constructors()
-> Result<(), Box<dyn std::error::Error>> {
    let source = external::Record {
        value: "generic remote".to_owned(),
    };
    let view = RecordAdapterView::create(&source)?;
    assert_eq!(view.value(), "generic remote");
    let decoded: external::Record<String> = view.into();
    assert_eq!(decoded, source);

    let source = external::Tuple([3u32, 5, 7]);
    let mut view = TupleAdapterView::create(&source)?;
    view.field_0_mut()
        .get_mut(1)
        .ok_or("array element")?
        .set(11);
    let decoded: external::Tuple<u32, 3> = view.into();
    assert_eq!(decoded, external::Tuple([3, 11, 7]));
    Ok(())
}
