//! Template identifiers are resolved before source is returned. Explicit markers
//! keep naming rules away from user literals and target standard-library names.

use crate::{GenerateError, Modifier, SymbolKind, Visibility, plan::TypeGraph};

pub fn render(plan: &TypeGraph, source: &str) -> Result<String, GenerateError> {
    let mut output = String::with_capacity(source.len());
    let mut remaining = source;
    while let Some(start) = remaining.find("$[") {
        output.push_str(&remaining[..start]);
        let marker = &remaining[start + 2..];
        let end = marker
            .find(']')
            .ok_or_else(|| GenerateError("unterminated symbol marker".into()))?;
        output.push_str(&resolve(plan, &marker[..end])?);
        remaining = &marker[end + 1..];
    }
    output.push_str(remaining);
    Ok(output)
}

fn resolve(plan: &TypeGraph, key: &str) -> Result<String, GenerateError> {
    if key == "literal_marker" {
        return Ok("$[".into());
    }
    if let Some(function) = key.strip_prefix("function:") {
        return Ok(plan.names.symbol(
            function,
            SymbolKind::Function,
            Some(Visibility::Internal),
            &[],
        ));
    }
    if let Some(scalar) = key
        .strip_prefix("storage_")
        .filter(|scalar| !matches!(*scalar, "type" | "bytes" | "data"))
    {
        return Ok(plan.names.property(scalar, true));
    }
    if let Some(scalar) = key.strip_prefix("cached_") {
        return Ok(plan
            .names
            .symbol(scalar, SymbolKind::Field, Some(Visibility::Private), &[]));
    }
    if let Some((operation @ ("measure" | "write"), index)) = key.split_once(':') {
        return codec_function(
            plan,
            operation,
            index
                .parse::<usize>()
                .map_err(|error| GenerateError(error.to_string()))?,
        );
    }
    runtime_symbol(plan, key)
}

pub(crate) fn codec_function(
    plan: &TypeGraph,
    operation: &str,
    index: usize,
) -> Result<String, GenerateError> {
    let node = plan
        .nodes
        .get(index)
        .ok_or_else(|| GenerateError(format!("unknown helper type {index}")))?;
    Ok(plan.names.symbol(
        &format!("{operation}_{}", node.name),
        plan.backend.codec_function_kind,
        Some(Visibility::Internal),
        plan.backend.codec_function_modifiers,
    ))
}

pub(crate) fn runtime_symbol(plan: &TypeGraph, key: &str) -> Result<String, GenerateError> {
    let names = &plan.names;
    let result = match key {
        "byte_size" | "alignment" => names.constant(key),
        "create"
        | "create_in_place"
        | "compute_byte_length"
        | "from_sources"
        | "from_bytes"
        | "from_parts"
        | "from_bytes_unchecked"
        | "from_parts_unchecked" => names.method(key, true),
        "get"
        | "copy_to"
        | "set"
        | "replace"
        | "string_at"
        | "view"
        | "as_span"
        | "to_value"
        | "try_get_native_span"
        | "get_value_or_default" => names.method(key, false),
        "bytes" | "tag" | "count" | "length" | "value" => names.property(key, true),
        "storage_bytes" | "storage_data" => names.symbol(
            if key == "storage_bytes" {
                "bytes"
            } else {
                "data"
            },
            SymbolKind::Field,
            Some(Visibility::Public),
            &[Modifier::Readonly],
        ),
        "encoder" | "decoder" => names.symbol(
            key,
            SymbolKind::Field,
            Some(Visibility::Internal),
            &[Modifier::Readonly],
        ),
        "count_function" | "utf8_length" => names.symbol(
            if key == "count_function" {
                "count"
            } else {
                key
            },
            SymbolKind::Function,
            Some(Visibility::Internal),
            &[],
        ),
        "storage" if !plan.backend.language.has_value_types() => names.symbol(
            key,
            SymbolKind::Field,
            Some(Visibility::Public),
            &[Modifier::Readonly],
        ),
        "buffer" | "offset" | "storage" | "element_offset" | "element_count" | "first_element"
        | "optional_storage" => names.symbol(
            key,
            SymbolKind::Field,
            Some(Visibility::Private),
            &[Modifier::Readonly],
        ),
        "cursor" | "codec" | "storage_type" | "base_view" => {
            let source = match key {
                "storage_type" => "Storage",
                "base_view" => "View",
                _ => key,
            };
            names.symbol(source, SymbolKind::Type, Some(Visibility::Internal), &[])
        }
        "optional" | "box" | "some" | "owned_view" => names.type_name(key),
        "metadata" => names.symbol(key, SymbolKind::Type, Some(Visibility::Public), &[]),
        "cursor_end" => names.member("end"),
        "cursor_reserve" => names.method("reserve", false),
        "size_property" => names.property("size", true),
        "aligned_delete" => names.symbol(key, SymbolKind::Type, Some(Visibility::Internal), &[]),
        "owned_storage" | "owned_size" => names.symbol(
            key.strip_prefix("owned_").unwrap_or(key),
            SymbolKind::Field,
            Some(Visibility::Private),
            &[],
        ),
        "read_u32" => names.method("u32", false),
        "buffer_property" => names.property("buffer", true),
        "allocate" | "initialize" => names.symbol(
            key.strip_suffix("_function").unwrap_or(key),
            SymbolKind::Function,
            Some(Visibility::Internal),
            &[],
        ),
        "optional_has_value" => names.property("has_value", true),
        "optional_value" | "box_value" | "some_value" => names.property("value", true),
        _ => return Err(GenerateError(format!("unknown symbol marker: {key}"))),
    };
    Ok(result)
}
