use std::marker::PhantomData;

use intrlay_layout::{Layout, field_offset};

use crate::{
    DecodeError, Decoder, EncodeError, Initialization, Intrlay, SequenceMut, SequenceView,
    SizeCursor, Writer,
};

/// A fixed-width native scalar that can be overwritten without changing a layout.
pub trait Scalar: Intrlay<Value = Self> + Copy {
    /// Reads one scalar from its encoded bytes.
    ///
    /// # Panics
    /// Panics if the scalar extends outside the supplied slice.
    fn read(bytes: &[u8], offset: usize) -> Self;
    /// Overwrites one scalar without modifying adjacent bytes.
    ///
    /// # Panics
    /// Panics if the scalar extends outside the supplied slice.
    fn store(self, bytes: &mut [u8], offset: usize);
}

/// An exclusive scalar field inside an encoded object.
pub struct ScalarMut<'a, T: Scalar> {
    bytes: &'a mut [u8],
    offset: usize,
    value: PhantomData<T>,
}

impl<'a, T: Scalar> ScalarMut<'a, T> {
    fn new(bytes: &'a mut [u8], offset: usize) -> Self {
        Self {
            bytes,
            offset,
            value: PhantomData,
        }
    }
    /// Reads the current scalar value.
    #[must_use]
    pub fn get(&self) -> T {
        T::read(self.bytes, self.offset)
    }
    /// Replaces the scalar value without changing the buffer layout.
    pub fn set(&mut self, value: T) {
        value.store(self.bytes, self.offset);
    }
}

macro_rules! scalar {
    ($($ty:ty),* $(,)?) => {$(
        // SAFETY: Numeric scalars have fixed, initialized little-endian encodings.
        unsafe impl Intrlay for $ty {
            type Value = Self;
            type View<'a> = Self;
            type ViewMut<'a> = ScalarMut<'a, Self>;
            const LAYOUT: Layout = Layout::new(size_of::<Self>(), size_of::<Self>());
            const NATIVE_LAYOUT: bool = cfg!(target_endian = "little");
            const ALL_BITS_VALID: bool = true;
            const FIXED_DECODE_DEPTH: Option<usize> = Some(1);
            const ZEROABLE: bool = true;
            const WRITES_FIXED_LAYOUT: bool = true;
            const IN_PLACE_WRITE: bool = true;
            const INITIALIZATION: Initialization = Initialization::FIXED;
            fn measure(_: &Self, _: &mut SizeCursor) -> Result<(), EncodeError> { Ok(()) }
            fn write(value: &Self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
                writer.write(offset, &value.to_le_bytes());
                Ok(())
            }
            #[inline]
            unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> Self {
                // SAFETY: The view contract guarantees the complete initialized scalar region.
                // Read bytes rather than a native reference to permit unaligned foreign buffers.
                let value = unsafe {
                    bytes.as_ptr().add(offset).cast::<[u8; size_of::<Self>()]>().read_unaligned()
                };
                Self::from_le_bytes(value)
            }
            #[inline]
            unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> ScalarMut<'_, Self> { ScalarMut::new(bytes, offset) }
            fn validate_fields(_: &mut crate::Validator<'_>, _: usize) -> Result<(), DecodeError> { Ok(()) }
            #[inline]
            fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<Self, DecodeError> {
                Ok(Self::from_le_bytes(decoder.read(offset)?))
            }
            #[inline]
            unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> Self {
                // SAFETY: Decode has the same complete-encoding contract as view.
                unsafe { Self::view_unchecked(bytes, offset) }
            }
        }
        impl Scalar for $ty {
            #[inline]
            fn read(bytes: &[u8], offset: usize) -> Self {
                let mut value = [0; size_of::<Self>()];
                value.copy_from_slice(&bytes[offset..offset + size_of::<Self>()]);
                Self::from_le_bytes(value)
            }
            #[inline]
            fn store(self, bytes: &mut [u8], offset: usize) {
                bytes[offset..offset + size_of::<Self>()].copy_from_slice(&self.to_le_bytes());
            }
        }
    )*};
}
scalar!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);

// SAFETY: Booleans use one initialized byte. Reading produces a valid native bool.
unsafe impl Intrlay for bool {
    fn validate_fields(
        validator: &mut crate::Validator<'_>,
        offset: usize,
    ) -> Result<(), DecodeError> {
        match validator.read::<1>(offset)?[0] {
            0 | 1 => Ok(()),
            tag => Err(DecodeError::InvalidTag {
                offset,
                tag: u32::from(tag),
            }),
        }
    }

