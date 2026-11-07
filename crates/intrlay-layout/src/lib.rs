//! Target-independent layout rules shared by derives, source frontends, and emitters.
//!
//! Fields retain declaration order. Each field starts at its own alignment, and
//! a record's size includes trailing padding to its maximum field alignment.
//! Relative references use unsigned byte offsets from their own header.

#![no_std]
#![forbid(unsafe_code)]

/// Size and alignment of the fixed portion of an encoded value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    size: usize,
    alignment: usize,
}

impl Layout {
    /// A zero-sized value with no alignment requirement.
    pub const UNIT: Self = Self::new(0, 1);
    /// A relative byte offset and a count, both represented by `u32`.
    pub const SEQUENCE: Self = Self::new(8, 4);
    /// A boxed value's relative `u32` byte offset.
    pub const BOX: Self = Self::new(4, 4);

    /// Creates a layout whose size includes its trailing padding.
    ///
    /// # Panics
    /// Panics if alignment is not a power of two or size is not aligned.
    #[must_use]
    pub const fn new(size: usize, alignment: usize) -> Self {
        assert!(
            alignment.is_power_of_two(),
            "alignment must be a power of two"
        );
        assert!(
            size.is_multiple_of(alignment),
            "size must include trailing padding"
        );
        Self { size, alignment }
    }

    /// Returns the size of the fixed header, including trailing padding.
    #[must_use]
    pub const fn size(self) -> usize {
        self.size
    }

    /// Returns the alignment required by the fixed header.
    #[must_use]
    pub const fn alignment(self) -> usize {
        self.alignment
    }

    /// Returns the layout of an inline fixed-size array.
    ///
    /// # Panics
    /// Panics if the total size exceeds the address space.
    #[must_use]
    pub const fn array(self, count: usize) -> Self {
        let Some(size) = self.size.checked_mul(count) else {
            panic!("array layout exceeds address space");
        };
        Self::new(size, self.alignment)
    }

    /// Places an inline payload after a tag, with padding before and after it.
    #[must_use]
    pub const fn tagged(tag: Self, payload: Self) -> Self {
        record(&[tag, payload], 1)
    }
}

/// Rounds a byte position up to a power-of-two alignment.
///
/// # Panics
/// Panics if alignment is invalid or the rounded position overflows.
#[must_use]
pub const fn align_up(position: usize, alignment: usize) -> usize {
    assert!(
        alignment.is_power_of_two(),
        "alignment must be a power of two"
    );
    let Some(value) = position.checked_add(alignment - 1) else {
        panic!("aligned position exceeds address space");
    };
    value & !(alignment - 1)
}

/// Computes a record's layout in declaration order.
///
/// `minimum_alignment` supports explicit alignment such as a padded SIMD record.
/// It does not reorder fields or apply the source compiler's ABI rules.
///
/// # Panics
/// Panics on an invalid alignment or a layout larger than the address space.
#[must_use]
pub const fn record(fields: &[Layout], minimum_alignment: usize) -> Layout {
    assert!(
        minimum_alignment.is_power_of_two(),
        "invalid record alignment"
    );
    let mut size = 0usize;
    let mut alignment = minimum_alignment;
    let mut i = 0;
    while i < fields.len() {
        let field = fields[i];
        if field.alignment > alignment {
            alignment = field.alignment;
        }
        let Some(end) = align_up(size, field.alignment).checked_add(field.size) else {
            panic!("record layout exceeds address space");
        };
        size = end;
        i += 1;
    }
    Layout::new(align_up(size, alignment), alignment)
}

/// Computes one field's byte offset in a declaration-ordered record.
///
/// # Panics
/// Panics if `index` is not a field or the preceding layout overflows.
#[must_use]
pub const fn field_offset(fields: &[Layout], index: usize) -> usize {
    assert!(index < fields.len(), "field index out of range");
    let mut offset = 0usize;
    let mut i = 0;
    while i < index {
        let Some(end) = align_up(offset, fields[i].alignment).checked_add(fields[i].size) else {
            panic!("field offset exceeds address space");
        };
        offset = end;
        i += 1;
    }
    align_up(offset, fields[index].alignment)
}

/// Computes an inline union large enough for every variant payload.
#[must_use]
pub const fn union(variants: &[Layout]) -> Layout {
    let mut size = 0;
    let mut alignment = 1;
    let mut i = 0;
    while i < variants.len() {
        if variants[i].size > size {
            size = variants[i].size;
        }
        if variants[i].alignment > alignment {
            alignment = variants[i].alignment;
        }
        i += 1;
    }
    Layout::new(align_up(size, alignment), alignment)
}

#[cfg(test)]
mod tests {
    use super::{Layout, field_offset, record, union};

    #[test]
    fn fields_and_array_stride_include_padding() {
        let fields = [Layout::new(1, 1), Layout::new(8, 8), Layout::new(2, 2)];
        assert_eq!(field_offset(&fields, 0), 0);
        assert_eq!(field_offset(&fields, 1), 8);
        assert_eq!(field_offset(&fields, 2), 16);
        assert_eq!(record(&fields, 1), Layout::new(24, 8));
        assert_eq!(record(&fields, 1).array(3).size(), 72);
    }

    #[test]
    fn explicit_alignment_preserves_simd_padding() {
        assert_eq!(record(&[Layout::new(4, 4); 3], 16), Layout::new(16, 16));
    }

    #[test]
    fn tags_align_the_largest_payload() {
        let payload = union(&[Layout::UNIT, Layout::new(8, 8), Layout::new(12, 4)]);
        assert_eq!(payload, Layout::new(16, 8));
        assert_eq!(
            Layout::tagged(Layout::new(4, 4), payload),
            Layout::new(24, 8)
        );
    }
}
