use std::{
    alloc::{Layout as AllocationLayout, alloc, alloc_zeroed, dealloc, handle_alloc_error},
    error::Error,
    fmt,
    mem::MaybeUninit,
    ptr::NonNull,
    slice,
};

use intrlay_layout::Layout;

/// A construction error; these checks do not validate incoming payload objects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The value exceeds the supported single-buffer size or a `u32` count.
    SizeOverflow,
    /// The supplied storage cannot hold the complete measured value.
    BufferTooSmall {
        /// Required number of bytes.
        required: usize,
        /// Available number of bytes.
        available: usize,
    },
    /// Externally provided storage does not satisfy its payload alignment.
    Misaligned {
        /// Required starting-address alignment.
        required: usize,
    },
    /// The source's payload layout changed between measurement and writing.
    LayoutChanged {
        /// Allocation requirements computed before writing.
        measured: BufferLayout,
        /// Allocation requirements observed while writing.
        written: BufferLayout,
    },
    /// A repeatable input yielded a different count than it declared.
    SequenceLengthChanged {
        /// Count captured when the sequence input was created.
        expected: usize,
        /// Number of elements observed before construction stopped.
        actual: usize,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SizeOverflow => f.write_str("value exceeds Intrlay's u32 buffer limits"),
            Self::BufferTooSmall {
                required,
                available,
            } => write!(f, "buffer requires {required} bytes, received {available}"),
            Self::Misaligned { required } => {
                write!(f, "buffer address must be aligned to {required} bytes")
            }
            Self::LayoutChanged { measured, written } => {
                write!(
                    f,
                    "source layout changed during construction: {measured:?} -> {written:?}"
                )
            }
            Self::SequenceLengthChanged { expected, actual } => {
                write!(
                    f,
                    "source sequence declared {expected} elements, yielded {actual}"
                )
            }
        }
    }
}
impl Error for EncodeError {}

/// Exact allocation requirements after measuring a complete value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferLayout {
    byte_length: usize,
    alignment: usize,
}

impl BufferLayout {
    /// Returns the complete byte count, including headers and payloads.
    #[must_use]
    pub const fn byte_length(self) -> usize {
        self.byte_length
    }
    /// Returns the alignment required by every payload in the buffer.
    #[must_use]
    pub const fn alignment(self) -> usize {
        self.alignment
    }
}

/// Measures dynamic payload regions in the same order in which they will be written.
#[derive(Clone, Copy, Debug)]
pub struct SizeCursor {
    position: usize,
    alignment: usize,
}

impl SizeCursor {
    /// Reserves the root's fixed header before measuring its dynamic descendants.
    ///
    /// # Errors
    /// Returns [`EncodeError::SizeOverflow`] for an unsupported header size.
    pub fn new(root: Layout) -> Result<Self, EncodeError> {
        u32::try_from(root.size()).map_err(|_| EncodeError::SizeOverflow)?;
        Ok(Self {
            position: root.size(),
            alignment: root.alignment(),
        })
    }

    /// Reserves a fixed-stride region and returns its byte position.
    ///
    /// # Errors
    /// Returns [`EncodeError::SizeOverflow`] if any count or position exceeds `u32`.
    pub fn reserve(&mut self, layout: Layout, count: usize) -> Result<usize, EncodeError> {
        u32::try_from(count).map_err(|_| EncodeError::SizeOverflow)?;
        let start = self
            .position
            .checked_add(layout.alignment() - 1)
            .ok_or(EncodeError::SizeOverflow)?
            & !(layout.alignment() - 1);
        let size = layout
            .size()
            .checked_mul(count)
            .ok_or(EncodeError::SizeOverflow)?;
        let end = start.checked_add(size).ok_or(EncodeError::SizeOverflow)?;
        u32::try_from(end).map_err(|_| EncodeError::SizeOverflow)?;
        self.position = end;
        self.alignment = self.alignment.max(layout.alignment());
        Ok(start)
    }

    /// Returns the measured requirements.
    #[must_use]
    pub const fn finish(self) -> BufferLayout {
        BufferLayout {
            byte_length: self.position,
            alignment: self.alignment,
        }
    }
}

/// An initialized, explicitly aligned allocation owned by Rust.
///
/// This owner is independent of generated view types. It can be retained across
/// asynchronous work or consumed after an FFI operation has finished.
pub struct OwnedBuffer {
    allocation: BufferAllocation,
}

/// Byte storage that can be recovered by consuming its owning view.
///
/// Implemented for `OwnedBuffer`, `Vec<u8>`, `Box<[u8]>`, byte arrays, and shared
/// `Arc<[u8]>`/`Rc<[u8]>` owners. This trait is sealed: borrowed storage must not
/// expose mutable bytes while a parent view still relies on their validity.
pub trait OwnedBytes: AsRef<[u8]> + owned_bytes::Sealed {}