    type Value = Self;
    type View<'a> = Self;
    type ViewMut<'a> = ScalarMut<'a, Self>;
    const LAYOUT: Layout = Layout::new(1, 1);
    const NATIVE_LAYOUT: bool = true;
    const ZEROABLE: bool = true;
    const WRITES_FIXED_LAYOUT: bool = true;
    const IN_PLACE_WRITE: bool = true;
    const INITIALIZATION: Initialization = Initialization::FIXED;
    fn measure(_: &Self, _: &mut SizeCursor) -> Result<(), EncodeError> {
        Ok(())
    }
    fn write(value: &Self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        writer.write(offset, &[u8::from(*value)]);
        Ok(())
    }
    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> Self {
        Self::read(bytes, offset)
    }
    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> ScalarMut<'_, Self> {
        ScalarMut::new(bytes, offset)
    }
    fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<Self, DecodeError> {
        match decoder.read::<1>(offset)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(DecodeError::InvalidTag {
                offset,
                tag: u32::from(tag),
            }),
        }
    }
    unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> Self {
        Self::read(bytes, offset)
    }
}
impl Scalar for bool {
    fn read(bytes: &[u8], offset: usize) -> Self {
        bytes[offset] != 0
    }
    fn store(self, bytes: &mut [u8], offset: usize) {
        bytes[offset] = u8::from(self);
    }
}

// SAFETY: Unit encodes no bytes and borrows no storage.
unsafe impl Intrlay for () {
    const ALL_BITS_VALID: bool = true;
    fn validate_fields(_: &mut crate::Validator<'_>, _: usize) -> Result<(), DecodeError> {
        Ok(())
    }

    type Value = Self;
    type View<'a> = ();
    type ViewMut<'a> = ();
    const LAYOUT: Layout = Layout::UNIT;
    const NATIVE_LAYOUT: bool = true;
    const ZEROABLE: bool = true;
    const WRITES_FIXED_LAYOUT: bool = true;
    const IN_PLACE_WRITE: bool = true;
    const INITIALIZATION: Initialization = Initialization::FIXED;
    fn measure((): &Self, _: &mut SizeCursor) -> Result<(), EncodeError> {
        Ok(())
    }
    fn write((): &Self, _: &mut Writer<'_>, _: usize) -> Result<(), EncodeError> {
        Ok(())
    }
    unsafe fn view_unchecked(_: &[u8], _: usize) {}
    unsafe fn view_mut_unchecked(_: &mut [u8], _: usize) {}
    fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<Self, DecodeError> {
        decoder.range(offset, 0)?;
        Ok(())
    }
    unsafe fn decode_unchecked(_: &[u8], _: usize) {}
}

// These helpers are only for codecs operating on a complete valid encoding.
// Checked decoding and validation resolve headers through Decoder/Validator.
#[inline]
unsafe fn reference_unchecked(bytes: &[u8], offset: usize) -> usize {
    // SAFETY: The caller supplies a valid reference header and in-bounds target.
    offset + unsafe { u32::view_unchecked(bytes, offset) } as usize
}
#[inline]
unsafe fn count_unchecked(bytes: &[u8], offset: usize) -> usize {
    // SAFETY: The caller supplies a complete eight-byte sequence header.
    (unsafe { u32::view_unchecked(bytes, offset + 4) }) as usize
}

// SAFETY: String construction writes valid UTF-8 and immutable relative metadata.
unsafe impl Intrlay for String {
    fn validate_fields(
        validator: &mut crate::Validator<'_>,
        offset: usize,
    ) -> Result<(), DecodeError> {
        let length = u32::from_le_bytes(validator.read(offset + 4)?) as usize;
        let target = validator.payload(offset, 8, length, 1)?;
        std::str::from_utf8(validator.range(target, length)?).map_err(|source| {
            DecodeError::InvalidUtf8 {
                offset: target,
                source,
            }
        })?;
        Ok(())
    }

    type Value = Self;
    type View<'a> = &'a str;
    type ViewMut<'a> = &'a str;
    const LAYOUT: Layout = Layout::SEQUENCE;
    const INITIALIZATION: Initialization =
        Initialization::payload(Layout::new(1, 1), Initialization::FIXED);
    fn measure(value: &Self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        cursor.reserve(Layout::new(1, 1), value.len())?;
        Ok(())
    }
    fn write(value: &Self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        let target = writer.reserve(Layout::new(1, 1), value.len())?;
        writer.reference(offset, target)?;
        writer.count(offset + 4, value.len())?;
        writer.write(target, value.as_bytes());
        Ok(())
    }
    #[inline]
    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> &str {
        // SAFETY: The codec contract guarantees a complete header whose target
        // region is in bounds and valid UTF-8. A checked view established this
        // before exposing field access; an unchecked caller guarantees it directly.
        unsafe {
            let start = reference_unchecked(bytes, offset);
            let length = count_unchecked(bytes, offset);
            std::str::from_utf8_unchecked(bytes.get_unchecked(start..start + length))
        }
    }
    #[inline]
    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> &str {
        // SAFETY: The exclusive storage borrow also permits an immutable string reborrow.
        unsafe { Self::view_unchecked(bytes, offset) }
    }
    fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<Self, DecodeError> {
        Ok(decoder.string(offset)?.to_owned())
    }
    unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> Self {
        // SAFETY: The caller guarantees the same valid encoding required by view.
        unsafe { Self::view_unchecked(bytes, offset) }.to_owned()
    }
}

