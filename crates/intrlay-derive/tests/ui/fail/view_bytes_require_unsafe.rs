use wire::Intrlay;

#[derive(Intrlay)]
struct Packet {
    value: bool,
}

fn main() {
    let mut packet = PacketView::create(&Packet { value: true }).unwrap();
    packet.storage.as_bytes_mut_unchecked()[0] = 2;
}
