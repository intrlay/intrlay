use wire::Intrlay;

#[derive(Intrlay)]
enum Value {
    Some {
        #[intrlay(getter = extract)]
        field: u32,
    },
}

fn main() {}