// SAFETY: Elements are reserved contiguously before their payloads and decoded by their codecs.
unsafe impl<C: Intrlay> Intrlay for Vec<C> {
    fn validate_fields(
        validator: &mut crate::Validator<'_>,
        offset: usize,
    ) -> Result<(), DecodeError> {
        let count = u32::from_le_bytes(validator.read(offset + 4)?) as usize;
        let target = validator.payload(offset, 8, count, C::LAYOUT.size())?;
        validator.elements::<C>(target, count)
    }

    type Value = Vec<C::Value>;
    type View<'a> = SequenceView<'a, C>;
    type ViewMut<'a> = SequenceMut<'a, C>;
    const LAYOUT: Layout = Layout::SEQUENCE;
    // Do not recursively evaluate element payload metadata: Vec<Self> and
    // mutually recursive records are valid source types. Fixed element codecs
    // provide a terminating proof; dynamic elements retain whole-buffer clearing.
    const INITIALIZATION: Initialization = if C::WRITES_FIXED_LAYOUT {
        Initialization::payload(C::LAYOUT, Initialization::FIXED)
    } else {
        Initialization::UNKNOWN
    };
    fn measure(value: &Self::Value, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        measure_sequence::<C>(value, cursor)
    }
    fn write(
        value: &Self::Value,
        writer: &mut Writer<'_>,
        offset: usize,
    ) -> Result<(), EncodeError> {
        write_sequence::<C>(value, writer, offset)
    }
    #[inline]
    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> Self::View<'_> {
        // SAFETY: Valid vector metadata identifies valid fixed-stride element headers.
        unsafe {
            SequenceView::new_unchecked(
                bytes,
                reference_unchecked(bytes, offset),
                count_unchecked(bytes, offset),
            )
        }
    }
    #[inline]
    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_> {
        // SAFETY: The caller grants exclusive access to the valid vector and descendants.
        unsafe {
            let target = reference_unchecked(bytes, offset);
            let length = count_unchecked(bytes, offset);
            SequenceMut::new_unchecked(bytes, target, length)
        }
    }
    fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<Self::Value, DecodeError> {
        let target = decoder.reference(offset, Self::LAYOUT.size())?;
        let length = u32::from_le_bytes(decoder.read(offset + 4)?) as usize;
        decoder.array_region(target, length, C::LAYOUT.size())?;
        let mut values: Vec<C::Value> = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| DecodeError::AllocationFailed)?;
        if decoder.decode_fixed_elements::<C>(target, &mut values.spare_capacity_mut()[..length])? {
            // SAFETY: Fixed-element reconstruction initialized every reserved element.
            unsafe { values.set_len(length) };
        } else {
            for index in 0..length {
                values.push(decoder.decode::<C>(target + index * C::LAYOUT.size())?);
            }
        }
        Ok(values)
    }
    unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> Self::Value {
        // SAFETY: The caller guarantees the full vector header and payload.
        let (target, length) = unsafe {
            (
                reference_unchecked(bytes, offset),
                count_unchecked(bytes, offset),
            )
        };
        (0..length)
            .map(|index| {
                // SAFETY: Each element belongs to the valid vector region.
                unsafe { C::decode_unchecked(bytes, target + index * C::LAYOUT.size()) }
            })
            .collect()
    }
}

/// Shared owned-value sequence encoding, independent of the native container.
pub(crate) fn measure_sequence<C: Intrlay>(
    values: &[C::Value],
    cursor: &mut SizeCursor,
) -> Result<(), EncodeError> {
    cursor.reserve(C::LAYOUT, values.len())?;
    if !C::WRITES_FIXED_LAYOUT {
        for value in values {
            C::measure(value, cursor)?;
        }
    }
    Ok(())
}

