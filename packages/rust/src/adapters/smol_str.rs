//! The `smol_str` feature maps small strings to the standard string encoding.

use crate::schema::{Describe, Registry, Type};
use crate::{
    DecodeError, Decoder, EncodeError, EncodeSource, Initialization, Intrlay, SizeCursor,
    Validator, Writer,
};

/// Codec for [`smol_str::SmolStr`] using exactly the [`String`] wire layout.
/// Encoding borrows the UTF-8 contents; decoding constructs the small string
/// directly without an intermediate `String` allocation.
pub struct SmolStr;

impl Describe for SmolStr {
    fn describe(registry: &mut Registry) -> Type {
        String::describe(registry)
    }
}

// SAFETY: The standard String codec defines the representation and validation.
// SmolStr supplies valid UTF-8 and copies only when materializing its owned value.
unsafe impl Intrlay for SmolStr {
    type Value = smol_str::SmolStr;
    type View<'a> = &'a str;
    type ViewMut<'a> = &'a str;
    const LAYOUT: crate::layout::Layout = String::LAYOUT;
    const INITIALIZATION: Initialization = <String as Intrlay>::INITIALIZATION;
    fn measure(value: &Self::Value, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        <str as EncodeSource<String>>::measure(value.as_str(), cursor)
    }
    fn write(
        value: &Self::Value,
        writer: &mut Writer<'_>,
        offset: usize,
    ) -> Result<(), EncodeError> {
        <str as EncodeSource<String>>::write(value.as_str(), writer, offset)
    }
    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> &str {
        // SAFETY: The caller supplies the standard validated string representation.
        unsafe { String::view_unchecked(bytes, offset) }
    }
    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> &str {
        // SAFETY: Strings expose only immutable contents within the exclusive borrow.
        unsafe { String::view_unchecked(bytes, offset) }
    }
    fn validate_fields(validator: &mut Validator<'_>, offset: usize) -> Result<(), DecodeError> {
        String::validate_fields(validator, offset)
    }
    fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<Self::Value, DecodeError> {
        Ok(smol_str::SmolStr::new(decoder.string(offset)?))
    }
    unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> Self::Value {
        // SAFETY: The caller guarantees the complete UTF-8 string encoding.
        smol_str::SmolStr::new(unsafe { String::view_unchecked(bytes, offset) })
    }
}

macro_rules! source {
    ($source:ty, $codec:ty) => {
        // SAFETY: All supported sources borrow valid UTF-8 and use String's writer.
        unsafe impl EncodeSource<$codec> for $source {
            const INITIALIZATION: Initialization = <String as Intrlay>::INITIALIZATION;
            fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
                <str as EncodeSource<String>>::measure(self.as_ref(), cursor)
            }
            fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
                <str as EncodeSource<String>>::write(self.as_ref(), writer, offset)
            }
        }
    };
}
source!(smol_str::SmolStr, SmolStr);
source!(String, SmolStr);
source!(str, SmolStr);
source!(smol_str::SmolStr, String);