impl<T: AsRef<[u8]> + owned_bytes::Sealed> OwnedBytes for T {}

mod owned_bytes {
    pub trait Sealed {}

    impl Sealed for super::OwnedBuffer {}
    impl Sealed for Vec<u8> {}
    impl Sealed for Box<[u8]> {}
    impl<const N: usize> Sealed for [u8; N] {}
    impl Sealed for std::sync::Arc<[u8]> {}
    impl Sealed for std::rc::Rc<[u8]> {}
}

/// Owns storage while construction has not yet established byte initialization.
/// Its destructor only deallocates, including on a failed or panicking codec.
pub(crate) struct BufferAllocation {
    pointer: NonNull<u8>,
    layout: BufferLayout,
    allocation: AllocationLayout,
}

impl BufferAllocation {
    pub(crate) fn new<const ZEROED: bool>(layout: BufferLayout) -> Result<Self, EncodeError> {
        let allocation =
            AllocationLayout::from_size_align(layout.byte_length.max(1), layout.alignment)
                .map_err(|_| EncodeError::SizeOverflow)?;
        // SAFETY: The layout is valid and nonzero. Uninitialized storage remains
        // private and is exposed only as MaybeUninit until construction succeeds.
        let pointer = NonNull::new(unsafe {
            if ZEROED {
                alloc_zeroed(allocation)
            } else {
                alloc(allocation)
            }
        })
        .unwrap_or_else(|| handle_alloc_error(allocation));
        Ok(Self {
            pointer,
            layout,
            allocation,
        })
    }

    pub(crate) fn as_uninit_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        // SAFETY: The allocation is valid for byte_length bytes and exclusively
        // borrowed. MaybeUninit imposes no initialization requirement.
        unsafe { slice::from_raw_parts_mut(self.pointer.as_ptr().cast(), self.layout.byte_length) }
    }

    /// Publishes this allocation as readable bytes.
    ///
    /// # Safety
    /// Every byte within `layout.byte_length()` must have been initialized.
    pub(crate) unsafe fn assume_init(self) -> OwnedBuffer {
        OwnedBuffer { allocation: self }
    }
}

impl OwnedBuffer {
    /// Allocates and initializes the exact measured buffer, including all padding.
    ///
    /// # Errors
    /// Returns [`EncodeError::SizeOverflow`] if the allocator cannot represent its layout.
    pub fn new(layout: BufferLayout) -> Result<Self, EncodeError> {
        let allocation = BufferAllocation::new::<true>(layout)?;
        // SAFETY: The complete allocation was initialized by alloc_zeroed.
        Ok(unsafe { allocation.assume_init() })
    }

    /// Returns the allocation requirements used to create this owner.
    #[must_use]
    pub const fn layout(&self) -> BufferLayout {
        self.allocation.layout
    }
}

impl AsRef<[u8]> for OwnedBuffer {
    fn as_ref(&self) -> &[u8] {
        // SAFETY: This owner retains an initialized allocation of at least byte_length bytes.
        unsafe {
            slice::from_raw_parts(
                self.allocation.pointer.as_ptr(),
                self.allocation.layout.byte_length,
            )
        }
    }
}
impl AsMut<[u8]> for OwnedBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        // SAFETY: The exclusive owner borrow prevents aliases through this allocation owner.
        unsafe {
            slice::from_raw_parts_mut(
                self.allocation.pointer.as_ptr(),
                self.allocation.layout.byte_length,
            )
        }
    }
}
impl Drop for BufferAllocation {
    fn drop(&mut self) {
        // SAFETY: The pointer was allocated with this exact layout and is freed exactly once.
        unsafe { dealloc(self.pointer.as_ptr(), self.allocation) };
    }
}
// SAFETY: Moving the owner transfers its unique allocation; it does not retain thread-local state.
unsafe impl Send for OwnedBuffer {}
// SAFETY: Shared borrows expose only immutable bytes; mutation requires an exclusive borrow.
unsafe impl Sync for OwnedBuffer {}

/// Writes into pre-sized storage using the same reservation rules as [`SizeCursor`].
pub struct Writer<'a> {
    bytes: &'a mut [MaybeUninit<u8>],
    cursor: SizeCursor,
}

impl<'a> Writer<'a> {
    /// Begins writing at an already measured root header.
    ///
    /// # Errors
    /// Returns an error if the root header cannot be represented.
    pub fn new(bytes: &'a mut [u8], root: Layout) -> Result<Self, EncodeError> {
        // SAFETY: MaybeUninit<u8> has the same layout as u8, and this exclusive
        // borrow is only written through APIs that copy initialized bytes.
        let bytes = unsafe { slice::from_raw_parts_mut(bytes.as_mut_ptr().cast(), bytes.len()) };
        Self::new_uninit(bytes, root)
    }

