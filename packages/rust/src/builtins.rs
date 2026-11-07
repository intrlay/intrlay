use std::marker::PhantomData;

use intrlay_layout::{Layout, field_offset};

use crate::{EncodeError, Initialization, Intrlay, SequenceMut, SequenceView, SizeCursor, Writer};

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
            const WRITES_FIXED_LAYOUT: bool = true;
            const IN_PLACE_WRITE: bool = true;
            const INITIALIZATION: Initialization = Initialization::FIXED;
            fn measure(_: &Self, _: &mut SizeCursor) -> Result<(), EncodeError> { Ok(()) }
            fn write(value: &Self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
                writer.write(offset, &value.to_le_bytes());
                Ok(())
            }
            #[inline]
            unsafe fn view(bytes: &[u8], offset: usize) -> Self {
                // SAFETY: The view contract guarantees the complete initialized scalar region.
                // Read bytes rather than a native reference to permit unaligned foreign buffers.
                let value = unsafe {
                    bytes.as_ptr().add(offset).cast::<[u8; size_of::<Self>()]>().read_unaligned()
                };
                Self::from_le_bytes(value)
            }
            #[inline]
            unsafe fn view_mut(bytes: &mut [u8], offset: usize) -> ScalarMut<'_, Self> { ScalarMut::new(bytes, offset) }
            #[inline]
            unsafe fn decode(bytes: &[u8], offset: usize) -> Self {
                // SAFETY: Decode has the same complete-encoding contract as view.
                unsafe { Self::view(bytes, offset) }
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
    type Value = Self;
    type View<'a> = Self;
    type ViewMut<'a> = ScalarMut<'a, Self>;
    const LAYOUT: Layout = Layout::new(1, 1);
    const NATIVE_LAYOUT: bool = true;
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
    unsafe fn view(bytes: &[u8], offset: usize) -> Self {
        Self::read(bytes, offset)
    }
    unsafe fn view_mut(bytes: &mut [u8], offset: usize) -> ScalarMut<'_, Self> {
        ScalarMut::new(bytes, offset)
    }
    unsafe fn decode(bytes: &[u8], offset: usize) -> Self {
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
    type Value = Self;
    type View<'a> = ();
    type ViewMut<'a> = ();
    const LAYOUT: Layout = Layout::UNIT;
    const NATIVE_LAYOUT: bool = true;
    const WRITES_FIXED_LAYOUT: bool = true;
    const IN_PLACE_WRITE: bool = true;
    const INITIALIZATION: Initialization = Initialization::FIXED;
    fn measure((): &Self, _: &mut SizeCursor) -> Result<(), EncodeError> {
        Ok(())
    }
    fn write((): &Self, _: &mut Writer<'_>, _: usize) -> Result<(), EncodeError> {
        Ok(())
    }
    unsafe fn view(_: &[u8], _: usize) {}
    unsafe fn view_mut(_: &mut [u8], _: usize) {}
    unsafe fn decode(_: &[u8], _: usize) {}
}

fn reference(bytes: &[u8], offset: usize) -> usize {
    offset + u32::read(bytes, offset) as usize
}
fn count(bytes: &[u8], offset: usize) -> usize {
    u32::read(bytes, offset + 4) as usize
}

// SAFETY: String construction writes valid UTF-8 and immutable relative metadata.
unsafe impl Intrlay for String {
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
    unsafe fn view(bytes: &[u8], offset: usize) -> &str {
        let start = reference(bytes, offset);
        let length = count(bytes, offset);
        // SAFETY: Intrlay's trusted input contract requires valid UTF-8 in this region.
        unsafe { std::str::from_utf8_unchecked(&bytes[start..start + length]) }
    }
    unsafe fn view_mut(bytes: &mut [u8], offset: usize) -> &str {
        // SAFETY: The exclusive storage borrow also permits an immutable string reborrow.
        unsafe { Self::view(bytes, offset) }
    }
    unsafe fn decode(bytes: &[u8], offset: usize) -> Self {
        // SAFETY: The caller guarantees the same valid encoding required by view.
        unsafe { Self::view(bytes, offset) }.to_owned()
    }
}

// SAFETY: Elements are reserved contiguously before their payloads and decoded by their codecs.
unsafe impl<C: Intrlay> Intrlay for Vec<C> {
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
        cursor.reserve(C::LAYOUT, value.len())?;
        for element in value {
            C::measure(element, cursor)?;
        }
        Ok(())
    }
    fn write(
        value: &Self::Value,
        writer: &mut Writer<'_>,
        offset: usize,
    ) -> Result<(), EncodeError> {
        let target = writer.reserve(C::LAYOUT, value.len())?;
        writer.reference(offset, target)?;
        writer.count(offset + 4, value.len())?;
        if C::NATIVE_LAYOUT {
            // SAFETY: Each native element has the same initialized bytes, size, and
            // byte order as its encoding. Vec stores these elements contiguously.
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    value.as_ptr().cast::<u8>(),
                    size_of_val(value.as_slice()),
                )
            };
            writer.write(target, bytes);
        } else {
            for (index, element) in value.iter().enumerate() {
                C::write(element, writer, target + index * C::LAYOUT.size())?;
            }
        }
        Ok(())
    }
    unsafe fn view(bytes: &[u8], offset: usize) -> Self::View<'_> {
        // SAFETY: Valid vector metadata identifies valid fixed-stride element headers.
        unsafe {
            SequenceView::new_unchecked(bytes, reference(bytes, offset), count(bytes, offset))
        }
    }
    unsafe fn view_mut(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_> {
        let target = reference(bytes, offset);
        let length = count(bytes, offset);
        // SAFETY: The caller grants exclusive access to the valid vector and descendants.
        unsafe { SequenceMut::new_unchecked(bytes, target, length) }
    }
    unsafe fn decode(bytes: &[u8], offset: usize) -> Self::Value {
        let target = reference(bytes, offset);
        (0..count(bytes, offset))
            .map(|index| {
                // SAFETY: Each element belongs to the valid vector region.
                unsafe { C::decode(bytes, target + index * C::LAYOUT.size()) }
            })
            .collect()
    }
}

