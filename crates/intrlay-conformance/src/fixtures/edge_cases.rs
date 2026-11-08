//! Cross-language regressions for valid boundary values and composed layouts.

use super::Fixtures;

use std::{error::Error, path::Path};

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::{GenerateOptions, generate_with_options};

#[derive(Intrlay)]
struct ZChild {
    value: u32,
}

#[derive(Intrlay)]
struct Nested {
    child: Option<Box<Option<Box<Option<ZChild>>>>>,
}

#[derive(Intrlay)]
struct Recursive {
    child: Option<Box<Option<Self>>>,
    value: u32,
}

#[derive(Intrlay)]
struct Zeroes {
    units: [(); 0],
    elements: Vec<[f32; 0]>,
    unit: Option<()>,
    after: u32,
}

#[derive(Intrlay)]
#[intrlay(generate_getters = false, generate_setters = false)]
struct NoAccessors {
    value: u32,
    values: Vec<u32>,
}

#[derive(Intrlay)]
#[intrlay(transparent, project = "f32")]
#[repr(C, align(16))]
struct Projected(f32);

#[derive(Intrlay)]
#[repr(C)]
struct Point {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Intrlay)]
struct Projection {
    numeric: Projected,
    #[intrlay(project = "fields")]
    named: Projected,
    #[intrlay(project = "indexed")]
    many: Vec<Point>,
    optional: Option<Box<Projected>>,
}

#[derive(Intrlay)]
#[intrlay(
    rust(name = "Option"),
    cpp(name = "std"),
    typescript(name = "ArrayLike"),
    csharp(name = "Nullable"),
    python(name = "Sequence"),
    dart(name = "List")
)]
struct Shadow {
    value: u32,
}

#[derive(Intrlay)]
struct ShadowRoot {
    custom: Shadow,
    optional: Option<u32>,
    items: Vec<u32>,
}

#[derive(Intrlay)]
enum Choice {
    Value { child: Option<ZChild> },
    Empty,
}

#[derive(Intrlay)]
struct Text {
    values: Vec<String>,
}

#[derive(Intrlay)]
struct EdgeCases {
    text: Text,
    nested: Nested,
    recursive: Recursive,
    zeroes: Zeroes,
    opaque: NoAccessors,
    projection: Projection,
    shadow: ShadowRoot,
    choice: Choice,
}

const TARGETS: [(Language, &str); 6] = [
    (Language::Rust, "rs"),
    (Language::Cpp, "hpp"),
    (Language::CSharp, "cs"),
    (Language::TypeScript, "ts"),
    (Language::Python, "py"),
    (Language::Dart, "dart"),
];

fn generate_edge_cases(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("edge-cases");

    let mut registry = Registry::new();
    registry.export::<EdgeCases>("EdgeCases");
    let schema = registry.finish();
    for (language, extension) in TARGETS {
        let source = generate_with_options(&schema, language, options())?;
        if language.has_value_types() {
            for helper in [
                "getX(index",
                "setX(index",
                "get_x(::std::size_t index",
                "set_x(::std::size_t index",
                "GetX(int index",
                "SetX(int index",
            ] {
                assert!(!source.contains(helper), "{language:?}: {helper}");
            }
        } else {
            assert!(source.contains("Unchecked indexed access"));
        }
        fixtures.insert(directory.join(format!("bindings.{extension}")), source)?;
    }
    write_values(fixtures, &directory)?;
    let mut registry = Registry::new();
    registry.export::<u32>("IntrlayU32");
    let schema = registry.finish();
    for (language, extension) in TARGETS {
        let mut options = options();
        options.cpp.namespace = Some("edge_scalar".into());
        options.csharp.namespace = Some("EdgeCases.Scalar".into());
        fixtures.insert(
            directory.join(format!("scalar.{extension}")),
            generate_with_options(&schema, language, options)?,
        )?;
    }
    Ok(())
}

fn write_values(fixtures: &mut Fixtures, directory: &Path) -> Result<(), Box<dyn Error>> {
    let text = Text {
        values: [
            "",
            "hello",
            "\u{feff}",
            "\u{feff}hello",
            "\u{feff}\u{feff}hello",
            "a\u{feff}b",
            "한글😀",
        ]
        .map(str::to_owned)
        .into(),
    };
    fixtures.insert(
        directory.join("text.bin"),
        TextView::create(&text)?.as_bytes(),
    )?;
    let value = EdgeCases {
        text,
        nested: Nested {
            child: Some(Box::new(Some(Box::new(Some(ZChild { value: 9 }))))),
        },
        recursive: Recursive {
            child: Some(Box::new(Some(Recursive {
                child: None,
                value: 8,
            }))),
            value: 7,
        },
        zeroes: Zeroes {
            units: [],
            elements: vec![[], []],
            unit: Some(()),
            after: 42,
        },
        opaque: NoAccessors {
            value: 11,
            values: vec![12],
        },
        projection: Projection {
            numeric: Projected(1.0),
            named: Projected(2.0),
            many: vec![Point {
                x: 3.0,
                y: 4.0,
                z: 5.0,
            }],
            optional: Some(Box::new(Projected(6.0))),
        },
        shadow: ShadowRoot {
            custom: Shadow { value: 13 },
            optional: Some(14),
            items: vec![15],
        },
        choice: Choice::Value {
            child: Some(ZChild { value: 16 }),
        },
    };
    fixtures.insert(
        directory.join("golden.bin"),
        EdgeCasesView::create(&value)?.as_bytes(),
    )?;
    for (index, child) in [
        None,
        Some(Box::new(None)),
        Some(Box::new(Some(Box::new(None)))),
    ]
    .into_iter()
    .enumerate()
    {
        fixtures.insert(
            directory.join(format!("absent-{index}.bin")),
            NestedView::create(&Nested { child })?.as_bytes(),
        )?;
    }
    Ok(())
}

fn options() -> GenerateOptions {
    GenerateOptions {
        cpp: intrlay_codegen::CppOptions {
            namespace: Some("edge_cases".into()),
        },
        csharp: intrlay_codegen::CSharpOptions {
            namespace: Some("EdgeCases.Generated".into()),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_edge_cases(fixtures)?;
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    #[test]
    fn generate_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
        super::generate_edge_cases(&mut super::Fixtures::default())
    }
}
