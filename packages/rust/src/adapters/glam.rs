// Shared codecs for glam vectors, matrices, quaternions, and affine transforms.
//
// Select these codecs with `with`, including recursively through `Vec`, arrays,
// `Option`, and `Box`. Matrices store columns; quaternions store `x, y, z, w`.
// SIMD alternatives retain explicit 16-byte alignment. Numeric native copies
// preserve initialized extra lanes in semantically unused wire padding.
// Boolean masks use boolean fields, not glam's private SIMD mask representation.
// Architecture-sized vector components use fixed 64-bit wire integers.

#[doc(hidden)]
pub use super::glam_size::{SignedSize, UnsignedSize};

// SAFETY: glam stores numeric components in declared order. Its numeric SIMD
// and scalar Vec3A representations initialize all four lanes. Matrices preserve
// their columns. Size/endianness and child proofs still gate each target; boolean
// SIMD masks use a different representation and explicitly opt out.
macro_rules! record {
    ($name:ident, $alignment:literal, $proof:expr, $construct:expr,
        $($field:ident: $native:ty => $codec:ty = $read:expr),+ $(,)?) => {
        #[doc = concat!("Codec and decoded-field mirror for glam's `", stringify!($name), "`.")]
        #[derive(crate::Intrlay)]
        #[intrlay(remote = native::$name, from = $name::into_native, unsafe(native_layout = $proof))]
        #[repr(C, align($alignment))]
        pub struct $name {
            $(
                #[doc = concat!("The `", stringify!($field), "` component.")]
                #[intrlay(with = $codec, getter = $name::$field)]
                pub $field: $native,
            )+
        }
        impl $name {
            fn into_native(value: Self) -> native::$name { ($construct)(value) }
            $(
                #[inline]
                fn $field(value: &native::$name) -> $native { ($read)(value) }
            )+
        }
    };
}

macro_rules! vector {
    ($name:ident, $scalar:ty => $codec:ty, $alignment:literal, $length:literal, $($field:ident: $index:literal),+) => {
        record!($name, $alignment, <$codec as crate::Intrlay>::NATIVE_LAYOUT,
            |v: $name| native::$name::from_array([$(v.$field),+]),
            $($field: $scalar => $codec = |v: &native::$name| <[$scalar; $length]>::from(*v)[$index]),+
        );
    };
    ($name:ident, $scalar:ty, $alignment:literal, $length:literal, $($field:ident: $index:literal),+) => {
        vector!($name, $scalar => $scalar, $alignment, $length, $($field: $index),+);
    };
}

macro_rules! vectors {
    ($two:ident, $three:ident, $four:ident, $scalar:ty, $alignment:literal) => {
        vector!($two, $scalar, $alignment, 2, x: 0, y: 1);
        vector!($three, $scalar, $alignment, 3, x: 0, y: 1, z: 2);
        vector!($four, $scalar, $alignment, 4, x: 0, y: 1, z: 2, w: 3);
    };
}

