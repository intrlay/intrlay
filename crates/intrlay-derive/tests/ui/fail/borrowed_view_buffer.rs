use wire::{Intrlay, ValueView};

#[derive(Intrlay)]
struct Item { label: String }

#[derive(Intrlay)]
struct Packet { item: Item, items: Vec<Item> }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scalar = ValueView::<bool, _>::from_bytes([1_u8])?;
    scalar.as_view_mut().into_buffer()[0] = 2;

    let mut packet = PacketView::create(&Packet {
        item: Item { label: "valid".into() },
        items: vec![Item { label: "valid".into() }],
    })?;
    // Neither a whole-view reborrow nor a nested mutable view can expose raw
    // bytes while packet remains alive and relies on the validated encoding.
    let _ = packet.as_view_mut().into_buffer();
    let _ = packet.item_mut().into_buffer();
    let _ = packet.items_mut().get_mut(0).unwrap().into_buffer();
    Ok(())
}
