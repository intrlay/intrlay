use intrlay_layout::Layout;
use intrlay_schema::Describe;

use crate::{
    BufferLayout, EncodeError, Initialization, OwnedBuffer, SizeCursor, Writer,
    buffer::BufferAllocation,
};

/// A concrete value's encoded layout, construction, and borrowed access.
///
/// `Self` identifies the codec; [`Value`](Self::Value) identifies its source type.
/// They are identical for ordinary derives and differ for remote adapters.
///
/// # Safety
/// Implementations must use the same reservation rules when measuring and writing,
/// initialize every meaningful byte, and produce representations accepted by their view/decode
/// methods. Mutable views may replace fixed inline values, including their tags,
/// but may not invalidate relative references or create overlapping mutable
/// references. All returned borrows must remain within the
/// supplied storage and retain its lifetime.
///
/// Initialization metadata must conservatively describe all successful writes.
/// If [`WRITES_ALL_BYTES`](Self::WRITES_ALL_BYTES) is true, `write` must initialize
/// every byte of the root header and every region reserved while writing,
/// including padding and inactive data. Construction may supply uninitialized
/// storage based on this guarantee and checks the final layout before publishing
/// it, since source getters may change their payload sizes between passes.
pub unsafe trait Intrlay: Describe + 'static {
    /// Rich source-language value converted by this codec.
    type Value;
    /// Read-only projection of encoded storage.
    type View<'a>
    where
        Self: 'a;
    /// Mutable projection whose operations preserve the established layout.
    type ViewMut<'a>
    where
        Self: 'a;
    /// Fixed header size and alignment, independent of variable payload lengths.
    const LAYOUT: Layout;
    /// Whether the source value is byte-identical to its fixed encoding on this target.
    ///
    /// Implementations may set this only when every byte is initialized, no native
    /// ownership is embedded, and the native size, field offsets, and byte order match.
    /// Derives use this for a whole-record copy without exposing a native-reference API.
    const NATIVE_LAYOUT: bool = false;
    /// Whether any value can require storage outside its fixed header.
    /// False permits replacing presence bits and enum tags in existing storage.
    /// A false value guarantees that measurement and writing reserve no payloads.
    const HAS_PAYLOAD: bool = !Self::WRITES_FIXED_LAYOUT;
    /// Whether a fixed-value write is guaranteed to complete without error or unwinding.
    ///
    /// True guarantees completion for a valid value and a sufficiently large
    /// destination. A false value stages replacement before copying; custom
    /// codecs should keep the default unless their writer satisfies this guarantee.
    const IN_PLACE_WRITE: bool = Self::NATIVE_LAYOUT;
    /// Whether this codec writes a complete fixed-size encoding without payloads.
    ///
    /// True guarantees that `measure` makes no reservations and `write` covers
    /// every header byte. Dynamic containers use this terminating proof instead
    /// of recursively evaluating their element types' payload metadata.
    const WRITES_FIXED_LAYOUT: bool = false;
    /// Evidence for header coverage and alignment between payload reservations.
    const INITIALIZATION: Initialization = Initialization::UNKNOWN;
    /// Whether every measured byte is written without clearing the destination.
    ///
    /// This is independent of `NATIVE_LAYOUT`: a non-native record can still
    /// initialize all its encoded bytes by writing its fields individually.
    const WRITES_ALL_BYTES: bool = Self::INITIALIZATION.covers(Self::LAYOUT);

    /// Measures descendants after the caller has reserved this value's header.
    ///
    /// # Errors
    /// Returns an error if the value exceeds the supported buffer representation.
    fn measure(value: &Self::Value, cursor: &mut SizeCursor) -> Result<(), EncodeError>;

    /// Initializes a reserved header and appends its descendants.
    ///
    /// # Errors
    /// Returns an error if the destination or encoded size is insufficient.
    fn write(
        value: &Self::Value,
        writer: &mut Writer<'_>,
        offset: usize,
    ) -> Result<(), EncodeError>;

    /// Borrows a value from trusted encoded bytes.
    ///
    /// # Safety
    /// `offset` must identify a complete, valid encoding of this type within `bytes`.
    /// All referenced descendants must remain valid and immutable for the borrow.
    unsafe fn view(bytes: &[u8], offset: usize) -> Self::View<'_>;

    /// Exclusively borrows a value from trusted encoded bytes.
    ///
    /// # Safety
    /// The input must meet [`view`](Self::view)'s representation requirements and
    /// be exclusively borrowed for the returned view's lifetime.
    unsafe fn view_mut(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_>;

    /// Materializes an owned source-language value.
    ///
    /// # Safety
    /// The input must meet [`view`](Self::view)'s representation requirements.
    unsafe fn decode(bytes: &[u8], offset: usize) -> Self::Value;
}

