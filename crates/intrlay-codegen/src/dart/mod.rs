use std::fmt::Write;

use intrlay_schema::{Exposure, Scalar};

use crate::{
    GenerateError,
    api::{Accessor, CollectionAccessors, IndexedAccessors},
    plan::{Field, GenerationPlan, Id, Kind, Node, TypeGraph, WriterParameters},
};

mod api;
mod documentation;
mod load;
mod mutation;
mod name_validation;
mod names;

pub const BACKEND: crate::backend::Backend = crate::backend::Backend {
    language: intrlay_schema::Language::Dart,
    api: &api::Api,
    naming_policy: names::policy,
    reserved_declarations: &["intrlay", "Some", "core", "typed", "convert"],
    is_reserved_name: name_validation::is_reserved,
    generate,
    field_setter: crate::api::SetterSyntax::UntaggedProperty,
    root_setter: crate::api::SetterSyntax::Property,
    input_field_kind: crate::SymbolKind::Field,
    input_field_modifiers: &[crate::Modifier::Readonly],
    codec_function_kind: crate::SymbolKind::Function,
    codec_function_modifiers: &[],
    fixed_array_api: crate::backend::FixedArrayApi::Numeric,
    native_numeric_array_views: true,
    runtime_numeric_array_writers: true,
};

pub fn generate(
    generation: &GenerationPlan,
    _options: &crate::GenerateOptions,
) -> Result<String, GenerateError> {
    name_validation::validate(generation)?;
    let plan = &generation.encoding.types;
    let mut output = String::from(include_str!("prelude.txt"));
    let has_views = !generation.views.emitted.is_empty();
    if has_views || plan.nodes.iter().any(Node::is_value_enum) {
        writeln!(output, "import 'dart:core' as core;")?;
    }
    if has_views {
        writeln!(
            output,
            "import 'dart:typed_data' as typed;\nimport 'package:intrlay/intrlay.dart' as intrlay;"
        )?;
    }
    writeln!(output, "export 'package:intrlay/intrlay.dart' show Some;")?;
    if generation
        .encoding
        .writers
        .iter()
        .any(|id| matches!(plan.nodes[*id].kind, Kind::String))
    {
        writeln!(output, "import 'dart:convert' as convert;")?;
    }
    for (id, node) in plan.nodes.iter().enumerate() {
        if generation.encoding.needs_input(id) {
            input(&mut output, plan, node)?;
        }
    }
    for (id, node) in plan.nodes.iter().enumerate() {
        if generation.encoding.measures.contains(&id) {
            measure(&mut output, plan, id, node)?;
        }
        if generation.encoding.writers.contains(&id) {
            write(&mut output, plan, id, node)?;
        }
    }
    let view_plan = &generation.views.types;
    let borrowed = &generation.views.borrowed;
    for &id in &generation.views.emitted {
        let node = &view_plan.nodes[id];
        view(&mut output, view_plan, id, node, borrowed.contains(&id))?;
    }
    for (alias, id) in &plan.exports {
        if *alias != plan.nodes[*id].name && generation.views.emitted.contains(id) {
            writeln!(output, "typedef {alias}View = {};", plan.view_name(*id))?;
        }
        if generation.encoding.needs_input(*id)
            && (*alias != plan.nodes[*id].name
                || !matches!(plan.nodes[*id].kind, Kind::Record(_) | Kind::Enum(_)))
        {
            writeln!(output, "typedef {alias}Input = {};", input_type(plan, *id))?;
        }
    }
    Ok(output)
}

