//! Checked reconstruction visits each value once, sharing a recursion budget.

use std::{error::Error, fmt, mem::MaybeUninit};

use crate::Intrlay;

/// A malformed encoding or a reconstruction limit encountered by checked decoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// A header or payload is not wholly contained in the input buffer.
    OutOfBounds {
        /// Start of the requested region.
        offset: usize,
        /// Required region size in bytes.
        size: usize,
    },
    /// An encoded count or offset cannot be represented by the address calculation.
    SizeOverflow,
    /// A fixed-width scalar cannot be represented by its source-language native type.
    ValueOutOfRange {
        /// Address of the scalar value.
        offset: usize,
    },
    /// A relative reference points inside a header or overlaps a reserved region.
    InvalidReference {
        /// Address of the relative-reference header.
        offset: usize,
    },
    /// A boolean, option, or enum contains an unknown tag.
    InvalidTag {
        /// Address of the discriminant.
        offset: usize,
        /// Discriminant found in the buffer.
        tag: u32,
    },
    /// A string's payload is not valid UTF-8.
    InvalidUtf8 {
        /// Start of the string payload.
        offset: usize,
        /// The standard UTF-8 validation failure.
        source: std::str::Utf8Error,
    },
    /// The configured limit for nested values was reached.
    DepthLimit,
    /// The decoded collection's storage could not be reserved.
    AllocationFailed,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds { offset, size } => {
                write!(output, "encoded region at {offset} requires {size} bytes")
            }
            Self::SizeOverflow => output.write_str("encoded offset or count overflows"),
            Self::ValueOutOfRange { offset } => write!(
                output,
                "encoded value at {offset} is outside the native range"
            ),
            Self::InvalidReference { offset } => {
                write!(output, "invalid forward reference at {offset}")
            }
            Self::InvalidTag { offset, tag } => write!(output, "invalid tag {tag} at {offset}"),
            Self::InvalidUtf8 { offset, source } => {
                write!(output, "invalid UTF-8 at {offset}: {source}")
            }
            Self::DepthLimit => output.write_str("decoding depth limit exceeded"),
            Self::AllocationFailed => {
                output.write_str("could not reserve decoded collection storage")
            }
        }
    }
}

impl Error for DecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidUtf8 { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Shared input and depth limit for checked codecs, including user-defined adapters.
///
/// Codecs use [`range`](Self::range) before addressing a header and
/// [`decode`](Self::decode) for children. The latter preserves the depth limit
/// across records and independently allocated descendants without a validation pass.
pub struct Decoder<'a> {
    bytes: &'a [u8],
    remaining_depth: usize,
}