pub(crate) fn write_sequence<C: Intrlay>(
    values: &[C::Value],
    writer: &mut Writer<'_>,
    offset: usize,
) -> Result<(), EncodeError> {
    let target = writer.reserve(C::LAYOUT, values.len())?;
    writer.reference(offset, target)?;
    writer.count(offset + 4, values.len())?;
    if C::NATIVE_LAYOUT {
        // SAFETY: C guarantees that each native element has fully initialized,
        // byte-identical storage. A slice adds no gaps between those elements.
        let bytes = unsafe {
            std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), size_of_val(values))
        };
        writer.write(target, bytes);
    } else {
        for (index, value) in values.iter().enumerate() {
            C::write(value, writer, target + index * C::LAYOUT.size())?;
        }
    }
    Ok(())
}

// SAFETY: Inline arrays reserve a fixed stride and delegate payload validity to each codec.
unsafe impl<C: Intrlay, const N: usize> Intrlay for [C; N] {
    const ALL_BITS_VALID: bool = N == 0 || C::ALL_BITS_VALID;
    const FIXED_DECODE_DEPTH: Option<usize> =
        if N == 0 || (Self::NATIVE_LAYOUT && Self::ALL_BITS_VALID) {
            Some(1)
        } else if Self::ALL_BITS_VALID {
            crate::decode::fixed_decode_depth(&[C::FIXED_DECODE_DEPTH])
        } else {
            None
        };
    fn validate_fields(
        validator: &mut crate::Validator<'_>,
        offset: usize,
    ) -> Result<(), DecodeError> {
        validator.elements::<C>(offset, N)
    }

    type Value = [C::Value; N];
    type View<'a> = SequenceView<'a, C>;
    type ViewMut<'a> = SequenceMut<'a, C>;
    const LAYOUT: Layout = C::LAYOUT.array(N);
    const NATIVE_LAYOUT: bool = C::NATIVE_LAYOUT;
    const HAS_PAYLOAD: bool = N != 0 && C::HAS_PAYLOAD;
    const ZEROABLE: bool = N == 0 || C::ZEROABLE;
    const IN_PLACE_WRITE: bool = N == 0 || C::IN_PLACE_WRITE;
    const WRITES_FIXED_LAYOUT: bool = N == 0 || C::WRITES_FIXED_LAYOUT;
    const INITIALIZATION: Initialization = if N == 0 {
        Initialization::FIXED
    } else {
        C::INITIALIZATION
    };
    fn measure(value: &Self::Value, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        if !C::WRITES_FIXED_LAYOUT {
            for element in value {
                C::measure(element, cursor)?;
            }
        }
        Ok(())
    }
    fn write(
        value: &Self::Value,
        writer: &mut Writer<'_>,
        offset: usize,
    ) -> Result<(), EncodeError> {
        if Self::NATIVE_LAYOUT {
            // SAFETY: Native array elements are contiguous, fully initialized,
            // and byte-identical to their encoding, including nested arrays.
            unsafe { crate::__private::write_native(value, writer, offset) };
        } else {
            for (index, element) in value.iter().enumerate() {
                C::write(element, writer, offset + index * C::LAYOUT.size())?;
            }
        }
        Ok(())
    }
    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> Self::View<'_> {
        // SAFETY: All N inline headers are part of the caller's valid array encoding.
        unsafe { SequenceView::new_unchecked(bytes, offset, N) }
    }
    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_> {
        // SAFETY: All N inline headers are valid and storage is exclusively borrowed.
        unsafe { SequenceMut::new_unchecked(bytes, offset, N) }
    }
    fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<Self::Value, DecodeError> {
        decoder.fixed_array::<C, N>(offset)
    }
    unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> Self::Value {
        if Self::NATIVE_LAYOUT {
            // SAFETY: The caller guarantees valid elements; their contiguous
            // native layout is identical even when not every bit pattern is valid.
            return unsafe { crate::__private::read_native(bytes, offset) };
        }
        std::array::from_fn(|index| {
            // SAFETY: array::from_fn only supplies indices below N.
            unsafe { C::decode_unchecked(bytes, offset + index * C::LAYOUT.size()) }
        })
    }
}

// SAFETY: Presence selects a valid inline value. Replacement exclusively rewrites
// presence and fixed payloads; indirect allocation and reference offsets stay fixed.
unsafe impl<C: Intrlay> Intrlay for Option<C> {
    fn validate_fields(
        validator: &mut crate::Validator<'_>,
        offset: usize,
    ) -> Result<(), DecodeError> {
        match validator.read::<1>(offset)?[0] {
            0 => Ok(()),
            1 => validator.field::<C>(offset + field_offset(&[Layout::new(1, 1), C::LAYOUT], 1)),
            tag => Err(DecodeError::InvalidTag {
                offset,
                tag: u32::from(tag),
            }),
        }
    }

