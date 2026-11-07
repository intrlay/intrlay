#[derive(wire::Intrlay)]
#[intrlay(read = false)]
struct OldRead { value: u32 }

#[derive(wire::Intrlay)]
#[intrlay(write = false)]
struct OldWrite { value: u32 }

fn main() {}
