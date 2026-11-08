use crate::{EncodeError, EncodeSource, Initialization, Intrlay, SizeCursor, Writer};

/// A repeatable sequence of element sources, without collecting them into a Vec.
///
/// `I` must yield the same logical elements each time it is cloned. Its items may
/// borrow existing storage or be small generated input values. Fixed encodings
/// are measured from the count alone; dynamic elements are traversed twice.
/// Cloning the iterator must not consume or mutate the original sequence.
pub struct SequenceInput<I> {
    iterator: I,
    count: usize,
}

impl<I: ExactSizeIterator + Clone> SequenceInput<I> {
    /// Retains an iterator and its exact count; no elements are copied or collected.
    #[must_use]
    pub fn new(iterator: I) -> Self {
        let count = iterator.len();
        Self { iterator, count }
    }
}

// SAFETY: Headers are reserved together before descendants. The write pass
// checks the actual element count: ExactSizeIterator is not an unsafe guarantee.
unsafe impl<C: Intrlay, I> EncodeSource<Vec<C>> for SequenceInput<I>
where
    I: ExactSizeIterator + Clone,
    I::Item: EncodeSource<C>,
{
    const INITIALIZATION: Initialization = if I::Item::WRITES_FIXED_LAYOUT {
        Initialization::payload(C::LAYOUT, Initialization::FIXED)
    } else {
        Initialization::UNKNOWN
    };

    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        cursor.reserve(C::LAYOUT, self.count)?;
        if !I::Item::WRITES_FIXED_LAYOUT {
            let mut actual = 0;
            for value in self.iterator.clone() {
                actual += 1;
                check_count(self.count, actual, false)?;
                value.measure(cursor)?;
            }
            check_count(self.count, actual, true)?;
        }
        Ok(())
    }

    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        let target = writer.reserve(C::LAYOUT, self.count)?;
        writer.reference(offset, target)?;
        writer.count(offset + 4, self.count)?;
        let mut actual = 0;
        for value in self.iterator.clone() {
            check_count(self.count, actual + 1, false)?;
            value.write(writer, target + actual * C::LAYOUT.size())?;
            actual += 1;
        }
        check_count(self.count, actual, true)
    }
}

fn check_count(expected: usize, actual: usize, finished: bool) -> Result<(), EncodeError> {
    if actual > expected || (finished && actual != expected) {
        Err(EncodeError::SequenceLengthChanged { expected, actual })
    } else {
        Ok(())
    }
}
