//! Plain-text helpers shared by the target documentation emitters.

pub fn with_note(text: &str, note: &str) -> String {
    if text.is_empty() {
        note.into()
    } else {
        format!("{text}\n\n{note}")
    }
}

/// Source text must survive the later template-symbol pass verbatim.
pub fn escape_symbols(text: &str) -> String {
    text.replace("$[", "$[literal_marker]")
}

pub fn space(line: &str) -> &'static str {
    if line.is_empty() { "" } else { " " }
}
/// Appends the caller's precondition for allocation-free indexed field access.
pub fn indexed(text: &str) -> String {
    with_note(
        text,
        "Unchecked indexed access. The caller must supply an integer index in 0 <= index < the collection's element count.",
    )
}

pub fn indexed_component(text: &str) -> String {
    with_note(
        &indexed(text),
        "The component index addresses scalar lanes of the numeric projection, including padding. It must be an integer within that projection's length.",
    )
}