/// Computes complete allocation requirements without allocating payload storage.
///
/// # Errors
/// Returns an error if the value exceeds the supported size or count limits.
pub fn compute_layout<C: Intrlay>(value: &C::Value) -> Result<BufferLayout, EncodeError> {
    let mut cursor = SizeCursor::new(C::LAYOUT)?;
    C::measure(value, &mut cursor)?;
    Ok(cursor.finish())
}

/// Measures and constructs one initialized owned buffer.
///
/// # Errors
/// Returns an error for unsupported sizes or a layout that changes during construction.
pub fn create<C: Intrlay>(value: &C::Value) -> Result<OwnedBuffer, EncodeError> {
    let layout = compute_layout::<C>(value)?;
    let mut buffer = if C::WRITES_ALL_BYTES {
        BufferAllocation::new::<false>(layout)?
    } else {
        BufferAllocation::new::<true>(layout)?
    };
    let mut writer = Writer::new_uninit(buffer.as_uninit_mut(), C::LAYOUT)?;
    C::write(value, &mut writer, 0)?;
    writer.finish(layout)?;
    // SAFETY: Either the allocation was zeroed, or the codec's byte-coverage
    // proof and matching final layout guarantee that every byte was initialized.
    Ok(unsafe { buffer.assume_init() })
}

/// Initializes externally owned storage, retaining all ownership in the caller.
///
/// Padding and inactive payload bytes are initialized before fields are written.
/// Bytes beyond the complete encoded value are not modified.
/// Input data, including nested references, must not overlap the bytes being
/// written. Safe borrows enforce this; unsafe and FFI callers must preserve it.
///
/// ```compile_fail
/// let mut bytes = [1_u8, 2, 3, 4];
/// let input = &bytes;
/// intrlay::write_into::<[u8; 4]>(input, &mut bytes).unwrap();
/// ```
///
/// # Errors
/// Returns an error for insufficient/misaligned storage, unsupported sizes, or a
/// layout that changes during construction. On failure, storage may be partially written.
pub fn write_into<C: Intrlay>(
    value: &C::Value,
    bytes: &mut [u8],
) -> Result<BufferLayout, EncodeError> {
    let layout = compute_layout::<C>(value)?;
    if bytes.len() < layout.byte_length() {
        return Err(EncodeError::BufferTooSmall {
            required: layout.byte_length(),
            available: bytes.len(),
        });
    }
    if !(bytes.as_ptr() as usize).is_multiple_of(layout.alignment()) {
        return Err(EncodeError::Misaligned {
            required: layout.alignment(),
        });
    }
    let bytes = &mut bytes[..layout.byte_length()];
    if !C::WRITES_ALL_BYTES {
        bytes.fill(0);
    }
    let mut writer = Writer::new(bytes, C::LAYOUT)?;
    C::write(value, &mut writer, 0)?;
    writer.finish(layout)?;
    Ok(layout)
}

/// Constructs into external storage and returns its mutable typed projection.
///
/// # Errors
/// Returns an error for insufficient/misaligned storage or unsupported sizes.
pub fn create_in_place<'a, C: Intrlay>(
    value: &C::Value,
    bytes: &'a mut [u8],
) -> Result<C::ViewMut<'a>, EncodeError> {
    let layout = write_into::<C>(value, bytes)?;
    // SAFETY: The codec has just initialized this exact region from a valid source value.
    Ok(unsafe { C::view_mut(&mut bytes[..layout.byte_length()], 0) })
}
