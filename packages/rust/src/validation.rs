//! One allocation-free pass establishes the contract required by borrowed views.

use crate::{DecodeError, Decoder, Intrlay, OwnedBuffer};

/// Storage whose encoded bytes stay stable while borrowed through `AsRef`.
///
/// # Safety
/// Repeated `AsRef` calls must expose the same contents and length until exclusive
/// mutation is possible. Interior mutation and independently writable aliases
/// must not change the bytes. If `AsMut<[u8]>` is implemented, it must refer to the
/// same bytes. Neither accessor may modify the contents as a side effect; changes
/// must occur through the returned exclusive slice. Moving the owner may move its
/// storage without changing its contents.
/// This guarantee lets checked construction validate once instead of every read.
pub unsafe trait StableBytes: AsRef<[u8]> {}

// SAFETY: A byte slice exposes its own contents and requires exclusive mutation.
unsafe impl StableBytes for [u8] {}
// SAFETY: Shared borrows prohibit mutation of stable storage.
unsafe impl<T: StableBytes + ?Sized> StableBytes for &T {}
// SAFETY: The mutable borrow exclusively owns the stable storage for its lifetime.
unsafe impl<T: StableBytes + ?Sized> StableBytes for &mut T {}
// SAFETY: These owners cannot mutate their bytes through a shared reference.
unsafe impl StableBytes for Vec<u8> {}
// SAFETY: Box owns its bytes and requires exclusive access for mutation.
unsafe impl StableBytes for Box<[u8]> {}
// SAFETY: Array contents can change only through exclusive access.
unsafe impl<const N: usize> StableBytes for [u8; N] {}
// SAFETY: OwnedBuffer exposes mutation only through an exclusive borrow.
unsafe impl StableBytes for OwnedBuffer {}
// SAFETY: Shared slice allocations are immutable; obtaining mutable access requires uniqueness.
unsafe impl StableBytes for std::sync::Arc<[u8]> {}
// SAFETY: Shared slice allocations are immutable; obtaining mutable access requires uniqueness.
unsafe impl StableBytes for std::rc::Rc<[u8]> {}

/// Validates headers, tags, strings, and disjoint payload reservations in wire order.
///
/// A monotonically increasing reservation boundary rejects overlapping references
/// without a set of visited regions. Gaps are permitted, including when a view
/// begins at a nested value within a larger packet. No payloads are copied.
pub struct Validator<'a> {
    reader: Decoder<'a>,
    reserved_end: usize,
    remaining_depth: usize,
}

impl<'a> Validator<'a> {
    /// Creates a validator with a maximum of 128 nested nontrivial values.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self::with_max_depth(bytes, 128)
    }

    /// Sets a limit on recursive validation, independent of array element count.
    #[must_use]
    pub const fn with_max_depth(bytes: &'a [u8], max_depth: usize) -> Self {
        Self {
            reader: Decoder::new(bytes),
            reserved_end: 0,
            remaining_depth: max_depth,
        }
    }

    /// Begins a new validation after reserving the entire root header.
    ///
    /// # Errors
    /// Returns an error for invalid regions, tags, UTF-8, or excessive nesting.
    pub fn validate<C: Intrlay + ?Sized>(&mut self, offset: usize) -> Result<(), DecodeError> {
        self.reader.range(offset, C::LAYOUT.size())?;
        self.reserved_end = offset + C::LAYOUT.size();
        self.field::<C>(offset)
    }

    /// Begins validation at a contiguous array of headers without a reference/count header.
    /// The complete array is reserved before any element's descendants are visited.
    ///
    /// # Errors
    /// Rejects overflowing extents, invalid elements, or overlapping descendants.
    pub fn validate_sequence<C: Intrlay>(
        &mut self,
        offset: usize,
        count: usize,
    ) -> Result<(), DecodeError> {
        let size = count
            .checked_mul(C::LAYOUT.size())
            .ok_or(DecodeError::SizeOverflow)?;
        self.reader.range(offset, size)?;
        self.reserved_end = offset + size;
        self.elements::<C>(offset, count)
    }

    /// Checks a child header already covered by its parent's reservation.
    ///
    /// # Errors
    /// Propagates malformed-data and nesting errors from the child codec.
    #[inline]
    pub fn field<C: Intrlay + ?Sized>(&mut self, offset: usize) -> Result<(), DecodeError> {
        self.reader.range(offset, C::LAYOUT.size())?;
        if C::ALL_BITS_VALID {
            return Ok(());
        }
        if self.remaining_depth == 0 {
            return Err(DecodeError::DepthLimit);
        }
        self.remaining_depth -= 1;
        let result = C::validate_fields(self, offset);
        self.remaining_depth += 1;
        result
    }

    /// Reads a checked scalar representation for a codec's tag or length.
    ///
    /// # Errors
    /// Returns an error when the requested scalar is outside the buffer.
    #[inline]
    pub fn read<const N: usize>(&self, offset: usize) -> Result<[u8; N], DecodeError> {
        self.reader.read(offset)
    }

    /// Borrows a checked region, for example to validate a string's UTF-8.
    ///
    /// # Errors
    /// Returns an error when the complete region is not present.
    #[inline]
    pub fn range(&self, offset: usize, size: usize) -> Result<&'a [u8], DecodeError> {
        self.reader.range(offset, size)
    }

    /// Reserves a referenced payload after all previously reserved regions.
    ///
    /// # Errors
    /// Rejects overflowing sizes, truncated payloads, and overlapping references.
    pub fn payload(
        &mut self,
        offset: usize,
        header_size: usize,
        count: usize,
        stride: usize,
    ) -> Result<usize, DecodeError> {
        let target = self.reader.reference(offset, header_size)?;
        let size = count.checked_mul(stride).ok_or(DecodeError::SizeOverflow)?;
        if target < self.reserved_end {
            return Err(DecodeError::InvalidReference { offset });
        }
        self.reader.range(target, size)?;
        self.reserved_end = target + size;
        Ok(target)
    }

    /// Validates array elements only when their bits carry tags or references.
    ///
    /// # Errors
    /// Rejects invalid element headers or descendants.
    pub fn elements<C: Intrlay>(&mut self, offset: usize, count: usize) -> Result<(), DecodeError> {
        self.reader.array_region(offset, count, C::LAYOUT.size())?;
        if !C::ALL_BITS_VALID {
            for index in 0..count {
                self.field::<C>(offset + index * C::LAYOUT.size())?;
            }
        }
        Ok(())
    }
}
