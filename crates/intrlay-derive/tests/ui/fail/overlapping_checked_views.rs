use wire::Intrlay;

#[derive(Intrlay)]
struct Packet { value: u32 }

fn main() {
    let mut bytes = [0; 4];
    let shared = PacketView::from_bytes(&bytes[..]).unwrap();
    let _exclusive = PacketView::from_bytes(&mut bytes[..]).unwrap();
    // The earlier shared view remains live across the exclusive borrow.
    let _ = shared.value();
}
