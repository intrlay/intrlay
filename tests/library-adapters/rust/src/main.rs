#[path = "../../../../target/library-adapters/bindings.rs"]
mod bindings;

use intrlay::Intrlay;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read("target/library-adapters/sample.bin")?;
    let sample = bindings::Sample::decode(&bytes, 0)?;
    assert_eq!(sample.bounds.min.y, -2.0);
    assert_eq!(sample.bounds.max.z, 6.0);
    assert_eq!(sample.points[1].x, 10.0);
    assert_eq!(sample.matrix.y_axis.z, 6.0);
    assert!(sample.mask.x && !sample.mask.y && sample.mask.z);
    assert_eq!(sample.labels, ["one", "two 🦀"]);
    assert_eq!(
        intrlay::create::<bindings::Sample>(&sample)?.as_ref(),
        bytes
    );
    Ok(())
}
