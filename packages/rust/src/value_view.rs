use std::marker::PhantomData;

use crate::{EncodeError, Intrlay, OwnedBuffer};

/// Buffer entry point for explicitly exported scalar, optional, and collection codecs.
///
/// `C` selects the representation and `B` carries the borrow or allocation. Inline
/// fields still use `C::View` directly, without allocating another wrapper.
pub struct ValueView<C: Intrlay, B> {
    buffer: B,
    offset: usize,
    codec: PhantomData<fn() -> C>,
}

impl<C: Intrlay, B: AsRef<[u8]>> ValueView<C, B> {
    /// Opens a value at an offset within caller-owned storage.
    ///
    /// # Safety
    /// The bytes must satisfy [`Intrlay::view`]'s representation and lifetime contract.
    #[inline]
    pub unsafe fn new_unchecked(buffer: B, offset: usize) -> Self {
        Self {
            buffer,
            offset,
            codec: PhantomData,
        }
    }

    /// Borrows the encoded value without rebuilding its owned object graph.
    #[must_use]
    #[inline]
    pub fn value(&self) -> C::View<'_> {
        // SAFETY: The constructor establishes the representation contract.
        unsafe { C::view(self.buffer.as_ref(), self.offset) }
    }

    /// Returns the storage supplied at construction.
    #[must_use]
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.buffer.as_ref()
    }

    /// Returns this value's position within its backing storage.
    #[must_use]
    #[inline]
    pub const fn byte_offset(&self) -> usize {
        self.offset
    }

    /// Reborrows the same valid encoding without copying its bytes.
    #[must_use]
    #[inline]
    pub fn as_view(&self) -> ValueView<C, &[u8]> {
        ValueView {
            buffer: self.buffer.as_ref(),
            offset: self.offset,
            codec: PhantomData,
        }
    }

    /// Consumes the view and returns ownership of its backing storage.
    #[must_use]
    #[inline]
    pub fn into_buffer(self) -> B {
        self.buffer
    }
}

impl<C: Intrlay, B: AsRef<[u8]> + AsMut<[u8]>> ValueView<C, B> {
    /// Replaces the complete inline encoding without changing allocation or offsets.
    /// Types with out-of-line payloads are rejected when this method is instantiated.
    /// Input data must not overlap the bytes being written, including nested inputs.
    ///
    /// # Errors
    /// Returns an error if a custom codec fails to encode its value.
    #[inline]
    pub fn replace(&mut self, value: &C::Value) -> Result<(), EncodeError> {
        const {
            assert!(
                !C::HAS_PAYLOAD,
                "in-place replacement requires a fixed encoding"
            );
        }
        let end = self.offset + C::LAYOUT.size();
        if C::IN_PLACE_WRITE {
            let mut writer =
                crate::Writer::new(&mut self.buffer.as_mut()[self.offset..end], C::LAYOUT)?;
            C::write(value, &mut writer, 0)?;
        } else {
            // Fallible custom accessors must not leave a live view half replaced.
            let encoded = crate::create::<C>(value)?;
            self.buffer.as_mut()[self.offset..end].copy_from_slice(encoded.as_ref());
        }
        Ok(())
    }

    /// Exclusively borrows the existing payload; its shape remains fixed.
    #[inline]
    pub fn value_mut(&mut self) -> C::ViewMut<'_> {
        // SAFETY: The constructor establishes validity and the mutable borrow is exclusive.
        unsafe { C::view_mut(self.buffer.as_mut(), self.offset) }
    }

    /// Exclusively reborrows the same valid encoding without copying its bytes.
    #[must_use]
    #[inline]
    pub fn as_view_mut(&mut self) -> ValueView<C, &mut [u8]> {
        ValueView {
            buffer: self.buffer.as_mut(),
            offset: self.offset,
            codec: PhantomData,
        }
    }

    /// Exposes backing bytes to generated, layout-preserving field mutators.
    ///
    /// # Safety
    /// The caller must preserve this codec's complete encoding, including its
    /// structural metadata. Only valid field mutations may be exposed to safe code.
    #[doc(hidden)]
    #[inline]
    pub unsafe fn as_bytes_mut_unchecked(&mut self) -> &mut [u8] {
        self.buffer.as_mut()
    }
}

impl<C: Intrlay> ValueView<C, OwnedBuffer> {
    /// Measures and allocates an encoded value.
    ///
    /// # Errors
    /// Returns an error for unsupported sizes or a layout that changes during construction.
    pub fn create(value: &C::Value) -> Result<Self, EncodeError> {
        let buffer = crate::create::<C>(value)?;
        // SAFETY: The codec initialized this allocation from a valid value.
        Ok(unsafe { Self::new_unchecked(buffer, 0) })
    }

    /// Constructs into externally allocated storage.
    ///
    /// # Errors
    /// Returns an error for insufficient/misaligned storage, unsupported sizes, or a
    /// layout that changes during construction. On failure, storage may be partially written.
    pub fn create_in_place<'a>(
        value: &C::Value,
        bytes: &'a mut [u8],
    ) -> Result<ValueView<C, &'a mut [u8]>, EncodeError> {
        let layout = crate::write_into::<C>(value, bytes)?;
        // SAFETY: write_into initialized this exact region.
        Ok(unsafe { ValueView::new_unchecked(&mut bytes[..layout.byte_length()], 0) })
    }
}
