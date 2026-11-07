use wire::Intrlay;

#[derive(Intrlay)]
struct Packet {
    value: bool,
}

#[derive(Intrlay)]
enum Choice {
    Value(u64),
}

fn main() {
    let mut packet = PacketView::create(&Packet { value: true }).unwrap();
    packet.storage.offset = 8;
    packet.storage.buffer.as_mut()[0] = 2;
    packet.storage.as_bytes_mut_unchecked()[0] = 2;

    let mut choice = ChoiceView::create(&Choice::Value(42)).unwrap();
    choice.storage.offset = 8;
    choice.storage = wire::ValueView::<u64, _>::create(&42).unwrap();
}
