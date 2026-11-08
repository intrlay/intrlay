use crate::{EncodeError, EncodeSource, Initialization, Intrlay, SizeCursor, Writer};

/// Zero-initializes a fixed array using its schema length, without element storage.
/// The element codec must prove that its all-zero wire representation is valid.
///
/// ```compile_fail
/// // Strings need valid payload references, so their headers cannot be zeroed.
/// let _ = intrlay::create_from::<[String; 2]>(&intrlay::Zeroed);
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct Zeroed;

/// Zero-initializes a sequence directly in the final buffer.
///
/// ```compile_fail
/// // A sequence of sequences needs an explicit input for each inner sequence.
/// let _ = intrlay::create_from::<Vec<Vec<f32>>>(&intrlay::ZeroedSequence::new(2));
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ZeroedSequence {
    length: usize,
}

impl ZeroedSequence {
    /// Requests `length` elements without allocating an intermediate collection.
    #[must_use]
    pub const fn new(length: usize) -> Self {
        Self { length }
    }
}

// SAFETY: The compile-time assertion proves that zeroing every header byte is a
// complete valid encoding. No native values or references are manufactured.
unsafe impl<C: Intrlay, const N: usize> EncodeSource<[C; N]> for Zeroed {
    const WRITES_FIXED_LAYOUT: bool = true;
    const INITIALIZATION: Initialization = Initialization::FIXED;

    fn measure(&self, _: &mut SizeCursor) -> Result<(), EncodeError> {
        const {
            assert!(
                N == 0 || C::ZEROABLE,
                "array elements do not support zero initialization"
            );
        };
        Ok(())
    }

    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        const {
            assert!(
                N == 0 || C::ZEROABLE,
                "array elements do not support zero initialization"
            );
        };
        writer.zero(offset, <[C; N]>::LAYOUT.size());
        Ok(())
    }
}

// SAFETY: Measurement and writing reserve the same region. The compile-time
// assertion proves each zeroed element is valid; the sequence header is written.
unsafe impl<C: Intrlay> EncodeSource<Vec<C>> for ZeroedSequence {
    const INITIALIZATION: Initialization =
        Initialization::payload(C::LAYOUT, Initialization::FIXED);

    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        const {
            assert!(
                C::ZEROABLE,
                "sequence elements do not support zero initialization"
            );
        };
        cursor.reserve(C::LAYOUT, self.length)?;
        Ok(())
    }

    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        const {
            assert!(
                C::ZEROABLE,
                "sequence elements do not support zero initialization"
            );
        };
        let target = writer.reserve(C::LAYOUT, self.length)?;
        writer.reference(offset, target)?;
        writer.count(offset + 4, self.length)?;
        writer.zero(target, C::LAYOUT.size() * self.length);
        Ok(())
    }
}
