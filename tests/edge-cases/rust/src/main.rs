#[path = "../../../../target/edge-cases/bindings.rs"]
mod bindings;
#[path = "../../../../target/edge-cases/hygiene.rs"]
mod hygiene;
#[path = "../../../../target/edge-cases/scalar.rs"]
mod scalar;

use intrlay::{layout::Layout, Intrlay, OwnedBuffer, SizeCursor};

fn load(name: &str) -> OwnedBuffer {
    let bytes = std::fs::read(format!("target/edge-cases/{name}.bin")).unwrap();
    let mut cursor = SizeCursor::new(Layout::new(0, 16)).unwrap();
    cursor.reserve(Layout::new(1, 1), bytes.len()).unwrap();
    let mut buffer = OwnedBuffer::new(cursor.finish()).unwrap();
    buffer.as_mut().copy_from_slice(&bytes);
    buffer
}

fn main() {
    let bytes = load("golden");
    // The fixture producer uses the same schema; load preserves its alignment.
    let mut root = unsafe { bindings::EdgeCasesView::from_bytes_unchecked(bytes) };
    let values = [
        "",
        "hello",
        "\u{feff}",
        "\u{feff}hello",
        "\u{feff}\u{feff}hello",
        "a\u{feff}b",
        "한글😀",
    ];
    let text = bindings::TextView::create(&bindings::Text {
        values: values.map(str::to_owned).into(),
    })
    .unwrap();
    assert_eq!(text.as_bytes(), load("text").as_ref());
    for (i, value) in values.into_iter().enumerate() {
        assert_eq!(root.text().values().get(i), Some(value));
    }
    assert_eq!(root.nested().child().unwrap().unwrap().unwrap().value(), 9);
    assert_eq!(root.recursive().child().unwrap().unwrap().value(), 8);
    assert_eq!(root.zeroes().after(), 42);
    assert_eq!(root.shadow().custom().value(), 13);
    assert_eq!(root.shadow().optional(), Some(14));
    assert_eq!(root.shadow().items().get(0), Some(15));
    root.projection_mut()
        .many_mut()
        .get_mut(0)
        .unwrap()
        .x_mut()
        .set(20.0);
    assert_eq!(root.projection().many().get(0).unwrap().x(), 20.0);
    for i in 0..3 {
        let bytes = load(&format!("absent-{i}"));
        // All nested presence combinations come from the same Rust codec.
        let view = unsafe { bindings::NestedView::from_bytes_unchecked(bytes) };
        match i {
            0 => assert!(view.child().is_none()),
            1 => assert!(view.child().unwrap().is_none()),
            _ => assert!(view.child().unwrap().unwrap().is_none()),
        }
    }
    assert_eq!(scalar::IntrlayU32View::create(&123).unwrap().value(), 123);
    assert!(bindings::Point::NATIVE_LAYOUT);
    // The generated module deliberately declares Result/Some/None/Ok/Err types.
    let names = unsafe { hygiene::HygieneView::from_bytes_unchecked(load("hygiene")) };
    assert_eq!(names.result().value(), 1);
    assert_eq!(names.selected().unwrap().value(), 8);
    let native: hygiene::Hygiene = names.into();
    assert_eq!(
        hygiene::HygieneView::create(&native).unwrap().as_bytes(),
        load("hygiene").as_ref()
    );
    println!("Rust boundary values and composed layouts passed");
}
