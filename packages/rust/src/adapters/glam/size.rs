//! Architecture-sized components retain fixed-width wire fields and checked narrowing.

use crate::schema::{Describe, Registry, Type};
use crate::{
    DecodeError, Decoder, EncodeError, EncodeSource, Initialization, Intrlay, Scalar, ScalarMut,
    SizeCursor, Validator, Writer,
};

macro_rules! size_codec {
    ($codec:ident, $mutable:ident, $native:ty, $wire:ty) => {
        #[doc = concat!("Fixed-width `", stringify!($wire), "` codec for `", stringify!($native), "` components.")]
        pub struct $codec;
        impl Describe for $codec {
            fn describe(registry: &mut Registry) -> Type { <$wire>::describe(registry) }
        }
        #[doc = concat!("A mutable `", stringify!($native), "` component with a fixed-width encoding.")]
        pub struct $mutable<'a>(ScalarMut<'a, $wire>);
        impl $mutable<'_> {
            /// Reads the component, whose native range was checked when opening the view.
            #[must_use]
            #[allow(clippy::cast_possible_truncation, reason = "validated values fit the native component")]
            pub fn get(&self) -> $native { self.0.get() as $native }
            /// Stores a native value, which is always representable by the wider wire type.
            pub fn set(&mut self, value: $native) { self.0.set(value as $wire); }
        }
        // SAFETY: Values are sign/zero extended into a fixed-width field. Checked
        // entry points reject values outside the native range, and setters accept
        // only native values, preserving that invariant on 32-bit hosts too.
        unsafe impl Intrlay for $codec {
            type Value = $native;
            type View<'a> = $native;
            type ViewMut<'a> = $mutable<'a>;
            const LAYOUT: crate::layout::Layout = <$wire>::LAYOUT;
            const NATIVE_LAYOUT: bool = cfg!(target_endian = "little") && <$native>::BITS == 64;
            const ALL_BITS_VALID: bool = <$native>::BITS == 64;
            const ZEROABLE: bool = true;
            const WRITES_FIXED_LAYOUT: bool = true;
            const IN_PLACE_WRITE: bool = true;
            const INITIALIZATION: Initialization = Initialization::FIXED;
            fn measure(_: &$native, _: &mut SizeCursor) -> Result<(), EncodeError> { Ok(()) }
            fn write(value: &$native, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
                <$wire as Intrlay>::write(&(*value as $wire), writer, offset)
            }
            #[allow(clippy::cast_possible_truncation, reason = "the unsafe contract includes representability on this host")]
            unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> $native { <$wire>::read(bytes, offset) as $native }
            unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> $mutable<'_> {
                // SAFETY: The caller guarantees valid exclusive scalar storage.
                $mutable(unsafe { <$wire>::view_mut_unchecked(bytes, offset) })
            }
            fn validate_fields(validator: &mut Validator<'_>, offset: usize) -> Result<(), DecodeError> {
                <$native>::try_from(<$wire>::from_le_bytes(validator.read(offset)?))
                    .map(|_| ()).map_err(|_| DecodeError::ValueOutOfRange { offset })
            }
            fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<$native, DecodeError> {
                <$native>::try_from(<$wire>::from_le_bytes(decoder.read(offset)?))
                    .map_err(|_| DecodeError::ValueOutOfRange { offset })
            }
            unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> $native {
                // SAFETY: The caller supplies the validated representation and native range.
                unsafe { Self::view_unchecked(bytes, offset) }
            }
        }
        // SAFETY: Delegates the scalar representation and all proofs to the codec.
        unsafe impl EncodeSource<$codec> for $native {
            const NATIVE_LAYOUT: bool = $codec::NATIVE_LAYOUT;
            const WRITES_FIXED_LAYOUT: bool = true;
            const INITIALIZATION: Initialization = Initialization::FIXED;
            fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> { $codec::measure(self, cursor) }
            fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> { $codec::write(self, writer, offset) }
        }
    };
}
size_codec!(SignedSize, SignedSizeMut, isize, i64);
size_codec!(UnsignedSize, UnsignedSizeMut, usize, u64);