impl<'a> Decoder<'a> {
    /// Opens arbitrary bytes with a maximum of 128 nested codec calls.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self::with_max_depth(bytes, 128)
    }

    /// Opens bytes with an application-selected nesting limit.
    #[must_use]
    pub const fn with_max_depth(bytes: &'a [u8], max_depth: usize) -> Self {
        Self {
            bytes,
            remaining_depth: max_depth,
        }
    }

    /// Reconstructs a value while bounding recursive traversal.
    ///
    /// # Errors
    /// Returns the codec's validation error or [`DecodeError::DepthLimit`].
    #[inline]
    pub fn decode<C: Intrlay + ?Sized>(&mut self, offset: usize) -> Result<C::Value, DecodeError> {
        if self.remaining_depth == 0 {
            return Err(DecodeError::DepthLimit);
        }
        if C::NATIVE_LAYOUT && C::ALL_BITS_VALID {
            let bytes = self.range(offset, C::LAYOUT.size())?;
            // SAFETY: The complete region is initialized and in bounds. These
            // independent proofs establish both native representation and value
            // validity; NATIVE_LAYOUT alone would also include bool and enums.
            // read_unaligned permits foreign buffers with arbitrary byte alignment.
            return Ok(unsafe { bytes.as_ptr().cast::<C::Value>().read_unaligned() });
        }
        if let Some(depth) = C::FIXED_DECODE_DEPTH
            && self.remaining_depth >= depth
        {
            self.range(offset, C::LAYOUT.size())?;
            // SAFETY: The fixed-header proof establishes validity and sufficient
            // depth without descendants or additional checked-decoder failures.
            // Conversion still uses the codec, never a mismatched native cast.
            return Ok(unsafe { C::decode_unchecked(self.bytes, offset) });
        }
        // Retain the ordinary traversal when depth is insufficient or unknown,
        // including its error ordering and custom adapter behavior.
        self.remaining_depth -= 1;
        let result = C::decode_from(self, offset);
        self.remaining_depth += 1;
        result
    }

    /// Borrows a complete in-bounds region without copying it.
    ///
    /// # Errors
    /// Returns [`DecodeError::OutOfBounds`] for a missing or overflowing region.
    #[inline]
    pub fn range(&self, offset: usize, size: usize) -> Result<&'a [u8], DecodeError> {
        let end = offset
            .checked_add(size)
            .ok_or(DecodeError::OutOfBounds { offset, size })?;
        self.bytes
            .get(offset..end)
            .ok_or(DecodeError::OutOfBounds { offset, size })
    }

    /// Copies a fixed number of validated bytes for scalar conversion.
    ///
    /// # Errors
    /// Returns [`DecodeError::OutOfBounds`] when the scalar is truncated.
    #[inline]
    pub fn read<const N: usize>(&self, offset: usize) -> Result<[u8; N], DecodeError> {
        let mut value = [0; N];
        value.copy_from_slice(self.range(offset, N)?);
        Ok(value)
    }

    /// Resolves a forward reference after validating its complete header.
    ///
    /// # Errors
    /// Returns an error for a truncated header, overlapping reference, or overflow.
    pub fn reference(&self, offset: usize, header_size: usize) -> Result<usize, DecodeError> {
        self.range(offset, header_size)?;
        let relative = u32::from_le_bytes(self.read(offset)?) as usize;
        if relative < header_size {
            return Err(DecodeError::InvalidReference { offset });
        }
        offset
            .checked_add(relative)
            .ok_or(DecodeError::SizeOverflow)
    }

    /// Borrows a checked UTF-8 string using the standard reference/count header.
    ///
    /// # Errors
    /// Returns an error for an invalid reference, truncated payload, or invalid UTF-8.
    pub fn string(&self, offset: usize) -> Result<&'a str, DecodeError> {
        let start = self.reference(offset, crate::layout::Layout::SEQUENCE.size())?;
        let length = u32::from_le_bytes(self.read(offset + 4)?) as usize;
        std::str::from_utf8(self.range(start, length)?).map_err(|source| DecodeError::InvalidUtf8 {
            offset: start,
            source,
        })
    }

    /// Validates the entire fixed-stride header region before iterating elements.
    ///
    /// # Errors
    /// Returns an error for a count overflow or a truncated region.
    pub fn array_region(
        &self,
        offset: usize,
        count: usize,
        stride: usize,
    ) -> Result<(), DecodeError> {
        let size = count.checked_mul(stride).ok_or(DecodeError::SizeOverflow)?;
        self.range(offset, size)?;
        Ok(())
    }

    /// Initializes fixed elements after validating their entire header region.
    /// A false result leaves the destination untouched for checked traversal.
    #[inline]
    pub(crate) fn decode_fixed_elements<C: Intrlay>(
        &self,
        offset: usize,
        destination: &mut [MaybeUninit<C::Value>],
    ) -> Result<bool, DecodeError> {
        if C::NATIVE_LAYOUT && C::ALL_BITS_VALID {
            self.range(offset, size_of_val(destination))?;
            // SAFETY: Both proofs establish identical size/stride and validity for
            // every initialized byte pattern, with no native ownership. The exclusive
            // destination holds exactly this many bytes and cannot alias the input.
            // Copying bytes also permits unaligned foreign storage and zero-sized values.
            unsafe { copy_native_elements_unchecked::<C>(self.bytes, offset, destination) };
            return Ok(true);
        }
        if let Some(depth) = C::FIXED_DECODE_DEPTH
            && self.remaining_depth >= depth
        {
            self.array_region(offset, destination.len(), C::LAYOUT.size())?;
            let mut prefix = DecodedPrefix {
                values: destination,
                initialized: 0,
            };
            for index in 0..prefix.values.len() {
                // SAFETY: The complete fixed-stride region is in bounds. Each
                // element's proof establishes validity and sufficient decode depth.
                // Keep a drop guard because an adapter's constructor may unwind.
                prefix.values[index].write(unsafe {
                    C::decode_unchecked(self.bytes, offset + index * C::LAYOUT.size())
                });
                prefix.initialized += 1;
            }
            prefix.initialized = 0;
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn fixed_array<C: Intrlay, const N: usize>(
        &mut self,
        offset: usize,
    ) -> Result<[C::Value; N], DecodeError> {
        self.array_region(offset, N, C::LAYOUT.size())?;
        let mut array = PartialArray {
            values: [const { MaybeUninit::uninit() }; N],
            initialized: 0,
        };
        for index in 0..N {
            array.values[index].write(self.decode::<C>(offset + index * C::LAYOUT.size())?);
            array.initialized += 1;
        }
        // SAFETY: Every element has been initialized. Reading transfers ownership;
        // clearing the prefix prevents PartialArray from dropping it a second time.
        let result = unsafe { array.values.as_ptr().cast::<[C::Value; N]>().read() };
        array.initialized = 0;
        Ok(result)
    }
}

/// Copies validated native elements into exclusively borrowed uninitialized slots.
///
/// # Safety
/// `C::NATIVE_LAYOUT` must hold. Starting at `offset`, `bytes` must contain valid
/// encodings of all destination elements. This includes valid native values,
/// not merely in-bounds bytes: booleans and enums still require valid tags.
#[inline]
pub(crate) unsafe fn copy_native_elements_unchecked<C: Intrlay>(
    bytes: &[u8],
    offset: usize,
    destination: &mut [MaybeUninit<C::Value>],
) {
    // SAFETY: The caller proves native representation and validity for the full
    // source region. Exclusive destination slots provide disjoint, aligned storage;
    // a byte copy accepts an unaligned source and also preserves zero-sized values.
    unsafe {
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr().add(offset),
            destination.as_mut_ptr().cast::<u8>(),
            size_of_val(destination),
        );
    }
}

