//! End-to-end checks of generated views, ownership, and rich source types.

use intrlay::{
    Intrlay, OwnedBuffer,
    schema::{Layouts, Registry},
};

#[repr(C)]
#[derive(Debug, PartialEq, Intrlay)]
struct Point {
    x: f32,
    y: f32,
    z: f32,
}

#[repr(C, align(16))]
#[derive(Debug, PartialEq, Intrlay)]
#[intrlay(project = "f32")]
struct PaddedPoint {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, PartialEq, Intrlay)]
struct Node {
    id: u32,
    child: Option<Box<Node>>,
}

#[derive(Debug, PartialEq, Intrlay)]
enum DrawingCommand {
    Clear,
    SetColor([f32; 4]),
    DrawPoint {
        position: Point,
        size: f64,
        label: String,
    },
}

#[derive(Debug, PartialEq, Intrlay)]
struct Frame {
    sequence: u32,
    label: String,
    points: Vec<PaddedPoint>,
    commands: Vec<DrawingCommand>,
    tree: Node,
}

#[derive(Debug, PartialEq, Intrlay)]
struct Generic<T> {
    value: T,
    extra: Option<T>,
}

#[derive(Debug, PartialEq, Intrlay)]
enum GenericOption<T> {
    Empty,
    Value { value: Option<T> },
    View(T),
}

#[test]
fn nested_options_preserve_presence() -> Result<(), Box<dyn std::error::Error>> {
    for value in [None, Some(None), Some(Some(0))] {
        let original = Generic {
            value,
            extra: Some(value),
        };
        let view = GenericView::create(&original)?;
        let decoded: Generic<Option<Option<u32>>> = view.into();
        assert_eq!(decoded, original);
    }
    for original in [
        GenericOption::Empty,
        GenericOption::Value { value: None },
        GenericOption::Value { value: Some(7_u32) },
        GenericOption::View(5),
    ] {
        let view = GenericOptionView::create(&original)?;
        let decoded: GenericOption<u32> = view.into();
        assert_eq!(decoded, original);
    }
    let original = Generic {
        value: Some(()),
        extra: Some(None),
    };
    let view = GenericView::create(&original)?;
    let decoded: Generic<Option<()>> = view.into();
    assert_eq!(decoded, original);
    Ok(())
}

#[test]
fn rich_values_roundtrip_and_mutate_without_relocation() -> Result<(), Box<dyn std::error::Error>> {
    let mut value = Frame {
        sequence: 7,
        label: "drawing 🦀".into(),
        points: vec![
            PaddedPoint {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            PaddedPoint {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
        ],
        commands: vec![
            DrawingCommand::Clear,
            DrawingCommand::DrawPoint {
                position: Point {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                size: 0.75,
                label: "example".into(),
            },
        ],
        tree: Node {
            id: 1,
            child: Some(Box::new(Node { id: 2, child: None })),
        },
    };
    let mut view = FrameView::create(&value)?;
    assert_eq!(view.sequence(), 7);
    assert_eq!(view.label(), "drawing 🦀");
    let length = view.as_bytes().len();
    view.sequence_mut().set(8);
    view.points_mut()
        .get_mut(1)
        .ok_or("missing point")?
        .z_mut()
        .set(12.0);
    assert_eq!(view.as_bytes().len(), length);
    value.sequence = 8;
    value.points[1].z = 12.0;
    let restored: Frame = view.into();
    assert_eq!(restored, value);
    Ok(())
}

#[test]
fn external_storage_retains_ownership_and_initializes_padding()
-> Result<(), Box<dyn std::error::Error>> {
    let value = PaddedPoint {
        x: 2.0,
        y: 4.0,
        z: 8.0,
    };
    let layout = PaddedPointView::compute_layout(&value)?;
    let mut storage = OwnedBuffer::new(layout)?;
    storage.as_mut().fill(0xff);
    {
        let mut view = PaddedPointView::create_in_place(&value, storage.as_mut())?;
        view.y_mut().set(9.0);
        assert_eq!(view.as_bytes()[12..16], [0, 0, 0, 0]);
    }
    // SAFETY: The storage was initialized above and only its scalar value was changed.
    let view = unsafe { PaddedPointView::from_bytes_unchecked(storage.as_ref()) };
    let restored: PaddedPoint = view.into();
    assert_eq!(
        restored,
        PaddedPoint {
            x: 2.0,
            y: 9.0,
            z: 8.0
        }
    );
    Ok(())
}

#[test]
fn compiler_resolves_concrete_generic_metadata_and_layout() -> Result<(), Box<dyn std::error::Error>>
{
    type Concrete = Generic<u64>;
    let value = Concrete {
        value: 42,
        extra: Some(100),
    };
    let view = GenericView::create(&value)?;
    let restored: Concrete = view.into();
    assert_eq!(restored, value);
    let mut registry = Registry::new();
    registry.export::<Concrete>("Concrete");
    registry.export::<Frame>("Frame");
    let schema = registry.finish();
    let layouts = Layouts::new(&schema).validate()?;
    assert_eq!(layouts[std::any::type_name::<Concrete>()], Concrete::LAYOUT);
    assert_eq!(layouts[std::any::type_name::<Frame>()], Frame::LAYOUT);
    Ok(())
}
