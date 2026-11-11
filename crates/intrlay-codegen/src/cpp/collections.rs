//! Collection aliases share storage, factories, and element access in the runtime.

use std::fmt::Write;

use crate::{GenerateError, SymbolKind, Visibility, plan::TypeGraph, symbols};

/// Public naming overrides need one module facade, not one class per array type.
pub fn has_custom_names(plan: &TypeGraph) -> Result<bool, GenerateError> {
    for (key, default) in [
        ("metadata", "Metadata"),
        ("byte_size", "byte_size"),
        ("alignment", "alignment"),
        ("bytes", "bytes"),
        ("size_property", "size"),
        ("from_bytes", "from_bytes"),
        ("from_parts", "from_parts"),
        ("from_bytes_unchecked", "from_bytes_unchecked"),
        ("from_parts_unchecked", "from_parts_unchecked"),
        ("get", "get"),
        ("copy_to", "copy_to"),
        ("set", "set"),
        ("replace", "replace"),
        ("compute_byte_length", "compute_byte_length"),
        ("create", "create"),
        ("create_in_place", "create_in_place"),
    ] {
        if symbols::runtime_symbol(plan, key)? != default {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn facade_name(plan: &TypeGraph) -> String {
    plan.names.symbol(
        "collection_view",
        SymbolKind::Type,
        Some(Visibility::Internal),
        &[],
    )
}

pub fn view_template(plan: &TypeGraph, context: &super::Context, custom_names: bool) -> String {
    if custom_names {
        context.qualified(&format!("detail::{}", facade_name(plan)))
    } else {
        "::intrlay::ArrayView".into()
    }
}

pub fn generate_facade(output: &mut String, plan: &TypeGraph) -> Result<(), GenerateError> {
    writeln!(
        output,
        "\n// Collection access uses the module's public naming policy.\nnamespace detail {{"
    )?;
    output.push_str(
        &include_str!("collection_facade.txt").replace("CollectionView", &facade_name(plan)),
    );
    writeln!(output, "}} // namespace detail")?;
    Ok(())
}
