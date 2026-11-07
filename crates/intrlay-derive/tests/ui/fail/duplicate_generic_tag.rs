use wire::Intrlay;

#[derive(Intrlay)]
enum Choice<const N: u32> {
    #[intrlay(tag = N)]
    First,
    #[intrlay(tag = 7)]
    Second,
}

const _: wire::layout::Layout = <Choice<7> as Intrlay>::LAYOUT;

fn main() {}
