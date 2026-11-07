use intrlay_layout::Layout;

/// Compile-time evidence that an encoder covers its headers and payload storage.
///
/// Header coverage alone is insufficient: aligning a later payload may leave a
/// gap after an earlier payload. This summary records both the largest payload
/// alignment and the divisibility shared by all payload sizes. Construction can
/// skip clearing only when the root header and every reservation preserve that
/// alignment. It does not require the source value to have a native wire layout.
#[derive(Clone, Copy, Debug)]
pub struct Initialization {
    headers_written: bool,
    payload_alignment: usize,
    payload_size_multiple: usize,
}

impl Initialization {
    /// No complete byte-coverage proof is available; construction must clear.
    pub const UNKNOWN: Self = Self {
        headers_written: false,
        payload_alignment: 1,
        payload_size_multiple: 1,
    };

    /// Every byte of a fixed encoding is written, with no dynamic payloads.
    pub const FIXED: Self = Self {
        headers_written: true,
        payload_alignment: 1,
        payload_size_multiple: 0,
    };

    /// Combines declaration-ordered fields, including the record's own padding.
    #[must_use]
    pub const fn record(layout: Layout, fields: &[(Layout, Self)]) -> Self {
        let mut result = Self::FIXED;
        let mut end = 0usize;
        let mut index = 0;
        while index < fields.len() {
            let (field, initialization) = fields[index];
            if !end.is_multiple_of(field.alignment()) {
                return Self::UNKNOWN;
            }
            end += field.size();
            result = result.merge(initialization);
            index += 1;
        }
        if end != layout.size() {
            return Self::UNKNOWN;
        }
        result
    }

    /// Adds a dynamic reservation whose element headers have this layout.
    ///
    /// The enclosing codec must also write its complete reference/count header.
    #[must_use]
    pub const fn payload(layout: Layout, element: Self) -> Self {
        element.merge(Self {
            headers_written: true,
            payload_alignment: layout.alignment(),
            payload_size_multiple: layout.size(),
        })
    }

    /// Whether all bytes are covered when construction starts after this header.
    #[must_use]
    pub const fn covers(self, root: Layout) -> bool {
        self.headers_written
            && root.size().is_multiple_of(self.payload_alignment)
            && self
                .payload_size_multiple
                .is_multiple_of(self.payload_alignment)
    }

    const fn merge(self, other: Self) -> Self {
        Self {
            headers_written: self.headers_written && other.headers_written,
            payload_alignment: if self.payload_alignment > other.payload_alignment {
                self.payload_alignment
            } else {
                other.payload_alignment
            },
            // Payload alignments are powers of two. OR preserves exactly the
            // common low zero bits, including zero-sized reservations.
            payload_size_multiple: self.payload_size_multiple | other.payload_size_multiple,
        }
    }
}
