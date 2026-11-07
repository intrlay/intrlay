use wire::Intrlay;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Intrlay)]
    struct Local<T> {
        item: T,
    }

    let mut view = LocalView::create(&Local { item: true })?;
    view.as_view_mut().item_mut().set(false);
    assert!(!view.as_view().item());
    let value: Local<bool> = view.as_view().into();
    assert!(!value.item);
    let bytes = view.into_buffer();
    assert_eq!(bytes.as_ref(), &[0]);
    Ok(())
}
