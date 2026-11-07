use crate::{Describe, Registry, Scalar, Type};

macro_rules! scalar {
    ($($ty:ty => $kind:ident),* $(,)?) => {$(
        impl Describe for $ty {
            fn describe(_: &mut Registry) -> Type { Type::Scalar(Scalar::$kind) }
        }
    )*};
}
scalar!(
    bool => Bool,
    i8 => I8,
    u8 => U8,
    i16 => I16,
    u16 => U16,
    i32 => I32,
    u32 => U32,
    i64 => I64,
    u64 => U64,
    f32 => F32,
    f64 => F64
);

impl Describe for () {
    fn describe(_: &mut Registry) -> Type {
        Type::Unit
    }
}
impl Describe for String {
    fn describe(_: &mut Registry) -> Type {
        Type::String
    }
}
impl<T: Describe> Describe for Vec<T> {
    fn describe(registry: &mut Registry) -> Type {
        Type::Sequence(Box::new(T::describe(registry)))
    }
}
impl<T: Describe> Describe for Box<T> {
    fn describe(registry: &mut Registry) -> Type {
        Type::Boxed(Box::new(T::describe(registry)))
    }
}
impl<T: Describe> Describe for Option<T> {
    fn describe(registry: &mut Registry) -> Type {
        Type::Optional(Box::new(T::describe(registry)))
    }
}
impl<T: Describe, const N: usize> Describe for [T; N] {
    fn describe(registry: &mut Registry) -> Type {
        Type::Array {
            element: Box::new(T::describe(registry)),
            length: N,
        }
    }
}
