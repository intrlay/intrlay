use std::fmt::Write;

use crate::{
    GenerateError,
    api::Accessor,
    plan::{Field, GenerationPlan, Id, Kind, Node, TypeGraph},
};

mod documentation;
mod names;

pub(crate) use names::INTRINSIC_KEYWORDS;

pub const BACKEND: crate::backend::Backend = crate::backend::Backend {
    language: intrlay_schema::Language::CSharp,
    naming_policy: names::policy,
    keywords: names::KEYWORDS,
    reserved_declarations: &[],
    generate,
    field_setter: crate::api::SetterSyntax::Method,
    root_setter: crate::api::SetterSyntax::Method,
    input_field_kind: crate::SymbolKind::Property,
    input_field_modifiers: &[crate::Modifier::Readonly],
    codec_function_kind: crate::SymbolKind::Method,
    codec_function_modifiers: &[crate::Modifier::Static],
    fixed_array_api: crate::backend::FixedArrayApi::Structural,
    native_numeric_array_views: false,
};

pub fn generate(generation: &GenerationPlan) -> Result<String, GenerateError> {
    let source = &generation.encoding.types;
    let mut output = String::from(include_str!("prelude.txt"));
    let namespace = source.csharp_namespace.clone().unwrap_or_else(|| {
        format!(
            "IntrlayGenerated.{}",
            source.names.type_name(&source.module_name)
        )
    });
    for segment in namespace.split('.') {
        if crate::plan::identifier(segment).is_err()
            || INTRINSIC_KEYWORDS.contains(&segment)
            || names::KEYWORDS
                .split_whitespace()
                .any(|keyword| keyword == segment)
        {
            return Err(GenerateError(format!("invalid C# namespace {namespace}")));
        }
    }
    writeln!(
        output,
        "namespace {namespace};\n\n// Native values. Members, constructors, and buffer views are supplied by the source generator.\n"
    )?;
    for node in &source.nodes {
        input(&mut output, node)?;
    }

    // Only resolved layout and API metadata cross into Roslyn. The generator
    // never infers wire offsets from CLR layout or repeats the layout algorithm.
    writeln!(
        output,
        "// Resolved wire layout, consumed during compilation. Type IDs below refer to this schema.\n// WireType: id, kind, name, header size, header alignment, buffer alignment.\n// WireField: owner type, field type, native name, view property name, byte offset.\n// WireVariant: owner type, tag, name, factory name.\n[global::Intrlay.GenerateBindings]\n\n// Public API and storage names after applying the naming policy."
    )?;
    binding_names(&mut output)?;
    let plan = &generation.views.types;
    let encoders = &generation.encoding.encoders;
    let views = &generation.views.emitted;
    for id in 0..plan.nodes.len() {
        type_metadata(&mut output, source, plan, id, encoders, views)?;
    }
    writeln!(
        output,
        "\n// Buffer views to generate. Readonly views preserve the same wire representation."
    )?;
    for &id in views {
        let name = view_name(plan, id);
        writeln!(
            output,
            "[global::Intrlay.WireView({id}, {}, ReadOnly = {}, SpanName = {})]",
            literal(&name)?,
            !plan.nodes[id].options.generation.setters,
            literal(&span_name(&name))?
        )?;
        if plan.nodes[id].options.generation.setters {
            writeln!(
                output,
                "[global::Intrlay.WireView({id}, {}, ReadOnly = true, SpanName = {})]",
                literal(&format!("{name}ReadOnly"))?,
                literal(&format!("{}ReadOnly", span_name(&name)))?
            )?;
        }
        writeln!(output)?;
    }
    for (alias, &id) in &plan.exports {
        let name = format!("{alias}View");
        if name != view_name(plan, id) {
            writeln!(
                output,
                "[global::Intrlay.WireView({id}, {}, ReadOnly = {}, SpanName = {})]",
                literal(&name)?,
                !plan.nodes[id].options.generation.setters,
                literal(&span_name(&name))?
            )?;
        }
    }
    writeln!(
        output,
        "internal static partial class {} {{ }}",
        binding_name(plan)
    )?;
    Ok(output)
}

