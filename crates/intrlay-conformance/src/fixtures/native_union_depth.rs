//! Nested C# native enums share storage plans instead of expanding alternative trees.

use super::Fixtures;

use std::{error::Error, path::Path};

use intrlay::{
    Intrlay,
    schema::{Language, Registry},
};
use intrlay_codegen::{GenerateOptions, generate_with_options};

#[derive(Clone, Intrlay)]
enum Level1 {
    A { values: Vec<u32>, label: String },
    B { values: Vec<u32>, label: String },
}

macro_rules! nested_enum {
    ($name:ident, $child:ty) => {
        #[derive(Clone, Intrlay)]
        enum $name {
            A { child: $child },
            B { child: $child },
        }
    };
}

nested_enum!(Level2, Level1);
nested_enum!(Level3, Level2);
nested_enum!(Level4, Level3);
nested_enum!(Level5, Level4);
nested_enum!(Level6, Level5);
nested_enum!(Level7, Level6);

#[derive(Intrlay)]
struct Payload {
    first: Option<Level3>,
    second: Level3,
    marker: u64,
}

#[derive(Intrlay)]
#[allow(
    clippy::large_enum_variant,
    reason = "exercise overlapping inline native payload storage"
)]
enum Mixed {
    Pair {
        prefix: String,
        payload: Payload,
        extra: Option<Level2>,
    },
    Scalar {
        value: f64,
    },
}

#[derive(Intrlay)]
#[allow(
    clippy::option_option,
    reason = "preserve both nullable presence states"
)]
enum OptionalStates {
    Values {
        outer_absent: Option<Option<Level3>>,
        inner_absent: Option<Option<Level3>>,
        present: Option<Option<Level3>>,
    },
}

#[derive(Intrlay)]
enum TransferNames {
    ReadStorage {
        #[intrlay(csharp(name = "storage"))]
        child: Level2,
    },
    CopyStorage {
        #[intrlay(csharp(name = "data"))]
        child: Level2,
    },
}

#[derive(Intrlay)]
enum TransferParent {
    Value { child: TransferNames },
}

fn write_value<T: Intrlay<Value = T>>(
    fixtures: &mut Fixtures,
    directory: &Path,
    name: &str,
    value: &T,
) -> Result<(), Box<dyn Error>> {
    fixtures.insert(directory.join(name), intrlay::create::<T>(value)?.as_ref())?;
    Ok(())
}

fn generate_nested_native_unions(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let directory = std::path::PathBuf::from("native-union-depth");

    let mut registry = Registry::new();
    registry.export::<Level7>("Level7");
    registry.export::<Mixed>("Mixed");
    registry.export::<OptionalStates>("OptionalStates");
    registry.export::<TransferParent>("TransferParent");
    fixtures.insert(
        directory.join("bindings.cs"),
        generate_with_options(
            &registry.finish(),
            Language::CSharp,
            GenerateOptions {
                csharp_namespace: Some("NativeUnionDepth.Generated".into()),
                ..Default::default()
            },
        )?,
    )?;

    for selected_b in [false, true] {
        let suffix = if selected_b { "b" } else { "a" };
        let values = vec![10, 20];
        let label = "leaf".into();
        let one = if selected_b {
            Level1::B { values, label }
        } else {
            Level1::A { values, label }
        };
        macro_rules! next {
            ($ty:ident, $child:expr, $depth:literal) => {{
                let child = $child;
                let value = if selected_b {
                    $ty::B { child }
                } else {
                    $ty::A { child }
                };
                write_value(
                    fixtures,
                    &directory,
                    &format!("level{}-{suffix}.bin", $depth),
                    &value,
                )?;
                value
            }};
        }
        let two = next!(Level2, one, 2);
        let three = next!(Level3, two.clone(), 3);
        let four = next!(Level4, three.clone(), 4);
        let five = next!(Level5, four, 5);
        let six = next!(Level6, five, 6);
        next!(Level7, six, 7);
        let first = Level3::A {
            child: Level2::B {
                child: Level1::B {
                    values: vec![31, 32],
                    label: "first".into(),
                },
            },
        };
        let extra = Level2::A {
            child: Level1::A {
                values: vec![51],
                label: "extra".into(),
            },
        };
        let optional = OptionalStates::Values {
            outer_absent: None,
            inner_absent: Some(None),
            present: Some(Some(three.clone())),
        };
        write_value(
            fixtures,
            &directory,
            &format!("optional-{suffix}.bin"),
            &optional,
        )?;
        let mixed = Mixed::Pair {
            prefix: "prefix".into(),
            payload: Payload {
                first: selected_b.then_some(first),
                second: three,
                marker: 0x1122_3344_5566_7788,
            },
            extra: selected_b.then_some(extra),
        };
        write_value(fixtures, &directory, &format!("mixed-{suffix}.bin"), &mixed)?;
    }
    write_value(
        fixtures,
        &directory,
        "mixed-scalar.bin",
        &Mixed::Scalar { value: 12.5 },
    )?;
    Ok(())
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn std::error::Error>> {
    generate_nested_native_unions(fixtures)?;
    Ok(())
}