// SAFETY: Inline arrays reserve a fixed stride and delegate payload validity to each codec.
unsafe impl<C: Intrlay, const N: usize> Intrlay for [C; N] {
    type Value = [C::Value; N];
    type View<'a> = SequenceView<'a, C>;
    type ViewMut<'a> = SequenceMut<'a, C>;
    const LAYOUT: Layout = C::LAYOUT.array(N);
    const NATIVE_LAYOUT: bool = C::NATIVE_LAYOUT;
    const HAS_PAYLOAD: bool = N != 0 && C::HAS_PAYLOAD;
    const IN_PLACE_WRITE: bool = N == 0 || C::IN_PLACE_WRITE;
    const WRITES_FIXED_LAYOUT: bool = N == 0 || C::WRITES_FIXED_LAYOUT;
    const INITIALIZATION: Initialization = if N == 0 {
        Initialization::FIXED
    } else {
        C::INITIALIZATION
    };
    fn measure(value: &Self::Value, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        for element in value {
            C::measure(element, cursor)?;
        }
        Ok(())
    }
    fn write(
        value: &Self::Value,
        writer: &mut Writer<'_>,
        offset: usize,
    ) -> Result<(), EncodeError> {
        for (index, element) in value.iter().enumerate() {
            C::write(element, writer, offset + index * C::LAYOUT.size())?;
        }
        Ok(())
    }
    unsafe fn view(bytes: &[u8], offset: usize) -> Self::View<'_> {
        // SAFETY: All N inline headers are part of the caller's valid array encoding.
        unsafe { SequenceView::new_unchecked(bytes, offset, N) }
    }
    unsafe fn view_mut(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_> {
        // SAFETY: All N inline headers are valid and storage is exclusively borrowed.
        unsafe { SequenceMut::new_unchecked(bytes, offset, N) }
    }
    unsafe fn decode(bytes: &[u8], offset: usize) -> Self::Value {
        std::array::from_fn(|index| {
            // SAFETY: array::from_fn only supplies indices below N.
            unsafe { C::decode(bytes, offset + index * C::LAYOUT.size()) }
        })
    }
}

// SAFETY: Presence selects a valid inline value. Replacement exclusively rewrites
// presence and fixed payloads; indirect allocation and reference offsets stay fixed.
unsafe impl<C: Intrlay> Intrlay for Option<C> {
    type Value = Option<C::Value>;
    type View<'a> = Option<C::View<'a>>;
    type ViewMut<'a> = Option<C::ViewMut<'a>>;
    const LAYOUT: Layout = Layout::tagged(Layout::new(1, 1), C::LAYOUT);
    const HAS_PAYLOAD: bool = C::HAS_PAYLOAD;
    const IN_PLACE_WRITE: bool = C::IN_PLACE_WRITE;
    fn measure(value: &Self::Value, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        if let Some(value) = value {
            C::measure(value, cursor)?;
        }
        Ok(())
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
            )?;
        }
        Ok(())
    }
    unsafe fn view(bytes: &[u8], offset: usize) -> Self::View<'_> {
        if bytes[offset] == 0 {
            return None;
        }
        // SAFETY: A present option contains a valid inline value at its aligned offset.
        Some(unsafe {
            C::view(
                bytes,
                offset + field_offset(&[Layout::new(1, 1), C::LAYOUT], 1),
            )
        })
    }
    unsafe fn view_mut(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_> {
        if bytes[offset] == 0 {
            return None;
        }
        // SAFETY: The existing variant remains unchanged and its payload is exclusively borrowed.
        Some(unsafe {
            C::view_mut(
                bytes,
                offset + field_offset(&[Layout::new(1, 1), C::LAYOUT], 1),
            )
        })
    }
    unsafe fn decode(bytes: &[u8], offset: usize) -> Self::Value {
        if bytes[offset] == 0 {
            return None;
        }
        // SAFETY: The caller guarantees a valid payload for a present option.
        Some(unsafe {
            C::decode(
                bytes,
                offset + field_offset(&[Layout::new(1, 1), C::LAYOUT], 1),
            )
        })
    }
}

// SAFETY: Box stores one forward reference to an independently reserved, valid value.
unsafe impl<C: Intrlay> Intrlay for Box<C> {
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
    unsafe fn view(bytes: &[u8], offset: usize) -> Self::View<'_> {
        // SAFETY: Valid box metadata refers to a valid encoded C in the same buffer.
        unsafe { C::view(bytes, reference(bytes, offset)) }
    }
    unsafe fn view_mut(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_> {
        let target = reference(bytes, offset);
        // SAFETY: The caller grants exclusive access to this tree, which contains no shared edges.
        unsafe { C::view_mut(bytes, target) }
    }
    unsafe fn decode(bytes: &[u8], offset: usize) -> Self::Value {
        // SAFETY: The referenced value is valid under the box's trusted input contract.
        Box::new(unsafe { C::decode(bytes, reference(bytes, offset)) })
    }
}