fn binding_names(output: &mut String) -> Result<(), GenerateError> {
    for key in [
        "byte_size",
        "alignment",
        "bytes",
        "buffer",
        "offset",
        "tag",
        "count",
        "value",
        "set",
        "replace",
        "compute_byte_length",
        "create",
        "create_in_place",
        "as_span",
        "to_value",
        "try_get_native_span",
        "element_offset",
        "element_count",
        "optional_has_value",
        "optional_value",
        "get_value_or_default",
        "first_element",
        "optional_storage",
        "length",
    ] {
        writeln!(
            output,
            "[global::Intrlay.BindingName({}, \"$[{key}]\")]",
            literal(key)?
        )?;
    }
    Ok(())
}

fn input(output: &mut String, node: &Node) -> Result<(), GenerateError> {
    match &node.kind {
        Kind::Record(_) => native_declaration(output, &node.name, &node.docs)?,
        Kind::Enum(variants) => {
            for variant in variants {
                native_declaration(
                    output,
                    &format!("{}{}", node.name, variant.name),
                    &variant.docs,
                )?;
            }
            native_declaration(output, &node.name, &node.docs)?;
        }
        _ => {}
    }
    Ok(())
}

fn native_declaration(output: &mut String, name: &str, docs: &str) -> Result<(), GenerateError> {
    documentation::write(output, docs, "")?;
    writeln!(output, "public readonly partial struct {name} {{ }}\n")?;
    Ok(())
}

fn type_metadata(
    output: &mut String,
    source: &TypeGraph,
    plan: &TypeGraph,
    id: Id,
    encoders: &std::collections::BTreeSet<Id>,
    views: &std::collections::BTreeSet<Id>,
) -> Result<(), GenerateError> {
    let node = &plan.nodes[id];
    writeln!(output, "\n// Type {id}: {}", node.name)?;
    let encoding = node.codec;
    let (measure, write) = if encoders.contains(&encoding) {
        (
            format!("\"$[measure:{encoding}]\""),
            format!("\"$[write:{encoding}]\""),
        )
    } else {
        ("\"\"".into(), "\"\"".into())
    };
    let (kind, child, argument) = match node.kind {
        Kind::Scalar(scalar) => (format!("{scalar:?}"), None, 0),
        Kind::Unit => ("Unit".into(), None, 0),
        Kind::String => ("String".into(), None, 0),
        Kind::Record(_) => ("Record".into(), None, 0),
        Kind::Enum(_) => ("Enum".into(), None, 0),
        Kind::Sequence(value) => ("Sequence".into(), Some(value), 0),
        Kind::Array { element, length } => ("Array".into(), Some(element), length),
        Kind::Optional { value, offset } => ("Optional".into(), Some(value), offset),
        Kind::Boxed(value) => ("Boxed".into(), Some(value), 0),
    };
    let mut attributes = Vec::new();
    if let Some(child) = child {
        attributes.push(format!("Value = {child}"));
    }
    if argument != 0 {
        attributes.push(format!("Argument = {argument}"));
    }
    for (name, value) in [
        ("Read", node.options.generation.getters),
        ("Write", node.options.generation.setters),
        ("Create", node.options.generation.create),
        ("HasPayload", plan.has_payload(id)),
        ("Encode", encoders.contains(&id)),
        ("Native", id < source.nodes.len()),
        ("NativeLayout", source.csharp_native_layout(encoding)),
        ("FullyWritten", source.writes_all_bytes(encoding)),
    ] {
        if value {
            attributes.push(format!("{name} = true"));
        }
    }
    if views.contains(&id) {
        attributes.push(format!("ViewName = {}", literal(&view_name(plan, id))?));
        attributes.push(format!(
            "SpanName = {}",
            literal(&span_name(&view_name(plan, id)))?
        ));
    }
    if let Kind::Array { length, .. } = node.kind {
        attributes.push(format!(
            "ArrayName = {}",
            literal(&array_name(source, length))?
        ));
    }
    if source.csharp_native_layout(encoding) {
        attributes.push(format!(
            "NativeReadName = {}",
            literal(&crate::symbols::codec_function(
                source,
                "read_native",
                encoding
            )?)?
        ));
    }
    if measure != "\"\"" && plan.has_payload(id) {
        attributes.push(format!("MeasureName = {measure}"));
    }
    if write != "\"\"" {
        attributes.push(format!("WriteName = {write}"));
    }
    if !node.docs.is_empty() {
        attributes.push(format!("Documentation = {}", literal(&node.docs)?));
    }
    attribute(
        output,
        &format!(
            "WireType({id}, global::Intrlay.WireKind.{kind}, {}, {}, {}, {}",
            literal(&node.name)?,
            node.layout.size(),
            node.layout.alignment(),
            node.buffer_alignment
        ),
        &attributes,
    )?;
    member_metadata(output, plan, id, node)
}

