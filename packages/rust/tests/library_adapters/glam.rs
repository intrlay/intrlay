use std::{error::Error, fmt::Debug};

use intrlay::{EncodeSource, Intrlay};

fn roundtrip<C: Intrlay>(value: C::Value) -> Result<(), Box<dyn Error>>
where
    C::Value: Copy + Debug + PartialEq + EncodeSource<C>,
{
    let bytes = intrlay::create::<C>(&value)?;
    assert_eq!(C::decode(bytes.as_ref(), 0)?, value);
    C::validate(bytes.as_ref(), 0)?;
    // SAFETY: This exact representation was validated above.
    assert_eq!(unsafe { C::decode_unchecked(bytes.as_ref(), 0) }, value);
    assert_eq!(intrlay::create_from::<C>(&value)?.as_ref(), bytes.as_ref());
    // Native SIMD types and converted boolean vectors share the same copy API.
    let sequence = intrlay::create::<Vec<C>>(&vec![value; 3])?;
    let mut unaligned = vec![0xa5];
    unaligned.extend_from_slice(sequence.as_ref());
    let mut copied = [value; 3];
    Vec::<C>::view(&unaligned, 1)?.copy_to(&mut copied);
    assert_eq!(copied, [value; 3]);
    for length in 0..C::LAYOUT.size() {
        assert!(C::decode(&bytes.as_ref()[..length], 0).is_err());
    }
    Ok(())
}

#[test]
fn every_glam_vector_family_preserves_component_order() -> Result<(), Box<dyn Error>> {
    macro_rules! family {
        ($two:ident, $three:ident, $four:ident, [$x:expr, $y:expr, $z:expr, $w:expr]) => {
            roundtrip::<codec::$two>(glam::$two::from_array([$x, $y]))?;
            roundtrip::<codec::$three>(glam::$three::from_array([$x, $y, $z]))?;
            roundtrip::<codec::$four>(glam::$four::from_array([$x, $y, $z, $w]))?;
        };
    }
    family!(Vec2, Vec3, Vec4, [1.25, -2.5, 3.75, -4.0]);
    family!(DVec2, DVec3, DVec4, [1.25, -2.5, 3.75, -4.0]);
    family!(BVec2, BVec3, BVec4, [true, false, true, false]);
    family!(I8Vec2, I8Vec3, I8Vec4, [i8::MIN, -1, 0, i8::MAX]);
    family!(U8Vec2, U8Vec3, U8Vec4, [u8::MAX, 0, 1, 128]);
    family!(I16Vec2, I16Vec3, I16Vec4, [i16::MIN, -1, 0, i16::MAX]);
    family!(U16Vec2, U16Vec3, U16Vec4, [u16::MAX, 0, 1, 256]);
    family!(IVec2, IVec3, IVec4, [i32::MIN, -1, 0, i32::MAX]);
    family!(UVec2, UVec3, UVec4, [u32::MAX, 0, 1, 65_536]);
    family!(I64Vec2, I64Vec3, I64Vec4, [i64::MIN, -1, 0, i64::MAX]);
    family!(U64Vec2, U64Vec3, U64Vec4, [u64::MAX, 0, 1, 1 << 40]);
    family!(
        ISizeVec2,
        ISizeVec3,
        ISizeVec4,
        [isize::MIN, -1, 0, isize::MAX]
    );
    family!(USizeVec2, USizeVec3, USizeVec4, [usize::MAX, 0, 1, 65_536]);
    roundtrip::<codec::Vec3A>(glam::Vec3A::new(1.25, -2.5, 3.75))?;
    roundtrip::<codec::BVec3A>(glam::BVec3A::new(true, false, true))?;
    roundtrip::<codec::BVec4A>(glam::BVec4A::new(false, true, false, true))?;
    // Quaternions are data, not a request to normalize during serialization.
    roundtrip::<codec::Quat>(glam::Quat::from_array([1.0, 2.0, 3.0, 4.0]))?;
    roundtrip::<codec::DQuat>(glam::DQuat::from_array([1.0, 2.0, 3.0, 4.0]))?;
    Ok(())
}

