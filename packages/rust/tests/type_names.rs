//! Internal view parameters must not capture source, field, or adapter types.
#![allow(
    non_camel_case_types,
    reason = "exercise legal internal-looking source names"
)]

use intrlay::Intrlay;
use std::error::Error;

#[derive(Debug, PartialEq, Intrlay)]
#[repr(C)]
struct __IntrlayBuffer {
    value: u32,
}

#[derive(Debug, PartialEq, Intrlay)]
struct __IntrlayBuffer_ {
    value: u32,
}

#[derive(Debug, PartialEq, Intrlay)]
struct Container {
    first: __IntrlayBuffer,
    second: __IntrlayBuffer_,
}

#[derive(Debug, PartialEq, Intrlay)]
enum Choice {
    Value(__IntrlayBuffer),
    Empty,
}

#[test]
fn source_and_field_types_survive_view_parameter_insertion() -> Result<(), Box<dyn Error>> {
    let mut direct = __IntrlayBufferView::create(&__IntrlayBuffer { value: 3 })?;
    direct.value_mut().set(5);
    assert_eq!(__IntrlayBuffer::from(direct), __IntrlayBuffer { value: 5 });
    let source = Container {
        first: __IntrlayBuffer { value: 7 },
        second: __IntrlayBuffer_ { value: 11 },
    };
    let layout = intrlay::compute_layout::<Container>(&source)?;
    let mut bytes = intrlay::OwnedBuffer::new(layout)?;
    let mut view = ContainerView::create_in_place(&source, bytes.as_mut())?;
    view.first_mut().value_mut().set(13);
    assert_eq!(view.first().value(), 13);
    assert_eq!(view.second().value(), 11);
    let choice = ChoiceView::create(&Choice::Value(__IntrlayBuffer { value: 17 }))?;
    assert_eq!(
        Choice::from(choice),
        Choice::Value(__IntrlayBuffer { value: 17 })
    );
    Ok(())
}

mod adapter {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct External {
        value: u32,
    }

    #[derive(Intrlay)]
    #[intrlay(remote = External)]
    #[allow(dead_code, reason = "adapter fields describe the external value")]
    struct __IntrlayBuffer {
        value: u32,
    }

    #[derive(Debug, PartialEq, Intrlay)]
    struct Owner {
        #[intrlay(with = __IntrlayBuffer)]
        external: External,
    }

    #[test]
    fn parsed_codec_paths_are_reserved() -> Result<(), Box<dyn Error>> {
        let source = Owner {
            external: External { value: 19 },
        };
        let mut view = OwnerView::create(&source)?;
        view.external_mut().value_mut().set(23);
        assert_eq!(Owner::from(view).external.value, 23);
        Ok(())
    }
}
