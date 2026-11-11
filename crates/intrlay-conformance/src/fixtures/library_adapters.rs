//! Library adapters enter the same schema and projection paths as local records.

use std::{error::Error, path::Path};

use intrlay::{
    Intrlay,
    adapters::{glam_0_34 as math, smallvec::SmallVec, smol_str::SmolStr},
    schema::{Language, Registry},
};
use intrlay_codegen::{GenerateOptions, generate_with_options};

use super::Fixtures;

#[derive(Intrlay)]
#[intrlay(project = "flatten")]
struct Bounds {
    #[intrlay(with = math::Vec3A)]
    min: glam::Vec3A,
    #[intrlay(with = math::Vec3A)]
    max: glam::Vec3A,
}

#[derive(Intrlay)]
struct Sample {
    bounds: Bounds,
    #[intrlay(with = Vec<math::Vec3A>, project = "f32")]
    points: Vec<glam::Vec3A>,
    #[intrlay(with = math::Mat3A)]
    matrix: glam::Mat3A,
    #[intrlay(with = math::BVec3A)]
    mask: glam::BVec3A,
    #[intrlay(with = SmallVec<SmolStr, 2>)]
    labels: smallvec::SmallVec<[smol_str::SmolStr; 2]>,
}

pub(super) fn collect(fixtures: &mut Fixtures) -> Result<(), Box<dyn Error>> {
    let mut registry = Registry::new();
    registry.export::<Sample>("Sample");
    // Export every supported math codec together to catch target-specific names,
    // integer widths, and aligned layouts as well as native round-trip behavior.
    macro_rules! export {
        ($($ty:ident),+ $(,)?) => { $(registry.export::<math::$ty>(stringify!($ty));)+ };
    }
    export!(
        Vec2, Vec3, Vec3A, Vec4, Quat, DVec2, DVec3, DVec4, DQuat, BVec2, BVec3, BVec4, BVec3A,
        BVec4A, I8Vec2, I8Vec3, I8Vec4, U8Vec2, U8Vec3, U8Vec4, I16Vec2, I16Vec3, I16Vec4, U16Vec2,
        U16Vec3, U16Vec4, IVec2, IVec3, IVec4, UVec2, UVec3, UVec4, I64Vec2, I64Vec3, I64Vec4,
        U64Vec2, U64Vec3, U64Vec4, ISizeVec2, ISizeVec3, ISizeVec4, USizeVec2, USizeVec3,
        USizeVec4, Mat2, Mat3, Mat3A, Mat4, DMat2, DMat3, DMat4, Affine2, Affine3, Affine3A,
        DAffine2, DAffine3,
    );
    let schema = registry.finish();
    let directory = Path::new("library-adapters");
    for (language, extension) in [
        (Language::Rust, "rs"),
        (Language::Cpp, "hpp"),
        (Language::CSharp, "cs"),
        (Language::TypeScript, "ts"),
        (Language::Python, "py"),
        (Language::Dart, "dart"),
    ] {
        fixtures.insert(
            directory.join(format!("bindings.{extension}")),
            generate_with_options(
                &schema,
                language,
                GenerateOptions {
                    module_name: Some("LibraryAdapters".into()),
                    ..GenerateOptions::default()
                },
            )?,
        )?;
    }
    // Field inputs give the cross-language corpus deterministic zero padding.
    // Native glam copies may preserve its initialized, semantically unused lanes.
    let vector = |x, y, z| math::Vec3AInput { x, y, z };
    let points = [vector(7.0, 8.0, 9.0), vector(10.0, 11.0, 12.0)];
    let sample = SampleInput {
        bounds: BoundsInput {
            min: vector(-1.0, -2.0, -3.0),
            max: vector(4.0, 5.0, 6.0),
        },
        points: points.as_slice(),
        matrix: math::Mat3AInput {
            x_axis: vector(1.0, 2.0, 3.0),
            y_axis: vector(4.0, 5.0, 6.0),
            z_axis: vector(7.0, 8.0, 9.0),
        },
        mask: glam::BVec3A::new(true, false, true),
        labels: ["one", "two 🦀"].as_slice(),
    };
    let bytes = intrlay::create_from::<Sample>(&sample)?;
    fixtures.insert(directory.join("sample.bin"), bytes.as_ref())?;
    fixtures.insert(
        directory.join("sample-data.dart"),
        format!("const sampleBytes = {:?};\n", bytes.as_ref()),
    )?;
    Ok(())
}
