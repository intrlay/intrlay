use std::marker::PhantomData;

use crate::Intrlay;

/// Borrowed fixed-stride element headers, whose payloads may live elsewhere in the buffer.
pub struct SequenceView<'a, C: Intrlay> {
    bytes: &'a [u8],
    offset: usize,
    length: usize,
    codec: PhantomData<C>,
}

impl<C: Intrlay> Copy for SequenceView<'_, C> {}
impl<C: Intrlay> Clone for SequenceView<'_, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, C: Intrlay> SequenceView<'a, C> {
    /// Borrows a region of valid element headers without validating their payloads.
    ///
    /// # Safety
    /// All `length` elements at `offset`, including their referenced descendants,
    /// must be valid encodings of `C` within `bytes` for its entire lifetime.
    #[must_use]
    pub const unsafe fn new_unchecked(bytes: &'a [u8], offset: usize, length: usize) -> Self {
        Self {
            bytes,
            offset,
            length,
            codec: PhantomData,
        }
    }

    /// Returns the fixed element count.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.length
    }
    /// Returns whether there are no elements.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Returns one borrowed element, without allocating a native value.
    #[must_use]
    #[inline]
    pub fn get(&self, index: usize) -> Option<C::View<'a>> {
        if index >= self.length {
            return None;
        }
        // SAFETY: Construction guarantees every header and its descendants are valid.
        Some(unsafe { C::view(self.bytes, self.offset + index * C::LAYOUT.size()) })
    }

    /// Iterates borrowed elements in source order.
    #[must_use]
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = C::View<'a>> + 'a {
        let bytes = self.bytes;
        let offset = self.offset;
        (0..self.length).map(move |index| {
            // SAFETY: Every index belongs to the region guaranteed by construction.
            unsafe { C::view(bytes, offset + index * C::LAYOUT.size()) }
        })
    }
}

/// An exclusive fixed-length sequence view; element mutation cannot resize it.
pub struct SequenceMut<'a, C: Intrlay> {
    bytes: &'a mut [u8],
    offset: usize,
    length: usize,
    codec: PhantomData<C>,
}

impl<'a, C: Intrlay> SequenceMut<'a, C> {
    /// Exclusively borrows a valid sequence without inspecting its elements.
    ///
    /// # Safety
    /// The region must meet [`SequenceView::new_unchecked`]'s validity requirements.
    /// Storage must be exclusively borrowed for the view's lifetime.
    #[must_use]
    pub const unsafe fn new_unchecked(bytes: &'a mut [u8], offset: usize, length: usize) -> Self {
        Self {
            bytes,
            offset,
            length,
            codec: PhantomData,
        }
    }

    /// Returns the fixed element count.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.length
    }
    /// Returns whether there are no elements.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Temporarily borrows the sequence for reading.
    #[must_use]
    #[inline]
    pub fn as_view(&self) -> SequenceView<'_, C> {
        // SAFETY: The shared reborrow retains all construction invariants.
        unsafe { SequenceView::new_unchecked(self.bytes, self.offset, self.length) }
    }

    /// Temporarily borrows one element for mutation, excluding other mutable aliases.
    #[must_use]
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<C::ViewMut<'_>> {
        if index >= self.length {
            return None;
        }
        // SAFETY: The reborrow is exclusive and the element belongs to the valid region.
        Some(unsafe { C::view_mut(self.bytes, self.offset + index * C::LAYOUT.size()) })
    }

    /// Replaces one fixed-size element, preserving the sequence's length.
    ///
    /// # Panics
    /// Panics if `index` is outside the sequence.
    ///
    /// # Errors
    /// Returns an error if a custom codec cannot encode the replacement.
    #[inline]
    pub fn set(&mut self, index: usize, value: &C::Value) -> Result<(), crate::EncodeError> {
        assert!(index < self.length, "sequence index out of bounds");
        // SAFETY: The checked index selects a valid exclusively borrowed element.
        unsafe {
            crate::ValueView::<C, _>::new_unchecked(
                &mut *self.bytes,
                self.offset + index * C::LAYOUT.size(),
            )
        }
        .replace(value)
    }
}
