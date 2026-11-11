//! Optional library codecs preserve values and share ordinary wire representations.

#[cfg(any(feature = "glam-0-33", feature = "glam-0-34"))]
macro_rules! glam_tests {
    ($module:ident, $native:ident) => {
        mod $module {
            use intrlay::adapters::$native as codec;
            use $native as glam;
            include!("library_adapters/glam.rs");
        }
    };
}
#[cfg(feature = "glam-0-33")]
glam_tests!(math_0_33, glam_0_33);
#[cfg(feature = "glam-0-34")]
glam_tests!(math_0_34, glam_0_34);

#[cfg(feature = "smol_str")]
mod strings {
    use intrlay::{Intrlay, adapters::smol_str::SmolStr as Codec};

    #[test]
    fn inline_heap_and_utf8_strings_match_string_encoding() -> Result<(), Box<dyn std::error::Error>>
    {
        for text in [
            "",
            "short",
            "a\0b",
            "UTF-8 🦀 한글",
            "long text requiring heap storage instead of inline storage",
        ] {
            let small = smol_str::SmolStr::new(text);
            let bytes = intrlay::create::<Codec>(&small)?;
            assert_eq!(
                bytes.as_ref(),
                intrlay::create_from::<String>(text)?.as_ref()
            );
            assert_eq!(
                bytes.as_ref(),
                intrlay::create_from::<String>(&small)?.as_ref()
            );
            assert_eq!(
                bytes.as_ref(),
                intrlay::create_from::<Codec>(text)?.as_ref()
            );
            assert_eq!(Codec::decode(bytes.as_ref(), 0)?, small);
            Codec::validate(bytes.as_ref(), 0)?;
            for length in 0..bytes.as_ref().len() {
                assert!(Codec::decode(&bytes.as_ref()[..length], 0).is_err());
            }
        }
        let mut malformed = intrlay::create_from::<Codec>("valid")?;
        malformed.as_mut()[8] = 0xff;
        assert!(Codec::decode(malformed.as_ref(), 0).is_err());
        assert!(Codec::validate(malformed.as_ref(), 0).is_err());
        Ok(())
    }
}

#[cfg(feature = "smallvec")]
mod sequences {
    use intrlay::{Intrlay, SequenceInput, adapters::smallvec::SmallVec as Codec};
    use smallvec::SmallVec;

    #[test]
    fn inline_and_spilled_inputs_match_vec_without_owner_conversion()
    -> Result<(), Box<dyn std::error::Error>> {
        for length in [0, 1, 4, 5, 20] {
            let value: SmallVec<[u32; 4]> = (0..length).collect();
            let bytes = intrlay::create::<Codec<u32, 4>>(&value)?;
            assert_eq!(
                bytes.as_ref(),
                intrlay::create_from::<Vec<u32>>(&value)?.as_ref()
            );
            assert_eq!(
                bytes.as_ref(),
                intrlay::create_from::<Codec<u32, 4>>(value.as_slice())?.as_ref()
            );
            assert_eq!(
                bytes.as_ref(),
                intrlay::create_from::<Codec<u32, 4>>(&SequenceInput::new(0..length))?.as_ref()
            );
            let decoded = Codec::<u32, 4>::decode(bytes.as_ref(), 0)?;
            assert_eq!(decoded, value);
            assert_eq!(decoded.spilled(), length > 4);
            Codec::<u32, 4>::validate(bytes.as_ref(), 0)?;
            assert_eq!(
                // SAFETY: This representation was validated above.
                unsafe { Codec::<u32, 4>::decode_unchecked(bytes.as_ref(), 0) },
                value
            );
            let no_inline = Codec::<u32, 0>::decode(bytes.as_ref(), 0)?;
            assert_eq!(no_inline.as_slice(), value.as_slice());
            let mut unaligned = vec![0xa5];
            unaligned.extend_from_slice(bytes.as_ref());
            assert_eq!(Codec::<u32, 4>::decode(&unaligned, 1)?, value);
            assert_eq!(
                // SAFETY: This is an unaligned copy of the validated encoding.
                unsafe { Codec::<u32, 4>::decode_unchecked(&unaligned, 1) },
                value
            );
            for length in 0..bytes.as_ref().len() {
                assert!(Codec::<u32, 4>::decode(&bytes.as_ref()[..length], 0).is_err());
            }
        }
        Ok(())
    }

    #[test]
    fn nested_dynamic_elements_and_invalid_metadata_use_vec_rules()
    -> Result<(), Box<dyn std::error::Error>> {
        type Strings = Codec<String, 2>;
        let value: SmallVec<[String; 2]> = ["one".into(), "two 🦀".into()].into();
        let bytes = intrlay::create::<Strings>(&value)?;
        assert_eq!(
            bytes.as_ref(),
            intrlay::create_from::<Vec<String>>(&value)?.as_ref()
        );
        assert_eq!(Strings::decode(bytes.as_ref(), 0)?, value);
        assert_eq!(
            // SAFETY: Construction initialized the complete string sequence.
            unsafe { Strings::decode_unchecked(bytes.as_ref(), 0) },
            value
        );
        for length in 0..bytes.as_ref().len() {
            assert!(Strings::decode(&bytes.as_ref()[..length], 0).is_err());
        }
        let mut invalid = bytes.as_ref().to_vec();
        invalid[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Strings::decode(&invalid, 0).is_err());
        assert!(Strings::validate(&invalid, 0).is_err());
        let zeros = intrlay::create_from::<Codec<u32, 4>>(&intrlay::ZeroedSequence::new(6))?;
        assert_eq!(
            Codec::<u32, 4>::decode(zeros.as_ref(), 0)?.as_slice(),
            &[0; 6]
        );
        let mut flags = intrlay::create::<Vec<bool>>(&vec![true, false])?;
        assert_eq!(
            // SAFETY: Construction produced valid boolean representations.
            unsafe { Codec::<bool, 4>::decode_unchecked(flags.as_ref(), 0) }.as_slice(),
            &[true, false]
        );
        flags.as_mut()[9] = 2;
        assert!(Codec::<bool, 4>::decode(flags.as_ref(), 0).is_err());
        let units = intrlay::create::<Vec<()>>(&vec![(); 5])?;
        assert_eq!(Codec::<(), 2>::decode(units.as_ref(), 0)?.len(), 5);
        assert_eq!(
            // SAFETY: Construction produced valid zero-sized element metadata.
            unsafe { Codec::<(), 2>::decode_unchecked(units.as_ref(), 0) }.len(),
            5
        );
        Ok(())
    }
}