    pub(crate) fn new_uninit(
        bytes: &'a mut [MaybeUninit<u8>],
        root: Layout,
    ) -> Result<Self, EncodeError> {
        Ok(Self {
            bytes,
            cursor: SizeCursor::new(root)?,
        })
    }

    /// Checks construction against the layout measured before allocation.
    pub(crate) fn finish(self, measured: BufferLayout) -> Result<(), EncodeError> {
        let written = self.cursor.finish();
        if written != measured {
            return Err(EncodeError::LayoutChanged { measured, written });
        }
        Ok(())
    }

    /// Reserves a payload region without reallocating the underlying storage.
    ///
    /// # Errors
    /// Returns an error for unsupported sizes or insufficient supplied storage.
    pub fn reserve(&mut self, layout: Layout, count: usize) -> Result<usize, EncodeError> {
        let start = self.cursor.reserve(layout, count)?;
        let required = self.cursor.finish().byte_length;
        if required > self.bytes.len() {
            return Err(EncodeError::BufferTooSmall {
                required,
                available: self.bytes.len(),
            });
        }
        Ok(start)
    }

    /// Borrows one fixed header for direct writes of native-compatible fields.
    ///
    /// Field offsets are relative to this header. This writer cannot reserve
    /// payloads or call codecs; custom codecs retain the original writer and
    /// absolute offsets. The fixed slice lets inlining eliminate field checks.
    ///
    /// # Panics
    /// Panics if the requested header is outside this writer's storage.
    #[doc(hidden)]
    #[inline]
    pub fn fixed_header(&mut self, offset: usize, layout: Layout) -> FixedHeader<'_> {
        FixedHeader {
            bytes: &mut self.bytes[offset..][..layout.size()],
        }
    }

    /// Copies initialized bytes into a fixed location.
    ///
    /// # Panics
    /// Panics if an incorrectly implemented codec writes outside its measured region.
    pub fn write(&mut self, offset: usize, bytes: &[u8]) {
        let destination = &mut self.bytes[offset..offset + bytes.len()];
        // SAFETY: The checked destination has exactly bytes.len() writable
        // bytes. The exclusive writer borrow makes source and destination
        // disjoint, and copying initialized bytes initializes this region.
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                destination.as_mut_ptr().cast(),
                bytes.len(),
            );
        }
    }

    /// Initializes a fixed region to zero, including its padding.
    ///
    /// # Panics
    /// Panics if an incorrectly implemented codec exceeds the measured region.
    pub fn zero(&mut self, offset: usize, length: usize) {
        self.bytes[offset..offset + length].fill(MaybeUninit::new(0));
    }

    /// Writes a forward offset relative to this reference header's own address.
    ///
    /// # Errors
    /// Returns an error if the target is behind the reference or exceeds `u32` distance.
    pub fn reference(&mut self, header: usize, target: usize) -> Result<(), EncodeError> {
        let distance = target
            .checked_sub(header)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(EncodeError::SizeOverflow)?;
        self.write(header, &distance.to_le_bytes());
        Ok(())
    }

    /// Writes a count as an unsigned thirty-two-bit integer.
    ///
    /// # Errors
    /// Returns an error if the count exceeds `u32`.
    pub fn count(&mut self, offset: usize, count: usize) -> Result<(), EncodeError> {
        self.write(
            offset,
            &u32::try_from(count)
                .map_err(|_| EncodeError::SizeOverflow)?
                .to_le_bytes(),
        );
        Ok(())
    }
}

/// A reserved header that accepts only native-compatible field copies.
///
/// Generated writers use this to retain a constant-sized destination without
/// changing the absolute offsets passed to field codecs.
#[doc(hidden)]
pub struct FixedHeader<'a> {
    bytes: &'a mut [MaybeUninit<u8>],
}

impl FixedHeader<'_> {
    /// Copies one initialized native field into this header.
    ///
    /// # Safety
    /// Every byte of `value` must be initialized, and its native representation
    /// must match the field's encoded layout. The source must be disjoint from
    /// the destination. Derives establish this through `NATIVE_LAYOUT`.
    ///
    /// # Panics
    /// Panics if the field would exceed this header.
    #[inline]
    pub unsafe fn write_native<T>(&mut self, value: &T, offset: usize) {
        let destination = &mut self.bytes[offset..][..size_of::<T>()];
        // SAFETY: The destination range has exactly the source's size. The
        // caller guarantees initialized source bytes and disjoint storage.
        unsafe {
            std::ptr::copy_nonoverlapping(
                std::ptr::from_ref(value).cast::<u8>(),
                destination.as_mut_ptr().cast(),
                size_of::<T>(),
            );
        }
    }
}
