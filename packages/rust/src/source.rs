//! Encoding inputs are independent of the schema's owned decode type.

mod builtins;
mod sequence;
mod zeroed;

pub use sequence::SequenceInput;
pub use zeroed::{Zeroed, ZeroedSequence};

use std::marker::PhantomData;

use crate::{EncodeError, Initialization, Intrlay, SizeCursor, Writer};

/// A possibly borrowed input for the schema codec `C`.
///
/// Implemented by derived `FooInput` types, owned values, slices, strings, and
/// recursively composed inputs. `C` determines the wire layout and the decoded
/// type; this trait only supplies values for construction. Inputs must describe
/// the same logical value during measurement and writing. Borrowed input storage
/// must not overlap the destination.
///
/// Built-in scalar sources require the schema's exact type, or a reference to it.
/// Containers apply this rule recursively to their elements. Numeric conversion
/// must be explicit, for example in an iterator supplied to [`SequenceInput`].
///
/// # Safety
/// Successful writes must produce a valid `C` encoding, with the same reservation
/// order as measurement and all meaningful bytes initialized. `INITIALIZATION`
/// must conservatively describe coverage, including padding. `NATIVE_LAYOUT`
/// requires every byte of `Self` to be initialized and identical to `C`'s fixed
/// encoding, with no ownership, padding, or out-of-line payloads.
pub unsafe trait EncodeSource<C: Intrlay> {
    /// Whether this input's own representation can be copied as encoded bytes.
    const NATIVE_LAYOUT: bool = false;
    /// Whether writes cover every byte of a fixed encoding, without payloads.
    const WRITES_FIXED_LAYOUT: bool = false;
    /// Coverage proof for this input, independent of the owned source's layout.
    const INITIALIZATION: Initialization = Initialization::UNKNOWN;
    /// Whether successful construction initializes every measured byte.
    const WRITES_ALL_BYTES: bool = Self::INITIALIZATION.covers(C::LAYOUT);

    /// Reserves descendants after the caller has reserved `C::LAYOUT`.
    ///
    /// # Errors
    /// Returns an error for unsupported sizes or inconsistent sequence lengths.
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError>;

    /// Writes the reserved header and appends its descendants.
    ///
    /// # Errors
    /// Returns an error if the destination is insufficient or the input changes.
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError>;
}

/// Uses a codec's existing owned-value implementation as an encoding input.
///
/// This adapter also supports custom codecs and remote types without requiring
/// another implementation. It borrows the value and allocates nothing.
pub struct ValueSource<'a, C: Intrlay> {
    value: &'a C::Value,
    codec: PhantomData<fn() -> C>,
}

impl<'a, C: Intrlay> ValueSource<'a, C> {
    /// Borrows a value using `C`'s existing measure and write implementation.
    #[must_use]
    pub const fn new(value: &'a C::Value) -> Self {
        Self {
            value,
            codec: PhantomData,
        }
    }
}

// SAFETY: Delegates every reservation, write, and coverage proof to C.
unsafe impl<C: Intrlay> EncodeSource<C> for ValueSource<'_, C> {
    const WRITES_FIXED_LAYOUT: bool = C::WRITES_FIXED_LAYOUT;
    const INITIALIZATION: Initialization = C::INITIALIZATION;
    const WRITES_ALL_BYTES: bool = C::WRITES_ALL_BYTES;
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        C::measure(self.value, cursor)
    }
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        C::write(self.value, writer, offset)
    }
}

// SAFETY: A reference delegates encoding, but is never itself wire-compatible.
unsafe impl<C: Intrlay, S: EncodeSource<C> + ?Sized> EncodeSource<C> for &S {
    const WRITES_FIXED_LAYOUT: bool = S::WRITES_FIXED_LAYOUT;
    const INITIALIZATION: Initialization = S::INITIALIZATION;
    const WRITES_ALL_BYTES: bool = S::WRITES_ALL_BYTES;
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        S::measure(self, cursor)
    }
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        S::write(self, writer, offset)
    }
}

/// Supplies an indirect wire value without allocating a native `Box`.
///
/// `S` can be an owned value, a borrowed value, or a generated input. Only the
/// wire encoder allocates the pointee's region in the final buffer.
pub struct BoxInput<S>(pub S);

// SAFETY: Reserves the pointee's header before its descendants and delegates to S.
unsafe impl<C: Intrlay, S: EncodeSource<C>> EncodeSource<Box<C>> for BoxInput<S> {
    const INITIALIZATION: Initialization = if S::WRITES_FIXED_LAYOUT {
        Initialization::payload(C::LAYOUT, Initialization::FIXED)
    } else {
        Initialization::UNKNOWN
    };
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        cursor.reserve(C::LAYOUT, 1)?;
        self.0.measure(cursor)
    }
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        let target = writer.reserve(C::LAYOUT, 1)?;
        writer.reference(offset, target)?;
        self.0.write(writer, target)
    }
}
