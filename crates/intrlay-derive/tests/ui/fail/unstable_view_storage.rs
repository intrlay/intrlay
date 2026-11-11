use wire::Intrlay;

#[derive(Intrlay)]
struct Packet { label: String }

// A safe AsRef implementation alone does not establish stable storage.
struct Unstable;
impl AsRef<[u8]> for Unstable {
    fn as_ref(&self) -> &[u8] { &[] }
}

fn main() {
    let _ = PacketView::from_bytes(Unstable);
}