fn input(output: &mut String, plan: &TypeGraph, node: &Node) -> Result<(), GenerateError> {
    if let Some(field) = plan.folded_node(node) {
        writeln!(output)?;
        documentation::write(output, &node.docs, "")?;
        writeln!(
            output,
            "typedef {} = {};",
            node.input_name(),
            input_type(plan, field.ty)
        )?;
        return Ok(());
    }
    match &node.kind {
        Kind::Record(fields) => {
            input_record(output, plan, &node.input_name(), fields, "", &node.docs)?;
        }
        Kind::Enum(variants) => {
            if node.is_value_enum() {
                let name = node.input_name();
                writeln!(output)?;
                documentation::write(output, &node.docs, "")?;
                writeln!(output, "enum {name} {{")?;
                for (index, variant) in variants.iter().enumerate() {
                    documentation::write(output, &variant.docs, "  ")?;
                    let separator = if index + 1 == variants.len() {
                        ";"
                    } else {
                        ","
                    };
                    writeln!(output, "  {}({}){separator}", variant.name, variant.tag)?;
                }
                writeln!(
                    output,
                    "\n  final core.int $[tag];\n\n  const {name}(this.$[tag]);\n}}"
                )?;
                return Ok(());
            }
            writeln!(output)?;
            documentation::write(output, &node.docs, "")?;
            writeln!(
                output,
                "sealed class {} {{ const {}(); }}",
                node.input_name(),
                node.input_name()
            )?;
            for variant in variants {
                input_record(
                    output,
                    plan,
                    &node.variant_input_name(variant),
                    &variant.fields,
                    &format!(" extends {}", node.input_name()),
                    &variant.docs,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn input_record(
    output: &mut String,
    plan: &TypeGraph,
    name: &str,
    fields: &[Field],
    base: &str,
    docs: &str,
) -> Result<(), GenerateError> {
    writeln!(output)?;
    documentation::write(output, docs, "")?;
    writeln!(output, "class {name}{base} {{")?;
    for field in fields {
        documentation::write(output, &field.docs, "  ")?;
        writeln!(
            output,
            "  final {} {};",
            input_type(plan, field.ty),
            field.name
        )?;
    }
    if fields.is_empty() {
        writeln!(output, "  const {name}();")?;
    } else {
        writeln!(output, "\n  const {name}({{")?;
        for field in fields {
            if field.name.starts_with('_') {
                writeln!(
                    output,
                    "    required {} {},",
                    input_type(plan, field.ty),
                    constructor_parameter(&field.name)
                )?;
            } else {
                writeln!(output, "    required this.{},", field.name)?;
            }
        }
        let initializers = fields
            .iter()
            .filter(|field| field.name.starts_with('_'))
            .map(|field| format!("{} = {}", field.name, constructor_parameter(&field.name)))
            .collect::<Vec<_>>();
        if initializers.is_empty() {
            writeln!(output, "  }});")?;
        } else {
            writeln!(output, "  }}) : {};", initializers.join(",\n       "))?;
        }
    }
    writeln!(output, "}}")?;
    Ok(())
}

/// Named constructor parameters are public even when their backing field is private.
/// Explicit initializers preserve this API on Dart versions before 3.12.
pub(crate) fn constructor_parameter(name: &str) -> &str {
    name.trim_start_matches('_')
}

fn measure(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
) -> Result<(), GenerateError> {
    if !plan.has_payload(id) {
        return Ok(());
    }
    let value = plan.measure_parameter(id);
    writeln!(
        output,
        "\nvoid $[measure:{id}]({} {value}, intrlay.Cursor cursor) {{",
        input_type(plan, id)
    )?;
    match &node.kind {
        Kind::String => writeln!(
            output,
            "  cursor.reserve(intrlay.utf8ByteLength(value), 1);"
        )?,
        Kind::Record(fields) => {
            for field in fields.iter().filter(|field| plan.has_payload(field.ty)) {
                writeln!(
                    output,
                    "  $[measure:{}]({}, cursor);",
                    field.ty,
                    input_member(plan, node, field)
                )?;
            }
        }
        Kind::Enum(variants) => {
            for variant in variants {
                writeln!(
                    output,
                    "  if (value is {}) {{",
                    node.variant_input_name(variant)
                )?;
                for field in variant
                    .fields
                    .iter()
                    .filter(|field| plan.has_payload(field.ty))
                {
                    writeln!(
                        output,
                        "    $[measure:{}](value.{}, cursor);",
                        field.ty, field.name
                    )?;
                }
                writeln!(output, "  }}")?;
            }
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            if matches!(node.kind, Kind::Sequence(_)) {
                writeln!(
                    output,
                    "  cursor.reserve(value.length * {}, {});",
                    plan.nodes[*element].layout.size(),
                    plan.nodes[*element].layout.alignment()
                )?;
            }
            if plan.has_payload(*element) {
                writeln!(
                    output,
                    "  for (final item in value) {{\n    $[measure:{element}](item, cursor);\n  }}"
                )?;
            }
        }
        Kind::Optional { value, .. } => {
            let payload = if plan.nullable_value(*value) {
                "value.value"
            } else {
                "value"
            };
            writeln!(
                output,
                "  if (value != null) {{\n    $[measure:{value}]({payload}, cursor);\n  }}"
            )?;
        }
        Kind::Boxed(value) => {
            writeln!(
                output,
                "  cursor.reserve({}, {});",
                plan.nodes[*value].layout.size(),
                plan.nodes[*value].layout.alignment()
            )?;
            if plan.has_payload(*value) {
                writeln!(output, "  $[measure:{value}](value, cursor);")?;
            }
        }
        _ => {}
    }
    writeln!(output, "}}")?;
    Ok(())
}

fn initialize_fixed_array(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
) -> Result<(), GenerateError> {
    if let Kind::Array { length, .. } = node.kind {
        if plan.zeroable(id) {
            writeln!(
                output,
                "  final items = value.values;\n  if (items == null) {{\n    storage.bytes.fillRange(offset, offset + {}, 0);\n    return;\n  }}",
                node.layout.size()
            )?;
        }
        let values = if plan.zeroable(id) { "items" } else { "value" };
        writeln!(
            output,
            "  if ({values}.length != {length}) {{\n    throw core.ArgumentError('fixed array length');\n  }}"
        )?;
    }
    Ok(())
}

fn write(output: &mut String, plan: &TypeGraph, id: Id, node: &Node) -> Result<(), GenerateError> {
    let optional = if plan.has_payload(id) { "" } else { "?" };
    let WriterParameters {
        storage,
        offset,
        value,
        cursor,
    } = plan.writer_parameters(id);
    writeln!(
        output,
        "\nvoid $[write:{id}](intrlay.WriteStorage {storage}, core.int {offset}, {} {value}, intrlay.Cursor{optional} {cursor}) {{",
        input_type(plan, id)
    )?;
    initialize_fixed_array(output, plan, id, node)?;
    match &node.kind {
        Kind::Scalar(scalar) => match scalar {
            Scalar::Bool => writeln!(output, "  storage.bytes[offset] = value ? 1 : 0;")?,
            Scalar::I8 | Scalar::U8 => writeln!(output, "  storage.bytes[offset] = value;")?,
            _ => writeln!(output, "  storage.write{}(offset, value);", method(*scalar))?,
        },
        Kind::Unit | Kind::Array { length: 0, .. } => {}
        Kind::String => writeln!(
            output,
            "  final data = convert.utf8.encode(value);\n  final payloadOffset = cursor.reserve(data.length, 1);\n  storage.writeUint32(offset, payloadOffset - offset);\n  storage.writeUint32(offset + 4, data.length);\n  storage.bytes.setRange(payloadOffset, payloadOffset + data.length, data);"
        )?,
        Kind::Record(fields) => {
            for field in fields {
                writeln!(
                    output,
                    "  $[write:{}](storage, offset + {}, {}, cursor);",
                    field.ty,
                    field.offset,
                    input_member(plan, node, field)
                )?;
            }
        }
        Kind::Enum(_) if node.is_value_enum() => {
            writeln!(output, "  storage.writeUint32(offset, value.$[tag]);")?;
        }
        Kind::Enum(variants) => {
            for variant in variants {
                writeln!(
                    output,
                    "  if (value is {}) {{\n    storage.writeUint32(offset, {});",
                    node.variant_input_name(variant),
                    variant.tag
                )?;
                for field in &variant.fields {
                    writeln!(
                        output,
                        "    $[write:{}](storage, offset + {}, value.{}, cursor);",
                        field.ty, field.offset, field.name
                    )?;
                }
                writeln!(output, "  }}")?;
            }
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            write_array(output, plan, id, node, *element)?;
        }
        Kind::Optional { value, offset } => {
            let payload = if plan.nullable_value(*value) {
                "value.value"
            } else {
                "value"
            };
            writeln!(
                output,
                "  storage.bytes[offset] = value == null ? 0 : 1;\n  if (value != null) {{\n    $[write:{value}](storage, offset + {offset}, {payload}, cursor);\n  }}"
            )?;
        }
        Kind::Boxed(value) => writeln!(
            output,
            "  final payloadOffset = cursor.reserve({}, {});\n  storage.writeUint32(offset, payloadOffset - offset);\n  $[write:{value}](storage, payloadOffset, value, cursor);",
            plan.nodes[*value].layout.size(),
            plan.nodes[*value].layout.alignment()
        )?,
    }
    writeln!(output, "}}")?;
    Ok(())
}

fn write_array(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
    element: Id,
) -> Result<(), GenerateError> {
    if matches!(node.kind, Kind::Sequence(_)) {
        writeln!(
            output,
            "  final payloadOffset = cursor.reserve(value.length * {}, {});\n  storage.writeUint32(offset, payloadOffset - offset);\n  storage.writeUint32(offset + 4, value.length);",
            plan.nodes[element].layout.size(),
            plan.nodes[element].layout.alignment()
        )?;
        if plan.zeroable(element) {
            writeln!(
                output,
                "  final items = value.values;\n  if (items == null) {{\n    storage.bytes.fillRange(payloadOffset, payloadOffset + value.length * {}, 0);\n    return;\n  }}",
                plan.nodes[element].layout.size()
            )?;
        }
    } else {
        writeln!(output, "  final payloadOffset = offset;")?;
    }
    let values = if plan.zeroable(element)
        || (matches!(node.kind, Kind::Array { .. }) && plan.zeroable(id))
    {
        "items"
    } else {
        "value"
    };
    match &plan.nodes[element].kind {
        Kind::Scalar(scalar) if *scalar != Scalar::Bool => {
            let method = method(*scalar);
            // Dart 3.13 AOT and dart2js have different range-copy setup costs.
            // Keep these fixed-extent choices aligned with the runtime limits
            // used when a sequence's length is only known during encoding.
            let web_loop_limit = if scalar.layout().size() == 1 { 64 } else { 128 };
            match node.kind {
                Kind::Array { length, .. } if length <= 4 => writeln!(
                    output,
                    "  intrlay.write{method}ArrayElements(storage, payloadOffset, {values});"
                )?,
                Kind::Array { length, .. } if length <= web_loop_limit => writeln!(
                    output,
                    "  if (const core.bool.fromEnvironment('dart.library.js_interop')) {{\n    intrlay.write{method}ArrayElements(storage, payloadOffset, {values});\n  }} else {{\n    intrlay.write{method}Array(storage, payloadOffset, {values});\n  }}"
                )?,
                _ => writeln!(
                    output,
                    "  intrlay.write{method}Array(storage, payloadOffset, {values});"
                )?,
            }
        }
        _ => writeln!(
            output,
            "  for (core.int index = 0; index < {values}.length; ++index) {{\n    $[write:{element}](storage, payloadOffset + index * {}, {values}[index], cursor);\n  }}",
            plan.nodes[element].layout.size()
        )?,
    }
    Ok(())
}

fn view(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
    borrowed: bool,
) -> Result<(), GenerateError> {
    let name = plan.view_name(id);
    let mut members = String::new();
    view_members(&mut members, plan, id, node)?;
    // Semantic name markers identify the storage actually used by the emitted
    // accessors, including zero-sized fields and omitted getter/setter APIs.
    let uses_offset = members.contains("$[offset]");
    writeln!(output)?;
    documentation::write(output, &node.docs, "")?;
    writeln!(
        output,
        "class {name} {{\n  static const core.int $[byte_size] = {};\n  static const core.int $[alignment] = {};\n  final intrlay.Storage $[storage];",
        node.layout.size(),
        node.buffer_alignment
    )?;
    if matches!(node.kind, Kind::Sequence(_)) {
        let offset_initializer = if members.contains("$[element_offset]") {
            writeln!(output, "  final core.int $[element_offset];")?;
            "$[element_offset] = offset + $[storage].u32(offset),\n        "
        } else {
            ""
        };
        writeln!(output, "  final core.int $[element_count];")?;
        writeln!(
            output,
            "\n  {name}(typed.Uint8List bytes, [core.int offset = 0]) : this._(intrlay.Storage(bytes), offset);"
        )?;
        writeln!(
            output,
            "\n  {name}._(this.$[storage], core.int offset)\n      : {offset_initializer}$[element_count] = $[storage].u32(offset + 4);"
        )?;
    } else {
        let initializer = if uses_offset {
            writeln!(output, "  final core.int $[offset];")?;
            ", $[offset] = offset"
        } else {
            ""
        };
        let offset = if uses_offset { "offset" } else { "_offset" };
        writeln!(
            output,
            "\n  {name}(typed.Uint8List bytes, [core.int {offset} = 0]) : $[storage] = intrlay.Storage(bytes){initializer};"
        )?;
        if borrowed || node.options.generation.create {
            let offset = if uses_offset {
                "this.$[offset]"
            } else {
                "core.int _"
            };
            writeln!(output, "\n  {name}._(this.$[storage], {offset});")?;
        }
    }
    writeln!(
        output,
        "\n  typed.Uint8List get $[bytes] => $[storage].bytes;"
    )?;
    if node.options.generation.create {
        construction(output, plan, id)?;
    }
    output.push_str(&members);
    writeln!(output, "}}")?;
    Ok(())
}

fn view_members(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
) -> Result<(), GenerateError> {
    if node.options.generation.setters && !plan.has_payload(id) {
        let codec = node.codec;
        writeln!(
            output,
            "\n  /// Replaces the inline value without reallocating its backing storage.\n  void $[replace]({} value) {{\n    $[write:{codec}]($[storage].writer, $[offset], value, null);\n  }}",
            input_type(plan, id)
        )?;
    }
    match &node.kind {
        Kind::Record(fields) => {
            for field in fields {
                let api = Accessor::field(plan, node, field, None);
                property(output, plan, field, &api, None)?;
            }
        }
        Kind::Enum(variants) => {
            if node.options.generation.getters {
                writeln!(
                    output,
                    "\n  core.int get $[tag] => $[storage].u32($[offset]);"
                )?;
            }
            for variant in variants {
                for field in &variant.fields {
                    let api = Accessor::field(plan, node, field, Some(variant));
                    property(output, plan, field, &api, Some(variant.tag))?;
                }
            }
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            sequence_view(output, plan, node, *element)?;
        }
        _ => {
            let api = Accessor::root(plan, id)?;
            if api.getter {
                let ty = output_type(plan, id);
                getter(
                    output,
                    plan,
                    &api,
                    &ty,
                    &load::Read {
                        id,
                        offset: "$[offset]",
                        exposure: node.options.exposure,
                        tag: None,
                    },
                    "",
                )?;
            }
            if let Some(setter) = &api.setter {
                let mutation = &setter.mutation;
                let signature = if setter.property {
                    format!("set {}", setter.name)
                } else {
                    format!("void {}", setter.name)
                };
                if !api.getter {
                    writeln!(output)?;
                }
                writeln!(
                    output,
                    "  {signature}({} value) {{\n    {}\n  }}",
                    input_type(plan, mutation.value),
                    mutation::body(plan, mutation, "$[offset]")
                )?;
            }
        }
    }
    Ok(())
}

fn construction(output: &mut String, plan: &TypeGraph, id: Id) -> Result<(), GenerateError> {
    let name = plan.view_name(id);
    let input = input_type(plan, id);
    let compute_size = compute_size(plan, id);
    let measured_value = if plan.has_payload(id) {
        "value"
    } else {
        "_value"
    };
    let write = if plan.has_payload(id) {
        format!(
            "final cursor = intrlay.Cursor($[byte_size]);\n    $[write:{id}](storage, 0, value, cursor);\n    cursor.finish(storage.bytes.length);"
        )
    } else {
        format!("$[write:{id}](storage, 0, value, intrlay.Cursor($[byte_size]));")
    };
    writeln!(
        output,
        r"
  static core.int $[compute_byte_length]({input} {measured_value}) {{
    {compute_size}
  }}

  static {name} $[create]({input} value) {{
    final storage = intrlay.WriteStorage.allocate($[compute_byte_length](value));
    {write}
    return {name}._(storage, 0);
  }}

  static {name} $[create_in_place](typed.Uint8List bytes, {input} value) {{
    final storage = intrlay.WriteStorage.forWrite(bytes, $[compute_byte_length](value), $[alignment]);
    {write}
    return {name}._(storage, 0);
  }}"
    )?;
    Ok(())
}

fn sequence_view(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    element: Id,
) -> Result<(), GenerateError> {
    let (position, length) = if let Kind::Array { length, .. } = node.kind {
        ("$[offset]".to_owned(), length.to_string())
    } else {
        ("$[element_offset]".into(), "$[element_count]".into())
    };
    writeln!(output, "\n  core.int get $[length] => {length};")?;
    let api = CollectionAccessors::new(plan, node, element);
    if api.getter {
        let index = load::index_parameter(plan, element, plan.nodes[element].options.exposure);
        writeln!(output)?;
        read_method(
            output,
            plan,
            &format!(
                "{} operator [](core.int {index})",
                output_type(plan, element)
            ),
            &load::Read {
                id: element,
                offset: &format!(
                    "({position}) + index * {}",
                    plan.nodes[element].layout.size()
                ),
                exposure: plan.nodes[element].options.exposure,
                tag: None,
            },
        )?;
    }
    if let Some(mutation) = &api.setter {
        if !api.getter {
            writeln!(output)?;
        }
        writeln!(
            output,
            "  void operator []=(core.int index, {} value) {{\n    {}\n  }}",
            input_type(plan, mutation.value),
            mutation::body(
                plan,
                mutation,
                &format!(
                    "({position}) + index * {}",
                    plan.nodes[element].layout.size()
                )
            )
        )?;
    }
    if plan.folded_field(element).is_none()
        && let Kind::Record(fields) = &plan.nodes[element].kind
    {
        for field in fields {
            let offset = format!(
                "({position}) + index * {} + {}",
                plan.nodes[element].layout.size(),
                field.offset
            );
            let api = IndexedAccessors::new(plan, node, &plan.nodes[element], field);
            numeric_component_accessors(output, plan, &api, field, &offset)?;
            if let Some(getter) = &api.getter {
                let index = load::index_parameter(plan, field.ty, field.options.exposure);
                writeln!(output)?;
                documentation::write(output, &crate::documentation::indexed(&field.docs), "  ")?;
                read_method(
                    output,
                    plan,
                    &format!(
                        "{} {}(core.int {index})",
                        output_type(plan, field.ty),
                        getter
                    ),
                    &load::Read {
                        id: field.ty,
                        offset: &offset,
                        exposure: field.options.exposure,
                        tag: None,
                    },
                )?;
            }
            if let Some(setter) = &api.setter {
                let mutation = &setter.mutation;
                if api.getter.is_none() {
                    writeln!(output)?;
                }
                documentation::write(output, &crate::documentation::indexed(&field.docs), "  ")?;
                writeln!(
                    output,
                    "  void {}(core.int index, {} value) {{\n    {}\n  }}",
                    setter.name,
                    input_type(plan, mutation.value),
                    mutation::body(plan, mutation, &offset)
                )?;
            }
        }
    }
    Ok(())
}

fn property(
    output: &mut String,
    plan: &TypeGraph,
    field: &Field,
    api: &Accessor,
    tag: Option<u32>,
) -> Result<(), GenerateError> {
    let name = &api.name;
    let offset = format!("$[offset] + {}", field.offset);
    let has_getter = api.getter;
    if has_getter {
        let ty = projected_type(plan, field.ty, field.options.exposure);
        let ty = if tag.is_some() {
            optional_type(plan, field.ty, &ty)
        } else {
            ty
        };
        getter(
            output,
            plan,
            api,
            &ty,
            &load::Read {
                id: field.ty,
                offset: &offset,
                exposure: field.options.exposure,
                tag,
            },
            &field.docs,
        )?;
    }
    if let Some(setter) = &api.setter {
        let mutation = &setter.mutation;
        let guard = tag.map_or(String::new(), |tag| {
            format!("if ($[storage].u32($[offset]) != {tag}) return; ")
        });
        if !has_getter {
            writeln!(output)?;
        }
        documentation::write(output, &field.docs, "  ")?;
        let signature = if setter.property {
            format!("set {name}")
        } else {
            format!("void {}", setter.name.clone())
        };
        writeln!(
            output,
            "  {signature}({} value) {{\n    {guard}{}\n  }}",
            input_type(plan, mutation.value),
            mutation::body(plan, mutation, &offset)
        )?;
    }
    Ok(())
}

/// Inline discriminant-aware getters need no separate cache initializer.
pub(crate) fn cached_loader_name(
    plan: &TypeGraph,
    id: Id,
    name: &str,
    exposure: Exposure,
    cache: &crate::plan::ViewCache,
) -> Option<String> {
    if cache.state.is_some() {
        return None;
    }
    load::expression(plan, id, "position", exposure)
        .is_none()
        .then(|| {
            plan.names.symbol(
                &format!("load_{name}"),
                crate::SymbolKind::Method,
                Some(crate::Visibility::Private),
                &[],
            )
        })
}

fn getter(
    output: &mut String,
    plan: &TypeGraph,
    api: &Accessor,
    ty: &str,
    read: &load::Read<'_>,
    docs: &str,
) -> Result<(), GenerateError> {
    let name = &api.name;
    writeln!(output)?;
    let Some(config) = &api.cache else {
        documentation::write(output, docs, "  ")?;
        return read_method(output, plan, &format!("{ty} get {name}"), read);
    };
    let cache = &config.value;
    if let Some(state) = &config.state {
        writeln!(output, "  late {ty} {cache};\n  core.int {state} = -1;")?;
        documentation::write(output, docs, "  ")?;
        writeln!(
            output,
            "  {ty} get {name} {{\n    {}\n  }}",
            load::cached_body(plan, read, Some(config)).replace('\n', "\n    ")
        )?;
        return Ok(());
    }
    if let Some(loader) = &api.loader {
        writeln!(output, "  late final {ty} {cache} = {loader}();")?;
        documentation::write(output, docs, "  ")?;
        writeln!(output, "  {ty} get {name} => {cache};\n")?;
        read_method(output, plan, &format!("{ty} {loader}()"), read)?;
    } else {
        let value = load::value(plan, read)
            .ok_or_else(|| GenerateError("missing cached Dart load expression".into()))?
            .replace('\n', "\n  ");
        writeln!(output, "  late final {ty} {cache} = {value};")?;
        documentation::write(output, docs, "  ")?;
        writeln!(output, "  {ty} get {name} => {cache};")?;
    }
    Ok(())
}

fn read_method(
    output: &mut String,
    plan: &TypeGraph,
    signature: &str,
    read: &load::Read<'_>,
) -> Result<(), GenerateError> {
    if let Some(value) = load::value(plan, read)
        && !value.contains('\n')
        && signature.len() + value.len() < 110
    {
        writeln!(output, "  {signature} => {value};")?;
    } else {
        writeln!(
            output,
            "  {signature} {{\n    {}\n  }}",
            load::body(plan, read).replace('\n', "\n    ")
        )?;
    }
    Ok(())
}

fn input_type(plan: &TypeGraph, id: Id) -> String {
    match &plan.nodes[id].kind {
        Kind::Scalar(scalar) => scalar_type(*scalar).into(),
        Kind::Unit => "core.Null".into(),
        Kind::String => "core.String".into(),
        Kind::Record(_) | Kind::Enum(_) => plan.input_name(id),
        Kind::Sequence(element) => format!(
            "{}<{}>",
            if plan.zeroable(*element) {
                "intrlay.SequenceInput"
            } else {
                "core.List"
            },
            input_type(plan, *element)
        ),
        Kind::Array { element, .. } => format!(
            "{}<{}>",
            if plan.zeroable(id) {
                "intrlay.FixedArrayInput"
            } else {
                "core.List"
            },
            input_type(plan, *element)
        ),
        Kind::Optional { value, .. } => optional_type(plan, *value, &input_type(plan, *value)),
        Kind::Boxed(value) => input_type(plan, *value),
    }
}
fn output_type(plan: &TypeGraph, id: Id) -> String {
    if !matches!(plan.nodes[id].options.exposure, Exposure::TypedArray(_))
        && let Some(field) = plan.folded_field(id)
    {
        return output_type(plan, field.ty);
    }
    if let Exposure::TypedArray(scalar) = plan.nodes[id].options.exposure {
        return numeric_type(scalar);
    }
    match &plan.nodes[id].kind {
        Kind::Scalar(scalar) => scalar_type(*scalar).into(),
        Kind::Unit => "core.Null".into(),
        Kind::String => "core.String".into(),
        Kind::Optional { value, .. } => optional_type(plan, *value, &output_type(plan, *value)),
        Kind::Boxed(value) => output_type(plan, *value),
        _ => plan.view_name(id),
    }
}
fn optional_type(plan: &TypeGraph, value: Id, ty: &str) -> String {
    if plan.nullable_value(value) {
        format!("intrlay.Some<{ty}>?")
    } else {
        format!("{ty}?")
    }
}
fn store(scalar: Scalar, offset: &str, value: &str) -> String {
    match scalar {
        Scalar::Bool => format!("storage.bytes[{offset}] = {value} ? 1 : 0"),
        Scalar::U8 => format!("storage.bytes[{offset}] = {value}"),
        Scalar::I8 => format!("storage.data.setInt8({offset}, {value})"),
        Scalar::I64 | Scalar::U64 => format!(
            "intrlay.write{}(storage.data, {offset}, {value})",
            method(scalar)
        ),
        _ => format!(
            "storage.data.set{}({offset}, {value}, typed.Endian.little)",
            method(scalar)
        ),
    }
}
fn scalar_type(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Bool => "core.bool",
        Scalar::F32 | Scalar::F64 => "core.double",
        _ => "core.int",
    }
}
fn method(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::I16 => "Int16",
        Scalar::U16 => "Uint16",
        Scalar::I32 => "Int32",
        Scalar::U32 => "Uint32",
        Scalar::I64 => "Int64",
        Scalar::U64 => "Uint64",
        Scalar::F32 => "Float32",
        Scalar::F64 => "Float64",
        _ => "Uint8",
    }
}

fn compute_size(plan: &TypeGraph, id: Id) -> String {
    let size = "$[byte_size]";
    if !plan.has_payload(id) {
        return format!("return {size};");
    }
    format!(
        "final cursor = intrlay.Cursor({size});\n    $[measure:{id}](value, cursor);\n    return cursor.end;"
    )
}

fn projected_type(plan: &TypeGraph, id: Id, exposure: Exposure) -> String {
    if let Exposure::TypedArray(scalar) = exposure {
        numeric_type(scalar)
    } else if matches!(plan.nodes[id].options.exposure, Exposure::TypedArray(_)) {
        plan.view_name(id)
    } else {
        output_type(plan, id)
    }
}

fn numeric_type(scalar: Scalar) -> String {
    match scalar {
        Scalar::I8 => "typed.Int8List",
        Scalar::U8 | Scalar::Bool => "typed.Uint8List",
        Scalar::I16 => "typed.Int16List",
        Scalar::U16 => "typed.Uint16List",
        Scalar::I32 => "typed.Int32List",
        Scalar::U32 => "typed.Uint32List",
        Scalar::I64 | Scalar::U64 => "core.List<core.int>",
        Scalar::F32 => "typed.Float32List",
        Scalar::F64 => "typed.Float64List",
    }
    .into()
}

fn input_member(plan: &TypeGraph, node: &Node, field: &Field) -> String {
    if plan.folded_node(node).is_some() {
        "value".into()
    } else {
        format!("value.{}", field.name)
    }
}

fn numeric_component_accessors(
    output: &mut String,
    plan: &TypeGraph,
    api: &IndexedAccessors,
    field: &Field,
    offset: &str,
) -> Result<(), GenerateError> {
    if let Some(scalar) = plan.component_scalar(field) {
        let mut prefix = format!("var position = {offset};\n");
        if matches!(plan.nodes[field.ty].kind, Kind::Sequence(_)) {
            prefix.push_str("position += $[storage].u32(position);\n");
        }
        let position = format!("position + component * {}", scalar.layout().size());
        let docs = crate::documentation::indexed_component(&field.docs);
        if let Some(getter) = &api.component_getter {
            let value = load::scalar(scalar, &position);
            writeln!(output)?;
            documentation::write(output, &docs, "  ")?;
            writeln!(
                output,
                "  {} {}(core.int index, core.int component) {{\n    {}return {value};\n  }}",
                scalar_type(scalar),
                getter,
                prefix.replace('\n', "\n    ")
            )?;
        }
        if let Some(setter) = &api.component_setter {
            let write = store(scalar, &position, "value").replace("storage.", "$[storage].");
            if api.component_getter.is_none() {
                writeln!(output)?;
            }
            documentation::write(output, &docs, "  ")?;
            writeln!(
                output,
                "  void {}(core.int index, core.int component, {} value) {{\n    {}{write};\n  }}",
                setter,
                scalar_type(scalar),
                prefix.replace('\n', "\n    ")
            )?;
        }
    }
    Ok(())
}
