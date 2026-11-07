//! Generates the shared conformance corpus independently of test execution.

mod fixtures;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: intrlay-conformance OUTPUT_DIRECTORY")?,
    );
    fixtures::collect()?.write_to(&directory)?;
    println!("wrote conformance fixtures to {}", directory.display());
    Ok(())
}