    type Value = Option<C::Value>;
    type View<'a> = Option<C::View<'a>>;
    type ViewMut<'a> = Option<C::ViewMut<'a>>;
    const LAYOUT: Layout = Layout::tagged(Layout::new(1, 1), C::LAYOUT);
    const HAS_PAYLOAD: bool = C::HAS_PAYLOAD;
    const IN_PLACE_WRITE: bool = C::IN_PLACE_WRITE;
    fn measure(value: &Self::Value, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        if let Some(value) = value {
            C::measure(value, cursor)
        } else {
            Ok(())
        }
    }
    fn write(
        value: &Self::Value,
        writer: &mut Writer<'_>,
        offset: usize,
    ) -> Result<(), EncodeError> {
        writer.write(offset, &[u8::from(value.is_some())]);
        if let Some(value) = value {
            C::write(
                value,
                writer,
                offset + field_offset(&[Layout::new(1, 1), C::LAYOUT], 1),
            )
        } else {
            Ok(())
        }
    }
    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> Self::View<'_> {
        if bytes[offset] == 0 {
            return None;
        }
        // SAFETY: A present option contains a valid inline value at its aligned offset.
        Some(unsafe {
            C::view_unchecked(
                bytes,
                offset + field_offset(&[Layout::new(1, 1), C::LAYOUT], 1),
            )
        })
    }
    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_> {
        if bytes[offset] == 0 {
            return None;
        }
        // SAFETY: The existing variant remains unchanged and its payload is exclusively borrowed.
        Some(unsafe {
            C::view_mut_unchecked(
                bytes,
                offset + field_offset(&[Layout::new(1, 1), C::LAYOUT], 1),
            )
        })
    }
    fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<Self::Value, DecodeError> {
        decoder.range(offset, Self::LAYOUT.size())?;
        match decoder.read::<1>(offset)?[0] {
            0 => Ok(None),
            1 => decoder
                .decode::<C>(offset + field_offset(&[Layout::new(1, 1), C::LAYOUT], 1))
                .map(Some),
            tag => Err(DecodeError::InvalidTag {
                offset,
                tag: u32::from(tag),
            }),
        }
    }
    unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> Self::Value {
        if bytes[offset] == 0 {
            return None;
        }
        // SAFETY: The caller guarantees a valid payload for a present option.
        Some(unsafe {
            C::decode_unchecked(
                bytes,
                offset + field_offset(&[Layout::new(1, 1), C::LAYOUT], 1),
            )
        })
    }
}

// SAFETY: Box stores one forward reference to an independently reserved, valid value.
unsafe impl<C: Intrlay> Intrlay for Box<C> {
    fn validate_fields(
        validator: &mut crate::Validator<'_>,
        offset: usize,
    ) -> Result<(), DecodeError> {
        let target = validator.payload(offset, 4, 1, C::LAYOUT.size())?;
        validator.field::<C>(target)
    }

    type Value = Box<C::Value>;
    type View<'a> = C::View<'a>;
    type ViewMut<'a> = C::ViewMut<'a>;
    const LAYOUT: Layout = Layout::BOX;
    const INITIALIZATION: Initialization = if C::WRITES_FIXED_LAYOUT {
        Initialization::payload(C::LAYOUT, Initialization::FIXED)
    } else {
        Initialization::UNKNOWN
    };
    fn measure(value: &Self::Value, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        cursor.reserve(C::LAYOUT, 1)?;
        C::measure(value, cursor)
    }
    fn write(
        value: &Self::Value,
        writer: &mut Writer<'_>,
        offset: usize,
    ) -> Result<(), EncodeError> {
        let target = writer.reserve(C::LAYOUT, 1)?;
        writer.reference(offset, target)?;
        C::write(value, writer, target)
    }
    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> Self::View<'_> {
        // SAFETY: Valid box metadata refers to a valid encoded C in the same buffer.
        unsafe { C::view_unchecked(bytes, reference_unchecked(bytes, offset)) }
    }
    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_> {
        // SAFETY: The caller grants exclusive access to this tree, which contains no shared edges.
        unsafe {
            let target = reference_unchecked(bytes, offset);
            C::view_mut_unchecked(bytes, target)
        }
    }
    fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<Self::Value, DecodeError> {
        let target = decoder.reference(offset, Self::LAYOUT.size())?;
        decoder.decode::<C>(target).map(Box::new)
    }
    unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> Self::Value {
        // SAFETY: The referenced value is valid under the box's trusted input contract.
        Box::new(unsafe { C::decode_unchecked(bytes, reference_unchecked(bytes, offset)) })
    }
}