#[test]
fn matrices_and_affines_preserve_columns_and_translation() -> Result<(), Box<dyn Error>> {
    macro_rules! matrices {
        ($($name:ident: $values:expr),+ $(,)?) => { $(
            roundtrip::<codec::$name>(glam::$name::from_cols_array(&$values))?;
        )+ };
    }
    matrices!(
        Mat2: [1.0, 2.0, 3.0, 4.0],
        Mat3: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
        Mat3A: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
        Mat4: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0],
        DMat2: [1.0, 2.0, 3.0, 4.0],
        DMat3: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
        DMat4: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0],
        Affine2: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        Affine3: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0],
        Affine3A: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0],
        DAffine2: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        DAffine3: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0],
    );
    Ok(())
}

#[derive(Intrlay)]
struct Nested {
    #[intrlay(with = Vec<codec::Vec3A>, project = "f32")]
    positions: Vec<glam::Vec3A>,
    #[intrlay(with = Option<Box<codec::Quat>>)]
    rotation: Option<Box<glam::Quat>>,
}

#[test]
fn padded_vectors_borrow_recursively_and_preserve_initialized_lanes() -> Result<(), Box<dyn Error>>
{
    let source = Nested {
        positions: vec![
            glam::Vec3A::new(1.0, 2.0, 3.0),
            glam::Vec3A::new(4.0, 5.0, 6.0),
        ],
        rotation: Some(Box::new(glam::Quat::IDENTITY)),
    };
    let input = NestedInput {
        positions: source.positions.as_slice(),
        rotation: Some(intrlay::BoxInput(glam::Quat::IDENTITY)),
    };
    let bytes = intrlay::create::<Nested>(&source)?;
    assert_eq!(
        intrlay::create_from::<Nested>(&input)?.as_ref(),
        bytes.as_ref()
    );
    let decoded = Nested::decode(bytes.as_ref(), 0)?;
    assert_eq!(decoded.positions, source.positions);
    assert_eq!(decoded.rotation, source.rotation);
    assert_eq!(
        (
            codec::Vec3A::LAYOUT.size(),
            codec::Vec3A::LAYOUT.alignment()
        ),
        (16, 16)
    );
    assert_eq!(
        (
            codec::Mat3A::LAYOUT.size(),
            codec::Mat3A::LAYOUT.alignment()
        ),
        (48, 16)
    );

    let matrix = intrlay::create::<codec::Mat3A>(&glam::Mat3A::from_cols(
        source.positions[0],
        source.positions[1],
        glam::Vec3A::ONE,
    ))?;
    for (column, z) in matrix.as_ref().chunks_exact(16).zip([3.0_f32, 6.0, 1.0]) {
        assert_eq!(&column[12..], &z.to_le_bytes());
    }
    assert!(std::hint::black_box(codec::Vec3A::NATIVE_LAYOUT));
    assert!(std::hint::black_box(codec::Mat3A::NATIVE_LAYOUT));
    assert!(std::hint::black_box(codec::Mat4::NATIVE_LAYOUT));
    assert!(!std::hint::black_box(codec::Affine2::NATIVE_LAYOUT));
    assert!(std::hint::black_box(codec::Affine3A::NATIVE_LAYOUT));
    assert!(!std::hint::black_box(codec::BVec3A::NATIVE_LAYOUT));
    let mask = intrlay::create::<codec::BVec3A>(&glam::BVec3A::new(true, false, true))?;
    assert_eq!(&mask.as_ref()[..3], &[1, 0, 1]);
    assert!(mask.as_ref()[3..].iter().all(|byte| *byte == 0));
    let mut invalid = mask.as_ref().to_vec();
    invalid[0] = 2;
    assert!(codec::BVec3A::validate(&invalid, 0).is_err());
    assert!(codec::BVec3A::decode(&invalid, 0).is_err());
    Ok(())
}
