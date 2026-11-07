//! Typed, borrowed access to Intrlay buffers without rebuilding an object graph.
//!
//! Construction measures the complete value first and initializes one fixed buffer.
//! Strings, lengths, and relative references are fixed after creation. Complete
//! inline values can be replaced, including fixed enum/option payloads.
//!
//! Construction and mutation inputs must not reference bytes being written,
//! including through nested inputs. Safe borrows enforce this; unsafe and FFI
//! callers must preserve it. No runtime overlap check is performed.
//!
//! Foreign buffers enter through an explicit unsafe contract. No recursive
//! validation, transport, allocator negotiation, or runtime schema lookup occurs
//! when a view is opened.

#[allow(
    unused_extern_crates,
    reason = "derive expansions use the same path inside the runtime crate"
)]
extern crate self as intrlay;

mod buffer;
mod builtins;
mod codec;
mod exports;
mod initialization;
mod sequence;
mod value_view;

pub use buffer::{BufferLayout, EncodeError, OwnedBuffer, SizeCursor, Writer};
pub use builtins::{Scalar, ScalarMut};
pub use codec::{Intrlay, compute_layout, create, create_in_place, write_into};
pub use initialization::Initialization;
pub use intrlay_layout as layout;
pub use intrlay_schema as schema;
pub use sequence::{SequenceMut, SequenceView};
pub use value_view::ValueView;

#[cfg(feature = "derive")]
pub use intrlay_derive::Intrlay;

/// Applies an element adapter to every element of a vector without wrapping values.
///
/// Used as a codec type, for example `Map<Vec3Def>` for `Vec<glam::Vec3>`.
pub type Map<C> = Vec<C>;

/// Support used by generated code; not a native-reference access API.
#[doc(hidden)]
pub mod __private {
    /// Copies an initialized, byte-identical native value into its encoded header.
    ///
    /// # Safety
    /// `T` must satisfy the complete `Intrlay::NATIVE_LAYOUT` contract.
    pub unsafe fn write_native<T>(value: &T, writer: &mut crate::Writer<'_>, offset: usize) {
        // SAFETY: The caller guarantees every native byte is initialized and matches the wire layout.
        let bytes = unsafe {
            std::slice::from_raw_parts(std::ptr::from_ref(value).cast::<u8>(), size_of::<T>())
        };
        writer.write(offset, bytes);
    }

    /// Materializes a byte-identical native value without requiring aligned input.
    ///
    /// # Safety
    /// `T` must satisfy `Intrlay::NATIVE_LAYOUT`, and the region must encode a valid T.
    #[must_use]
    pub unsafe fn read_native<T>(bytes: &[u8], offset: usize) -> T {
        // SAFETY: The caller guarantees the complete initialized region and native type validity.
        unsafe { bytes.as_ptr().add(offset).cast::<T>().read_unaligned() }
    }
}