fn member_metadata(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
) -> Result<(), GenerateError> {
    match &node.kind {
        Kind::Record(fields) => {
            for field in fields {
                field_metadata(output, plan, id, node, field, None)?;
            }
        }
        Kind::Enum(variants) => {
            for variant in variants {
                let docs = if variant.docs.is_empty() {
                    Vec::new()
                } else {
                    vec![format!("Documentation = {}", literal(&variant.docs)?)]
                };
                attribute(
                    output,
                    &format!(
                        "WireVariant({id}, {}, {}, {}",
                        variant.tag,
                        literal(&variant.name)?,
                        literal(&plan.names.method(&format!("from_{}", variant.name), true))?
                    ),
                    &docs,
                )?;
                for field in &variant.fields {
                    field_metadata(output, plan, id, node, field, Some(variant))?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn field_metadata(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
    field: &Field,
    variant: Option<&crate::plan::Variant>,
) -> Result<(), GenerateError> {
    let api = Accessor::field(plan, node, field, variant);
    let property = &api.name;
    let generation = api.generation;
    let mut attributes = Vec::new();
    if generation.getters {
        attributes.push("Read = true".into());
    }
    if generation.setters {
        attributes.push("Write = true".into());
    }
    if let Some(variant) = variant {
        attributes.push(format!("Variant = {}", variant.tag));
    }
    if let Some(setter) = &api.setter {
        attributes.push(format!("SetterName = {}", literal(&setter.name)?));
    }
    // Roslyn decides whether CLR recursion requires this backing field. The
    // shared naming policy supplies its spelling without selecting storage.
    let storage_name = plan.names.symbol(
        &format!("{}_memory", field.name),
        crate::SymbolKind::Field,
        Some(crate::Visibility::Private),
        &[crate::Modifier::Readonly],
    );
    attributes.push(format!("NativeStorageName = {}", literal(&storage_name)?));
    if !field.docs.is_empty() {
        attributes.push(format!("Documentation = {}", literal(&field.docs)?));
    }
    attribute(
        output,
        &format!(
            "WireField({id}, {}, {}, {}, {}",
            field.ty,
            literal(&field.name)?,
            literal(property)?,
            field.offset
        ),
        &attributes,
    )?;
    Ok(())
}

fn attribute(output: &mut String, positional: &str, named: &[String]) -> Result<(), GenerateError> {
    let mut line = format!("[global::Intrlay.{positional}");
    for value in named {
        if line.len() + value.len() + 4 > 116 {
            writeln!(output, "{line},")?;
            line = format!("    {value}");
        } else {
            write!(line, ", {value}")?;
        }
    }
    writeln!(output, "{line})]")?;
    Ok(())
}

pub(crate) fn view_name(plan: &TypeGraph, id: Id) -> String {
    plan.view_name(id)
}

pub(crate) fn binding_name(plan: &TypeGraph) -> String {
    plan.names.type_name("Bindings")
}

pub(crate) fn span_name(view: &str) -> String {
    view.strip_suffix("View")
        .map_or_else(|| format!("{view}Span"), |name| format!("{name}SpanView"))
}

pub(crate) fn array_name(plan: &TypeGraph, length: usize) -> String {
    if length == 0 {
        "global::Intrlay.EmptyArray".into()
    } else {
        plan.names.type_name(&format!("FixedArray{length}"))
    }
}

fn literal(text: &str) -> Result<String, GenerateError> {
    serde_json::to_string(&crate::documentation::escape_symbols(text))
        .map_err(|error| GenerateError(error.to_string()))
}