/// Combines fixed fields' known decode depths, including their enclosing codec.
/// Unknown fields or arithmetic overflow conservatively retain checked traversal.
#[must_use]
pub const fn fixed_decode_depth(fields: &[Option<usize>]) -> Option<usize> {
    let mut depth = 0;
    let mut index = 0;
    while index < fields.len() {
        let Some(field_depth) = fields[index] else {
            return None;
        };
        if field_depth > depth {
            depth = field_depth;
        }
        index += 1;
    }
    depth.checked_add(1)
}

struct DecodedPrefix<'a, T> {
    values: &'a mut [MaybeUninit<T>],
    initialized: usize,
}

impl<T> Drop for DecodedPrefix<'_, T> {
    fn drop(&mut self) {
        for value in &mut self.values[..self.initialized] {
            // SAFETY: The prefix counts exactly the elements successfully decoded.
            unsafe { value.assume_init_drop() };
        }
    }
}

struct PartialArray<T, const N: usize> {
    values: [MaybeUninit<T>; N],
    initialized: usize,
}

impl<T, const N: usize> Drop for PartialArray<T, N> {
    fn drop(&mut self) {
        for value in &mut self.values[..self.initialized] {
            // SAFETY: The prefix counts exactly the elements successfully decoded.
            unsafe { value.assume_init_drop() };
        }
    }
}