vector!(Vec2, f32, 4, 2, x: 0, y: 1);
vector!(Vec3, f32, 4, 3, x: 0, y: 1, z: 2);
vector!(Vec3A, f32, 16, 3, x: 0, y: 1, z: 2);
vector!(Vec4, f32, 16, 4, x: 0, y: 1, z: 2, w: 3);
vector!(Quat, f32, 16, 4, x: 0, y: 1, z: 2, w: 3);
vector!(DQuat, f64, 8, 4, x: 0, y: 1, z: 2, w: 3);
vectors!(DVec2, DVec3, DVec4, f64, 8);
vectors!(BVec2, BVec3, BVec4, bool, 1);
record!(BVec3A, 16, false, |v: BVec3A| native::BVec3A::new(v.x, v.y, v.z),
    x: bool => bool = |v: &native::BVec3A| <[bool; 3]>::from(*v)[0],
    y: bool => bool = |v: &native::BVec3A| <[bool; 3]>::from(*v)[1],
    z: bool => bool = |v: &native::BVec3A| <[bool; 3]>::from(*v)[2],
);
record!(BVec4A, 16, false, |v: BVec4A| native::BVec4A::new(v.x, v.y, v.z, v.w),
    x: bool => bool = |v: &native::BVec4A| <[bool; 4]>::from(*v)[0],
    y: bool => bool = |v: &native::BVec4A| <[bool; 4]>::from(*v)[1],
    z: bool => bool = |v: &native::BVec4A| <[bool; 4]>::from(*v)[2],
    w: bool => bool = |v: &native::BVec4A| <[bool; 4]>::from(*v)[3],
);
vectors!(I8Vec2, I8Vec3, I8Vec4, i8, 1);
vectors!(U8Vec2, U8Vec3, U8Vec4, u8, 1);
vectors!(I16Vec2, I16Vec3, I16Vec4, i16, 2);
vectors!(U16Vec2, U16Vec3, U16Vec4, u16, 2);
vectors!(IVec2, IVec3, IVec4, i32, 4);
vectors!(UVec2, UVec3, UVec4, u32, 4);
vectors!(I64Vec2, I64Vec3, I64Vec4, i64, 8);
vectors!(U64Vec2, U64Vec3, U64Vec4, u64, 8);
vector!(ISizeVec2, isize => SignedSize, 8, 2, x: 0, y: 1);
vector!(ISizeVec3, isize => SignedSize, 8, 3, x: 0, y: 1, z: 2);
vector!(ISizeVec4, isize => SignedSize, 8, 4, x: 0, y: 1, z: 2, w: 3);
vector!(USizeVec2, usize => UnsignedSize, 8, 2, x: 0, y: 1);
vector!(USizeVec3, usize => UnsignedSize, 8, 3, x: 0, y: 1, z: 2);
vector!(USizeVec4, usize => UnsignedSize, 8, 4, x: 0, y: 1, z: 2, w: 3);

macro_rules! matrix {
    ($name:ident, $column:ident, $alignment:literal, $($field:ident),+) => {
        record!($name, $alignment, <$column as crate::Intrlay>::NATIVE_LAYOUT,
            |v: $name| native::$name::from_cols($(v.$field),+),
            $($field: native::$column => $column = |v: &native::$name| v.$field),+
        );
    };
}
matrix!(Mat2, Vec2, 16, x_axis, y_axis);
matrix!(Mat3, Vec3, 4, x_axis, y_axis, z_axis);
matrix!(Mat3A, Vec3A, 16, x_axis, y_axis, z_axis);
matrix!(Mat4, Vec4, 16, x_axis, y_axis, z_axis, w_axis);
matrix!(DMat2, DVec2, 8, x_axis, y_axis);
matrix!(DMat3, DVec3, 8, x_axis, y_axis, z_axis);
matrix!(DMat4, DVec4, 8, x_axis, y_axis, z_axis, w_axis);

macro_rules! affine {
    ($name:ident, $matrix:ident, $vector:ident, $field:ident, $alignment:literal) => {
        // In particular, SIMD Affine2 has uninitialized trailing struct padding.
        // Child layout proofs cover initialized lanes, but never that padding.
        record!($name, $alignment,
            <$matrix as crate::Intrlay>::NATIVE_LAYOUT && <$vector as crate::Intrlay>::NATIVE_LAYOUT
                && core::mem::size_of::<native::$name>()
                    == core::mem::size_of::<native::$matrix>() + core::mem::size_of::<native::$vector>(),
            |v: $name| native::$name { $field: v.$field, translation: v.translation },
            $field: native::$matrix => $matrix = |v: &native::$name| v.$field,
            translation: native::$vector => $vector = |v: &native::$name| v.translation
        );
    };
}
affine!(Affine2, Mat2, Vec2, matrix2, 16);
affine!(Affine3, Mat3, Vec3, matrix3, 4);
affine!(Affine3A, Mat3A, Vec3A, matrix3, 16);
affine!(DAffine2, DMat2, DVec2, matrix2, 8);
affine!(DAffine3, DMat3, DVec3, matrix3, 8);
