use wire::Intrlay;
const BASE: u32 = 8;
#[derive(Debug, PartialEq, Intrlay)]
enum Choice {
    #[intrlay(tag = BASE + 1, name = "Renamed")]
    Original { value: u32 },
    Next,
}
#[derive(Intrlay)]
#[intrlay(transparent)]
struct Id(u32);
#[derive(Intrlay)]
struct Names { #[intrlay(name = "renamed")] original: u32 }
fn main() {
    let view = ChoiceView::create(&Choice::Original { value: 3 }).unwrap();
    assert!(matches!(view.variant(), ChoiceVariantView::Renamed { value: 3 }));
    assert_eq!(Choice::from(ChoiceView::create(&Choice::Next).unwrap()), Choice::Next);
    let mut registry = wire::schema::Registry::default();
    registry.export::<Id>("Id");
    let schema = registry.finish();
    assert!(schema.definitions.values().any(|definition| definition.transparent));
    assert_eq!(NamesView::create(&Names { original: 9 }).unwrap().renamed(), 9);
}
