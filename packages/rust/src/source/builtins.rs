use crate::{EncodeError, EncodeSource, Initialization, Intrlay, SizeCursor, Writer};

macro_rules! owned {
    ($($ty:ty),* $(,)?) => {$(
        // SAFETY: The owned codec establishes the same representation and proofs.
        unsafe impl EncodeSource<$ty> for $ty {
            const NATIVE_LAYOUT: bool = <Self as Intrlay>::NATIVE_LAYOUT;
            const WRITES_FIXED_LAYOUT: bool = <Self as Intrlay>::WRITES_FIXED_LAYOUT;
            const INITIALIZATION: Initialization = <Self as Intrlay>::INITIALIZATION;
            fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
                <Self as Intrlay>::measure(self, cursor)
            }
            fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
                <Self as Intrlay>::write(self, writer, offset)
            }
        }
    )*};
}
owned!(
    (),
    bool,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    i64,
    u64,
    f32,
    f64,
    String
);

// SAFETY: str provides valid UTF-8; its bytes use String's reference/count layout.
unsafe impl EncodeSource<String> for str {
    const INITIALIZATION: Initialization = <String as Intrlay>::INITIALIZATION;
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        cursor.reserve(crate::layout::Layout::new(1, 1), self.len())?;
        Ok(())
    }
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        let target = writer.reserve(crate::layout::Layout::new(1, 1), self.len())?;
        writer.reference(offset, target)?;
        writer.count(offset + 4, self.len())?;
        writer.write(target, self.as_bytes());
        Ok(())
    }
}

// SAFETY: Headers are contiguous; element encoders establish descendant validity.
unsafe impl<C: Intrlay, S: EncodeSource<C>> EncodeSource<Vec<C>> for [S] {
    const INITIALIZATION: Initialization = if S::WRITES_FIXED_LAYOUT {
        Initialization::payload(C::LAYOUT, Initialization::FIXED)
    } else {
        Initialization::UNKNOWN
    };
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        cursor.reserve(C::LAYOUT, self.len())?;
        if !S::WRITES_FIXED_LAYOUT {
            for value in self {
                value.measure(cursor)?;
            }
        }
        Ok(())
    }
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        let target = writer.reserve(C::LAYOUT, self.len())?;
        writer.reference(offset, target)?;
        writer.count(offset + 4, self.len())?;
        write_elements::<C, S>(self, writer, target)
    }
}

fn write_elements<C: Intrlay, S: EncodeSource<C>>(
    values: &[S],
    writer: &mut Writer<'_>,
    offset: usize,
) -> Result<(), EncodeError> {
    if S::NATIVE_LAYOUT {
        // SAFETY: S proves its own layout, size, byte order, and full initialization.
        // A slice stores these native elements contiguously, with no additional gaps.
        let bytes = unsafe {
            std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), size_of_val(values))
        };
        writer.write(offset, bytes);
    } else {
        for (index, value) in values.iter().enumerate() {
            value.write(writer, offset + index * C::LAYOUT.size())?;
        }
    }
    Ok(())
}

// SAFETY: Vec delegates to the same contiguous slice encoding without copying.
unsafe impl<C: Intrlay, S: EncodeSource<C>> EncodeSource<Vec<C>> for Vec<S> {
    const INITIALIZATION: Initialization = <[S] as EncodeSource<Vec<C>>>::INITIALIZATION;
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        <[S] as EncodeSource<Vec<C>>>::measure(self, cursor)
    }
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        <[S] as EncodeSource<Vec<C>>>::write(self, writer, offset)
    }
}

// SAFETY: The fixed array has exactly N consecutive element headers.
unsafe impl<C: Intrlay, S: EncodeSource<C>, const N: usize> EncodeSource<[C; N]> for [S; N] {
    const NATIVE_LAYOUT: bool = S::NATIVE_LAYOUT;
    const WRITES_FIXED_LAYOUT: bool = N == 0 || S::WRITES_FIXED_LAYOUT;
    const INITIALIZATION: Initialization = if N == 0 {
        Initialization::FIXED
    } else {
        S::INITIALIZATION
    };
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        if !S::WRITES_FIXED_LAYOUT {
            for value in self {
                value.measure(cursor)?;
            }
        }
        Ok(())
    }
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        write_elements::<C, S>(self, writer, offset)
    }
}

// SAFETY: The presence byte selects the same aligned inline payload as Option<C>.
unsafe impl<C: Intrlay, S: EncodeSource<C>> EncodeSource<Option<C>> for Option<S> {
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        if let Some(value) = self {
            value.measure(cursor)
        } else {
            Ok(())
        }
    }
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        writer.write(offset, &[u8::from(self.is_some())]);
        if let Some(value) = self {
            value.write(
                writer,
                offset
                    + crate::layout::field_offset(
                        &[crate::layout::Layout::new(1, 1), C::LAYOUT],
                        1,
                    ),
            )
        } else {
            Ok(())
        }
    }
}

// SAFETY: The BoxInput adapter writes an independent pointee region, without aliases.
unsafe impl<C: Intrlay, S: EncodeSource<C>> EncodeSource<Box<C>> for Box<S> {
    const INITIALIZATION: Initialization =
        <super::BoxInput<S> as EncodeSource<Box<C>>>::INITIALIZATION;
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        <super::BoxInput<&S> as EncodeSource<Box<C>>>::measure(
            &super::BoxInput(self.as_ref()),
            cursor,
        )
    }
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        <super::BoxInput<&S> as EncodeSource<Box<C>>>::write(
            &super::BoxInput(self.as_ref()),
            writer,
            offset,
        )
    }
}
