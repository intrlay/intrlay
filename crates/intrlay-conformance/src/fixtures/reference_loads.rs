//! Reference-target loads stay readable and visit each wrapper once.

use super::Fixtures;

use std::error::Error;

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::generate;

type FourBoxes = Box<Box<Box<Box<u32>>>>;
type EightBoxes = Box<Box<Box<Box<FourBoxes>>>>;
type TenBoxes = Box<Box<EightBoxes>>;
type SixteenBoxes = Box<Box<Box<Box<Box<Box<Box<Box<EightBoxes>>>>>>>>;
type Mixed<T> = Option<Box<Option<Box<T>>>>;

#[derive(Intrlay)]
#[intrlay(python(name = "enumerate"))]
struct AEnumerate {
    value: u32,
}

#[derive(Intrlay)]
#[intrlay(transparent)]
struct Flat(TenBoxes);

#[derive(Intrlay)]
struct Child {
    value: u32,
}

#[derive(Intrlay)]
#[repr(C, align(16))]
#[intrlay(transparent, project = "f32")]
struct Projected(f32);

#[derive(Intrlay)]
#[intrlay(project = "f32")]
struct Empty {}

#[derive(Intrlay)]
struct Item {
    scalar: Flat,
    child: Mixed<Child>,
    projected: Mixed<Projected>,
}

#[derive(Intrlay)]
enum Choice {
    Empty,
    Selected {
        scalar: Mixed<Flat>,
        child: Mixed<Child>,
    },
}

#[derive(Intrlay)]
struct Fixture {
    custom: AEnumerate,
    deep: TenBoxes,
    nested: Mixed<Flat>,
    absent: Mixed<Flat>,
    missing_inner: Mixed<Flat>,
    values: Vec<TenBoxes>,
    #[intrlay(project = "indexed")]
    items: Vec<Item>,
    cached: Mixed<Child>,
    projected: Mixed<Projected>,
    #[intrlay(project = "f32")]
    lanes: Vec<Projected>,
    choice: Choice,
    inactive: Choice,
    before: u8,
    empty: Empty,
    after: u8,
    #[intrlay(project = "f32")]
    empty_items: Vec<Empty>,
    #[intrlay(generate_setters = false)]
    read_only: Mixed<Flat>,
}

#[allow(
    clippy::unnecessary_box_returns,
    reason = "each box is an intentional encoded pointer in the regression"
)]
fn deep(value: u32) -> TenBoxes {
    Box::new(Box::new(Box::new(Box::new(Box::new(Box::new(Box::new(
        Box::new(Box::new(Box::new(value))),
    )))))))
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "construct the present state of the nested optional fixture"
)]
fn mixed<T>(value: T) -> Mixed<T> {
    Some(Box::new(Some(Box::new(value))))
}

fn generate_reference_fix_fixtures(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("reference-fixes");

    let mut registry = Registry::new();
    registry.export::<Fixture>("Fixture");
    registry.export::<TenBoxes>("DeepRoot");
    registry.export::<Mixed<Child>>("CachedRoot");
    let schema = registry.finish();
    for (language, extension) in [
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        fixtures.insert(
            directory.join(format!("bindings.{extension}")),
            generate(&schema, language)?,
        )?;
    }
    let value = Fixture {
        custom: AEnumerate { value: 1 },
        deep: deep(7),
        nested: mixed(Flat(deep(8))),
        absent: None,
        missing_inner: Some(Box::new(None)),
        values: vec![deep(9)],
        items: vec![Item {
            scalar: Flat(deep(10)),
            child: mixed(Child { value: 11 }),
            projected: mixed(Projected(12.0)),
        }],
        cached: mixed(Child { value: 13 }),
        projected: mixed(Projected(14.0)),
        lanes: vec![Projected(15.0), Projected(16.0)],
        choice: Choice::Selected {
            scalar: mixed(Flat(deep(17))),
            child: mixed(Child { value: 18 }),
        },
        inactive: Choice::Empty,
        before: 19,
        empty: Empty {},
        after: 20,
        empty_items: vec![Empty {}, Empty {}],
        read_only: mixed(Flat(deep(21))),
    };
    fixtures.insert(
        directory.join("fixture.bin"),
        FixtureView::create(&value)?.as_bytes(),
    )?;
    Ok(())
}

fn getter<T: Intrlay>(language: Language) -> Result<String, Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<T>("Root");
    let source = generate(&registry.finish(), language)?;
    let (start, end) = match language {
        Language::TypeScript => ("    public get value()", "    public set("),
        Language::Python => ("    def value(self)", "    def set(self"),
        Language::Dart => ("  core.int get value", "  void set("),
        _ => unreachable!(),
    };
    let start = source.find(start).ok_or("missing root getter")?;
    let end = source[start..].find(end).ok_or("missing root setter")? + start;
    Ok(source[start..end].to_owned())
}

#[test]
fn wrapper_getter_size_and_load_count_grow_linearly() -> Result<(), Box<dyn Error>> {
    for language in [Language::TypeScript, Language::Python, Language::Dart] {
        let four = getter::<FourBoxes>(language)?;
        let eight = getter::<EightBoxes>(language)?;
        let sixteen = getter::<SixteenBoxes>(language)?;
        // Permit formatting and local-name changes while rejecting source growth
        // faster than the number of wrappers. The load counts below also forbid
        // recomputing the path to an intermediate box.
        assert!(eight.len() <= four.len() * 2, "{language:?}");
        assert!(sixteen.len() <= eight.len() * 2, "{language:?}");
        for (depth, body) in [(4, four), (8, eight), (16, sixteen)] {
            assert_eq!(body.matches("position +=").count(), depth, "{language:?}");
            assert!(
                body.lines().all(|line| line.len() < 120),
                "{language:?}: {body}"
            );
            let reads = if language == Language::Dart {
                depth
            } else {
                depth + 1
            };
            assert_eq!(body.matches(".u32").count(), reads, "{language:?}");
        }
    }
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_reference_fix_fixtures(fixtures)?;
    Ok(())
}
