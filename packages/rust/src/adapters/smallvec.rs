//! The `smallvec` feature maps inline/heap storage to ordinary sequences.

use crate::schema::{Describe, Registry, Type};
use crate::{
    DecodeError, Decoder, EncodeError, EncodeSource, Initialization, Intrlay, SequenceInput,
    SizeCursor, Validator, Writer, ZeroedSequence,
};
use std::marker::PhantomData;

/// Codec for `smallvec::SmallVec<[C::Value; N]>` with the layout of `Vec<C>`.
/// `C` is the element codec; `N` controls only the decoded native inline capacity.
/// Both inline and spilled inputs encode directly from their borrowed elements.
pub struct SmallVec<C, const N: usize>(PhantomData<C>);

impl<C: Describe, const N: usize> Describe for SmallVec<C, N> {
    fn describe(registry: &mut Registry) -> Type {
        Vec::<C>::describe(registry)
    }
}

// SAFETY: This codec delegates representation, views, and validation to Vec<C>.
// Reconstruction changes only the owning container and preserves prefix drops on error.
unsafe impl<C: Intrlay, const N: usize> Intrlay for SmallVec<C, N> {
    type Value = smallvec::SmallVec<[C::Value; N]>;
    type View<'a> = <Vec<C> as Intrlay>::View<'a>;
    type ViewMut<'a> = <Vec<C> as Intrlay>::ViewMut<'a>;
    const LAYOUT: crate::layout::Layout = Vec::<C>::LAYOUT;
    const INITIALIZATION: Initialization = Vec::<C>::INITIALIZATION;
    fn measure(value: &Self::Value, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        crate::builtins::measure_sequence::<C>(value.as_slice(), cursor)
    }
    fn write(
        value: &Self::Value,
        writer: &mut Writer<'_>,
        offset: usize,
    ) -> Result<(), EncodeError> {
        crate::builtins::write_sequence::<C>(value.as_slice(), writer, offset)
    }
    unsafe fn view_unchecked(bytes: &[u8], offset: usize) -> Self::View<'_> {
        // SAFETY: The encoding is identical to the valid Vec<C> representation.
        unsafe { Vec::<C>::view_unchecked(bytes, offset) }
    }
    unsafe fn view_mut_unchecked(bytes: &mut [u8], offset: usize) -> Self::ViewMut<'_> {
        // SAFETY: Vec<C> retains the exclusive borrow and fixed sequence extent.
        unsafe { Vec::<C>::view_mut_unchecked(bytes, offset) }
    }
    fn validate_fields(validator: &mut Validator<'_>, offset: usize) -> Result<(), DecodeError> {
        Vec::<C>::validate_fields(validator, offset)
    }
    fn decode_from(decoder: &mut Decoder<'_>, offset: usize) -> Result<Self::Value, DecodeError> {
        let target = decoder.reference(offset, Self::LAYOUT.size())?;
        let length = u32::from_le_bytes(decoder.read(offset + 4)?) as usize;
        decoder.array_region(target, length, C::LAYOUT.size())?;
        let mut values = Self::Value::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| DecodeError::AllocationFailed)?;
        // SAFETY: The empty container has reserved `length` correctly aligned
        // slots, in either inline or heap storage. MaybeUninit permits borrowing
        // these slots without claiming their values are initialized yet.
        let spare = unsafe {
            std::slice::from_raw_parts_mut(
                values
                    .as_mut_ptr()
                    .cast::<std::mem::MaybeUninit<C::Value>>(),
                length,
            )
        };
        if decoder.decode_fixed_elements::<C>(target, spare)? {
            // SAFETY: Fixed-element reconstruction initialized all reserved elements.
            unsafe { values.set_len(length) };
        } else {
            for index in 0..length {
                values.push(decoder.decode::<C>(target + index * C::LAYOUT.size())?);
            }
        }
        Ok(values)
    }
    unsafe fn decode_unchecked(bytes: &[u8], offset: usize) -> Self::Value {
        // SAFETY: The caller guarantees valid sequence metadata and all elements.
        let target = offset + <u32 as crate::Scalar>::read(bytes, offset) as usize;
        let length = <u32 as crate::Scalar>::read(bytes, offset + 4) as usize;
        if C::NATIVE_LAYOUT {
            let mut values = Self::Value::with_capacity(length);
            // SAFETY: Capacity provides exclusive, correctly aligned uninitialized
            // slots. The caller guarantees all source elements are valid, including
            // native types whose tags would otherwise require element-wise checks.
            unsafe {
                let spare = std::slice::from_raw_parts_mut(
                    values
                        .as_mut_ptr()
                        .cast::<std::mem::MaybeUninit<C::Value>>(),
                    length,
                );
                crate::decode::copy_native_elements_unchecked::<C>(bytes, target, spare);
                values.set_len(length);
            }
            return values;
        }
        (0..length)
            .map(|index| {
                // SAFETY: Every index belongs to the validated sequence.
                unsafe { C::decode_unchecked(bytes, target + index * C::LAYOUT.size()) }
            })
            .collect()
    }
}

// SAFETY: SmallVec's elements form one stable contiguous slice for both storage modes.
unsafe impl<C: Intrlay, A: smallvec::Array> EncodeSource<Vec<C>> for smallvec::SmallVec<A>
where
    A::Item: EncodeSource<C>,
{
    const INITIALIZATION: Initialization = <[A::Item] as EncodeSource<Vec<C>>>::INITIALIZATION;
    fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
        <[A::Item] as EncodeSource<Vec<C>>>::measure(self.as_slice(), cursor)
    }
    fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
        <[A::Item] as EncodeSource<Vec<C>>>::write(self.as_slice(), writer, offset)
    }
}

macro_rules! source {
    ([$($generics:tt)*] $source:ty) => {
        // SAFETY: Inputs use the existing Vec<C> writer and its initialization proof.
        unsafe impl<C: Intrlay, const N: usize, $($generics)*> EncodeSource<SmallVec<C, N>> for $source
        where Self: EncodeSource<Vec<C>> {
            const INITIALIZATION: Initialization = <Self as EncodeSource<Vec<C>>>::INITIALIZATION;
            fn measure(&self, cursor: &mut SizeCursor) -> Result<(), EncodeError> {
                <Self as EncodeSource<Vec<C>>>::measure(self, cursor)
            }
            fn write(&self, writer: &mut Writer<'_>, offset: usize) -> Result<(), EncodeError> {
                <Self as EncodeSource<Vec<C>>>::write(self, writer, offset)
            }
        }
    };
}
source!([S][S]);
source!([S] Vec<S>);
source!([A: smallvec::Array] smallvec::SmallVec<A>);
source!([I] SequenceInput<I>);
source!([] ZeroedSequence);
