use std::fmt::Write;

use intrlay_schema::{Exposure, Scalar};

use crate::{
    GenerateError,
    api::{Accessor, CollectionAccessors, IndexedAccessors},
    plan::{Field, GenerationPlan, Id, Kind, Node, TypeGraph, Variant, WriterParameters},
};

mod documentation;
mod load;
mod mutation;
mod name_validation;
mod names;

pub const BACKEND: crate::backend::Backend = crate::backend::Backend {
    language: intrlay_schema::Language::TypeScript,
    api: &crate::api::DefaultApi,
    naming_policy: names::policy,
    reserved_declarations: &["intrlay", "Some", "globalThis"],
    is_reserved_name: name_validation::is_reserved,
    generate,
    field_setter: crate::api::SetterSyntax::PropertyWithGetter,
    root_setter: crate::api::SetterSyntax::Property,
    input_field_kind: crate::SymbolKind::Field,
    input_field_modifiers: &[crate::Modifier::Readonly],
    codec_function_kind: crate::SymbolKind::Function,
    codec_function_modifiers: &[],
    fixed_array_api: crate::backend::FixedArrayApi::Numeric,
    native_numeric_array_views: true,
    runtime_numeric_array_writers: false,
};

pub fn generate(
    generation: &GenerationPlan,
    _options: &crate::GenerateOptions,
) -> Result<String, GenerateError> {
    name_validation::validate(generation)?;
    let plan = &generation.encoding.types;
    let mut output = String::from(include_str!("prelude.txt"));
    if !generation.views.emitted.is_empty() {
        writeln!(output, "import * as intrlay from \"@intrlay/runtime\";")?;
    }
    writeln!(output, "export type {{ Some }} from \"@intrlay/runtime\";")?;
    for (id, node) in plan.nodes.iter().enumerate() {
        if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            input(
                &mut output,
                plan,
                id,
                node,
                generation.encoding.needs_input(id),
            )?;
        }
    }
    for (id, node) in plan.nodes.iter().enumerate() {
        if generation.encoding.measures.contains(&id) {
            measure(&mut output, plan, id, node)?;
        }
        if generation.encoding.writers.contains(&id) {
            write_function(&mut output, plan, id, node)?;
        }
    }
    let view_plan = &generation.views.types;
    for &id in &generation.views.emitted {
        let node = &view_plan.nodes[id];
        view(&mut output, view_plan, id, node)?;
    }
    for (alias, id) in &plan.exports {
        if *alias != plan.nodes[*id].name {
            if generation.views.emitted.contains(id) {
                writeln!(
                    output,
                    "export {{ {} as {alias}View }};",
                    plan.view_name(*id)
                )?;
            }
            if matches!(plan.nodes[*id].kind, Kind::Enum(_)) && !plan.nodes[*id].is_value_enum() {
                writeln!(
                    output,
                    "export {{ {} as {alias}Kind }};",
                    plan.nodes[*id].kind_name()
                )?;
            }
        }
        if generation.encoding.needs_input(*id)
            && (*alias != plan.nodes[*id].name
                || !matches!(plan.nodes[*id].kind, Kind::Record(_) | Kind::Enum(_)))
        {
            if plan.nodes[*id].is_value_enum() {
                writeln!(
                    output,
                    "export {{ {} as {alias}Input }};",
                    input_type(plan, *id)
                )?;
            } else {
                writeln!(
                    output,
                    "export type {alias}Input = {};",
                    input_type(plan, *id)
                )?;
            }
        }
    }
    Ok(output)
}

fn input(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
    needed: bool,
) -> Result<(), GenerateError> {
    if !needed && !matches!(node.kind, Kind::Enum(_)) {
        return Ok(());
    }
    let name = plan.input_name(id);
    writeln!(output)?;
    documentation::write(output, &node.docs, "")?;
    if let Some(field) = plan.folded_node(node) {
        writeln!(
            output,
            "export type {name} = {};",
            input_type(plan, field.ty)
        )?;
        return Ok(());
    }
    match &node.kind {
        Kind::Record(fields) => {
            writeln!(output, "export interface {name} {{")?;
            for field in fields {
                documentation::write(output, &field.docs, "    ")?;
                writeln!(
                    output,
                    "    readonly {}: {};",
                    field.name,
                    input_type(plan, field.ty)
                )?;
            }
            writeln!(output, "}}")?;
        }
        Kind::Enum(variants) => {
            let kind = node.tag_type_name();
            writeln!(output, "export const enum {kind} {{")?;
            for variant in variants {
                documentation::write(output, &variant.docs, "    ")?;
                writeln!(output, "    {} = {},", variant.name, variant.tag)?;
            }
            writeln!(output, "}}")?;
            if node.is_value_enum() || !needed {
                return Ok(());
            }
            writeln!(output)?;
            documentation::write(output, &node.docs, "")?;
            writeln!(output, "export type {name} =")?;
            for variant in variants {
                writeln!(output, "    | {{")?;
                documentation::write(output, &variant.docs, "        ")?;
                writeln!(output, "        readonly kind: {kind}.{};", variant.name)?;
                for field in &variant.fields {
                    documentation::write(output, &field.docs, "        ")?;
                    writeln!(
                        output,
                        "        readonly {}: {};",
                        field.name,
                        input_type(plan, field.ty)
                    )?;
                }
                writeln!(output, "    }}")?;
            }
            writeln!(output, ";")?;
        }
        _ => {}
    }
    Ok(())
}

fn measure(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
) -> Result<(), GenerateError> {
    let value = plan.measure_parameter(id);
    writeln!(
        output,
        "\nfunction $[measure:{id}]({value}: {}, cursor: intrlay.Cursor): void {{",
        input_type(plan, id)
    )?;
    match &node.kind {
        Kind::String => writeln!(output, "    cursor.reserve(intrlay.utf8Length(value), 1);")?,
        Kind::Record(fields) => {
            for field in fields.iter().filter(|field| plan.has_payload(field.ty)) {
                writeln!(
                    output,
                    "    $[measure:{}]({}, cursor);",
                    field.ty,
                    input_member(plan, node, field)
                )?;
            }
        }
        Kind::Enum(variants) => {
            writeln!(output, "    switch (value.kind) {{")?;
            for variant in variants {
                writeln!(
                    output,
                    "        case {}.{}:",
                    node.kind_name(),
                    variant.name
                )?;
                for field in variant
                    .fields
                    .iter()
                    .filter(|field| plan.has_payload(field.ty))
                {
                    writeln!(
                        output,
                        "            $[measure:{}](value.{}, cursor);",
                        field.ty, field.name
                    )?;
                }
                writeln!(output, "            break;")?;
            }
            writeln!(output, "    }}")?;
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            let element_node = &plan.nodes[*element];
            if matches!(node.kind, Kind::Sequence(_)) {
                writeln!(
                    output,
                    "    cursor.reserve(intrlay.count(value) * {}, {});",
                    element_node.layout.size(),
                    element_node.layout.alignment()
                )?;
            }
            if plan.has_payload(*element) {
                writeln!(
                    output,
                    "    if (typeof value !== \"number\") {{\n        for (let index = 0; index < value.length; ++index) {{\n            $[measure:{element}](value[index]!, cursor);\n        }}\n    }}"
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
                "    if (value !== null) {{\n        $[measure:{value}]({payload}, cursor);\n    }}"
            )?;
        }
        Kind::Boxed(value) => {
            let child = &plan.nodes[*value];
            writeln!(
                output,
                "    cursor.reserve({}, {});",
                child.layout.size(),
                child.layout.alignment()
            )?;
            if plan.has_payload(*value) {
                writeln!(output, "    $[measure:{value}](value, cursor);")?;
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
                "    if (typeof value === \"symbol\") {{\n        storage.u8.fill(0, offset, offset + {});\n        return;\n    }}",
                node.layout.size()
            )?;
        }
        writeln!(
            output,
            "    if (value.length !== {length}) {{\n        throw new globalThis.RangeError(\"fixed array length\");\n    }}"
        )?;
    }
    Ok(())
}

fn write_function(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
) -> Result<(), GenerateError> {
    let optional = if plan.has_payload(id) {
        ""
    } else {
        " | undefined"
    };
    let WriterParameters {
        storage,
        offset,
        value,
        cursor,
    } = plan.writer_parameters(id);
    writeln!(
        output,
        "\nfunction $[write:{id}]({storage}: intrlay.Storage, {offset}: number, {value}: {}, {cursor}: intrlay.Cursor{optional}): void {{",
        input_type(plan, id)
    )?;
    initialize_fixed_array(output, plan, id, node)?;
    match &node.kind {
        Kind::Scalar(scalar) => writeln!(output, "    {};", store(*scalar, "offset", "value"))?,
        Kind::Unit | Kind::Array { length: 0, .. } => {}
        Kind::String => writeln!(
            output,
            "    const length = intrlay.utf8Length(value);\n    const payloadOffset = cursor.reserve(length, 1);\n    storage.u32[offset >>> 2] = payloadOffset - offset;\n    storage.u32[(offset >>> 2) + 1] = length;\n    intrlay.encodeString(value, storage.bytes.subarray(payloadOffset, payloadOffset + length));"
        )?,
        Kind::Record(fields) => {
            for field in fields {
                writeln!(
                    output,
                    "    $[write:{}](storage, offset + {}, {}, cursor);",
                    field.ty,
                    field.offset,
                    input_member(plan, node, field)
                )?;
            }
        }
        Kind::Enum(variants) => write_enum(output, node, variants)?,
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            write_array(output, plan, node, *element)?;
        }
        Kind::Optional { value, offset } => {
            let payload = if plan.nullable_value(*value) {
                "value.value"
            } else {
                "value"
            };
            writeln!(
                output,
                "    storage.u8[offset] = value === null ? 0 : 1;\n    if (value !== null) {{\n        $[write:{value}](storage, offset + {offset}, {payload}, cursor);\n    }}"
            )?;
        }
        Kind::Boxed(value) => {
            let child = &plan.nodes[*value];
            writeln!(
                output,
                "    const payloadOffset = cursor.reserve({}, {});\n    storage.u32[offset >>> 2] = payloadOffset - offset;\n    $[write:{value}](storage, payloadOffset, value, cursor);",
                child.layout.size(),
                child.layout.alignment()
            )?;
        }
    }
    writeln!(output, "}}")?;
    Ok(())
}

fn write_enum(output: &mut String, node: &Node, variants: &[Variant]) -> Result<(), GenerateError> {
    if node.is_value_enum() {
        writeln!(output, "    switch (value) {{")?;
        for variant in variants {
            writeln!(
                output,
                "        case {}.{}:",
                node.input_name(),
                variant.name
            )?;
        }
        writeln!(
            output,
            "            storage.u32[offset >>> 2] = value;\n            break;"
        )?;
    } else {
        writeln!(output, "    switch (value.kind) {{")?;
        for variant in variants {
            writeln!(
                output,
                "        case {}.{}:\n            storage.u32[offset >>> 2] = {};",
                node.kind_name(),
                variant.name,
                variant.tag
            )?;
            for field in &variant.fields {
                writeln!(
                    output,
                    "            $[write:{}](storage, offset + {}, value.{}, cursor);",
                    field.ty, field.offset, field.name
                )?;
            }
            writeln!(output, "            break;")?;
        }
    }
    writeln!(
        output,
        "        default: throw new globalThis.RangeError(\"Unknown enum tag.\");\n    }}"
    )?;
    Ok(())
}

fn write_array(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    element: Id,
) -> Result<(), GenerateError> {
    let element_node = &plan.nodes[element];
    if matches!(node.kind, Kind::Sequence(_)) {
        writeln!(
            output,
            "    const length = intrlay.count(value);\n    const payloadOffset = cursor.reserve(length * {}, {});\n    storage.u32[offset >>> 2] = payloadOffset - offset;\n    storage.u32[(offset >>> 2) + 1] = length;",
            element_node.layout.size(),
            element_node.layout.alignment()
        )?;
    } else {
        writeln!(output, "    const payloadOffset = offset;")?;
    }
    let bulk_scalar = match element_node.kind {
        Kind::Scalar(scalar) if scalar != Scalar::Bool => {
            // Node 24 / V8: set dispatch costs more than a short fixed loop.
            // Byte loops remain competitive longer than wider numeric lanes.
            let loop_limit = if scalar.layout().size() == 1 { 16 } else { 8 };
            if matches!(node.kind, Kind::Array { length, .. } if length <= loop_limit) {
                None
            } else {
                Some(scalar)
            }
        }
        _ => None,
    };
    writeln!(output, "    if (typeof value !== \"number\") {{")?;
    let indent = if let Some(scalar) = bulk_scalar {
        // The input contract already requires ArrayLike. ArrayBuffer views here
        // are typed arrays; set also preserves conversion between numeric lanes.
        writeln!(
            output,
            "        if (globalThis.ArrayBuffer.isView(value)) {{\n            storage.{}.set(value, {});\n        }} else {{",
            storage_name(scalar),
            storage_index(scalar, "payloadOffset")
        )?;
        "            "
    } else {
        "        "
    };
    writeln!(
        output,
        "{indent}for (let index = 0; index < value.length; ++index) {{\n{indent}    $[write:{element}](storage, payloadOffset + index * {}, value[index]!, cursor);\n{indent}}}",
        element_node.layout.size()
    )?;
    if bulk_scalar.is_some() {
        writeln!(output, "        }}")?;
    }
    writeln!(output, "    }}")?;
    Ok(())
}

fn view(output: &mut String, plan: &TypeGraph, id: Id, node: &Node) -> Result<(), GenerateError> {
    let name = plan.view_name(id);
    let mut members = String::new();
    view_members(&mut members, plan, id, node)?;
    writeln!(output)?;
    documentation::write(
        output,
        &crate::documentation::with_note(
            &node.docs,
            "Borrowed fixed-layout view. The caller retains the backing buffer.",
        ),
        "",
    )?;
    // Keep storage only for emitted operations, not for disabled accessors.
    let mut fields = String::new();
    let mut initializers = String::new();
    for (key, value) in [
        ("offset", "offset"),
        (
            "element_offset",
            "offset + this.$[storage].u32[offset >>> 2]!",
        ),
        ("element_count", "this.$[storage].u32[(offset >>> 2) + 1]!"),
    ] {
        if members.contains(&format!("$[{key}]")) {
            write!(fields, "\n    private readonly $[{key}]: number;")?;
            write!(initializers, "\n        this.$[{key}] = {value};")?;
        }
    }
    let offset = if initializers.is_empty() {
        "_offset"
    } else {
        "offset"
    };
    writeln!(
        output,
        "export class {name} {{\n    public static readonly $[byte_size] = {};\n    public static readonly $[alignment] = {};\n    private readonly $[storage]: intrlay.Storage;{fields}\n\n    public constructor(bytes: globalThis.Uint8Array | intrlay.Storage, {offset} = 0) {{\n        this.$[storage] = bytes instanceof intrlay.Storage ? bytes : new intrlay.Storage(bytes);{initializers}\n    }}\n\n    public get $[bytes](): globalThis.Uint8Array {{ return this.$[storage].bytes; }}",
        node.layout.size(),
        node.buffer_alignment
    )?;
    if node.options.generation.create {
        construction_methods(output, plan, id)?;
    }
    output.push_str(&members);
    writeln!(output, "}}")?;
    Ok(())
}

fn construction_methods(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
) -> Result<(), GenerateError> {
    let name = plan.view_name(id);
    let input = input_type(plan, id);
    let compute_size = compute_size(plan, id);
    let measured_value = if plan.has_payload(id) {
        "value"
    } else {
        "_value"
    };
    let measure = if plan.has_payload(id) {
        format!("$[measure:{id}](value, cursor);")
    } else {
        String::new()
    };
    let finish = if plan.has_payload(id) {
        "\n        cursor.finish(storage.bytes.byteLength);"
    } else {
        ""
    };
    writeln!(
        output,
        r"
    public static $[compute_byte_length]({measured_value}: {input}): number {{
        {compute_size}
    }}

    public static $[create](value: {input}): {name} {{
        const bytes = new globalThis.Uint8Array({name}.$[compute_byte_length](value));
        const storage = new intrlay.Storage(bytes);
        const cursor = new intrlay.Cursor({name}.$[byte_size]);
        $[write:{id}](storage, 0, value, cursor);{finish}
        return new {name}(storage);
    }}

    public static $[create_in_place](bytes: globalThis.Uint8Array | intrlay.EncodingBuffer, value: {input}): {name} {{
        const cursor = globalThis.ArrayBuffer.isView(bytes) ? new intrlay.Cursor(0) : bytes.cursor;
        cursor.end = {name}.$[byte_size];
        {measure}
        const storage = intrlay.prepareWrite(bytes, cursor.end, {name}.$[alignment]);
        cursor.end = {name}.$[byte_size];
        $[write:{id}](storage, 0, value, cursor);{finish}
        return new {name}(storage);
    }}"
    )?;
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
        method(
            output,
            &format!("public $[replace](value: {}): void", input_type(plan, id)),
            &format!("$[write:{codec}](this.$[storage], this.$[offset], value, undefined);"),
            "Replaces the inline value without reallocating its backing storage.",
        )?;
    }
    match &node.kind {
        Kind::Record(fields) => {
            for field in fields {
                let api = Accessor::field(plan, node, field, None);
                field_view(output, plan, field, &api, None)?;
            }
        }
        Kind::Enum(variants) => {
            if node.options.generation.getters {
                writeln!(
                    output,
                    "\n    public get $[tag](): {} {{ return this.$[storage].u32[this.$[offset] >>> 2]! as {}; }}",
                    node.tag_type_name(),
                    node.tag_type_name(),
                )?;
            }
            for variant in variants {
                for field in &variant.fields {
                    let api = Accessor::field(plan, node, field, Some(variant));
                    field_view(output, plan, field, &api, Some(variant.tag))?;
                }
            }
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            sequence_view(output, plan, node, *element)?;
        }
        _ => {
            let api = Accessor::root(plan, id)?;
            if api.getter {
                let ty = output_type(plan, id, node.options.exposure);
                let read = load::Read {
                    id,
                    offset: "this.$[offset]",
                    exposure: node.options.exposure,
                    tag: None,
                };
                getter(output, plan, &api, &ty, &read, "")?;
            }
            if let Some(setter) = &api.setter {
                let mutation = &setter.mutation;
                let signature = if setter.property {
                    format!("public set {}", setter.name)
                } else {
                    format!("public {}", setter.name)
                };
                if !api.getter {
                    writeln!(output)?;
                }
                method(
                    output,
                    &format!("{signature}(value: {})", input_type(plan, mutation.value)),
                    &mutation::body(plan, mutation, "this.$[offset]"),
                    "",
                )?;
            }
        }
    }
    Ok(())
}

fn field_view(
    output: &mut String,
    plan: &TypeGraph,
    field: &Field,
    api: &Accessor,
    tag: Option<u32>,
) -> Result<(), GenerateError> {
    let name = &api.name;
    let position = format!("this.$[offset] + {}", field.offset);
    let has_getter = api.getter;
    if has_getter {
        let optional = if tag.is_some() { " | undefined" } else { "" };
        let ty = format!(
            "{}{optional}",
            output_type(plan, field.ty, field.options.exposure)
        );
        let read = load::Read {
            id: field.ty,
            offset: &position,
            exposure: field.options.exposure,
            tag,
        };
        getter(output, plan, api, &ty, &read, &field.docs)?;
    }
    if let Some(setter) = &api.setter {
        let mutation = &setter.mutation;
        let guard = tag.map_or(String::new(), |tag| {
            format!(
                "if (this.$[storage].u32[this.$[offset] >>> 2] !== {tag}) {{\n    return;\n}}\n"
            )
        });
        let signature = if setter.property {
            format!(
                "public set {name}(value: {})",
                input_type(plan, mutation.value)
            )
        } else {
            format!(
                "public {}(value: {}): void",
                setter.name.clone(),
                input_type(plan, mutation.value)
            )
        };
        if !has_getter {
            writeln!(output)?;
        }
        method(
            output,
            &signature,
            &format!("{guard}{}", mutation::body(plan, mutation, &position)),
            &field.docs,
        )?;
    }
    Ok(())
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
    if let Some(cache) = &api.cache {
        let field = &cache.value;
        writeln!(
            output,
            "    private {field}: {ty} | typeof intrlay.uninitialized = intrlay.uninitialized;"
        )?;
        if let Some(state) = &cache.state {
            writeln!(output, "    private {state} = -2;")?;
        }
    }
    method(
        output,
        &format!("public get {name}(): {ty}"),
        &load::body(plan, read, api.cache.as_ref()),
        docs,
    )
}

fn method(
    output: &mut String,
    signature: &str,
    body: &str,
    docs: &str,
) -> Result<(), GenerateError> {
    let inline = format!("    {signature} {{ {body} }}");
    documentation::write(output, docs, "    ")?;
    if inline.len() <= 120 && !body.contains('\n') {
        writeln!(output, "{inline}")?;
    } else {
        let body = body.replace('\n', "\n        ");
        writeln!(output, "    {signature} {{\n        {body}\n    }}")?;
    }
    Ok(())
}

fn output_type(plan: &TypeGraph, id: Id, exposure: Exposure) -> String {
    if !matches!(exposure, Exposure::TypedArray(_))
        && let Some(field) = plan.folded_field(id)
    {
        return output_type(plan, field.ty, field.options.exposure);
    }
    if let Exposure::TypedArray(scalar) = exposure {
        return array_type(scalar).into();
    }
    match &plan.nodes[id].kind {
        Kind::Scalar(scalar) => scalar_type(*scalar).into(),
        Kind::Unit => "null".into(),
        Kind::String => "string".into(),
        Kind::Optional { value, .. } => {
            let payload = output_type(plan, *value, plan.nodes[*value].options.exposure);
            if plan.nullable_value(*value) {
                format!("intrlay.Some<{payload}> | null")
            } else {
                format!("{payload} | null")
            }
        }
        Kind::Boxed(value) => output_type(plan, *value, plan.nodes[*value].options.exposure),
        _ => plan.view_name(id),
    }
}

fn store(scalar: Scalar, offset: &str, value: &str) -> String {
    let value = if scalar == Scalar::Bool {
        format!("({value} ? 1 : 0)")
    } else {
        value.into()
    };
    format!(
        "storage.{}[{}] = {value}",
        storage_name(scalar),
        storage_index(scalar, offset)
    )
}

fn storage_index(scalar: Scalar, offset: &str) -> String {
    // Encoded byte offsets use the full u32 range, including its sign bit.
    let shift = scalar.layout().size().trailing_zeros();
    if shift == 0 {
        offset.into()
    } else {
        format!("({offset}) >>> {shift}")
    }
}

fn storage_name(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Bool | Scalar::U8 => "u8",
        Scalar::I8 => "i8",
        Scalar::I16 => "i16",
        Scalar::U16 => "u16",
        Scalar::I32 => "i32",
        Scalar::U32 => "u32",
        Scalar::I64 => "i64",
        Scalar::U64 => "u64",
        Scalar::F32 => "f32",
        Scalar::F64 => "f64",
    }
}
fn array_type(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Bool | Scalar::U8 => "globalThis.Uint8Array",
        Scalar::I8 => "globalThis.Int8Array",
        Scalar::I16 => "globalThis.Int16Array",
        Scalar::U16 => "globalThis.Uint16Array",
        Scalar::I32 => "globalThis.Int32Array",
        Scalar::U32 => "globalThis.Uint32Array",
        Scalar::I64 => "globalThis.BigInt64Array",
        Scalar::U64 => "globalThis.BigUint64Array",
        Scalar::F32 => "globalThis.Float32Array",
        Scalar::F64 => "globalThis.Float64Array",
    }
}
fn scalar_type(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Bool => "boolean",
        Scalar::I64 | Scalar::U64 => "bigint",
        _ => "number",
    }
}
fn input_type(plan: &TypeGraph, id: Id) -> String {
    match &plan.nodes[id].kind {
        Kind::Scalar(scalar) => scalar_type(*scalar).into(),
        Kind::Unit => "null".into(),
        Kind::String => "string".into(),
        Kind::Record(_) | Kind::Enum(_) => plan.input_name(id),
        Kind::Sequence(element) => {
            let count = if plan.zeroable(*element) {
                " | intrlay.ZeroedSequence"
            } else {
                ""
            };
            format!(
                "globalThis.ArrayLike<{}>{count}",
                input_type(plan, *element)
            )
        }
        Kind::Array { element, .. } => {
            let zeroed = if plan.zeroable(id) {
                " | intrlay.Zeroed"
            } else {
                ""
            };
            format!(
                "globalThis.ArrayLike<{}>{zeroed}",
                input_type(plan, *element)
            )
        }
        Kind::Optional { value, .. } => {
            let value_type = input_type(plan, *value);
            if plan.nullable_value(*value) {
                format!("intrlay.Some<{value_type}> | null")
            } else {
                format!("{value_type} | null")
            }
        }
        Kind::Boxed(value) => input_type(plan, *value),
    }
}

fn sequence_view(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    element: Id,
) -> Result<(), GenerateError> {
    let child = &plan.nodes[element];
    let (position, length) = if let Kind::Array { length, .. } = node.kind {
        ("this.$[offset]".to_owned(), length.to_string())
    } else {
        (
            "this.$[element_offset]".into(),
            "this.$[element_count]".into(),
        )
    };
    writeln!(
        output,
        "\n    public get $[length](): number {{ return {length}; }}"
    )?;
    let api = CollectionAccessors::new(plan, node, element);
    if api.getter {
        let index = load::index_parameter(plan, element, child.options.exposure);
        writeln!(output)?;
        method(
            output,
            &format!(
                "public $[get]({index}: number): {}",
                output_type(plan, element, child.options.exposure)
            ),
            &load::body(
                plan,
                &load::Read {
                    id: element,
                    offset: &format!("({position}) + index * {}", child.layout.size()),
                    exposure: child.options.exposure,
                    tag: None,
                },
                None,
            ),
            "",
        )?;
    }
    if let Some(mutation) = &api.setter {
        if !api.getter {
            writeln!(output)?;
        }
        method(
            output,
            &format!(
                "public $[set](index: number, value: {}): void",
                input_type(plan, mutation.value)
            ),
            &mutation::body(
                plan,
                mutation,
                &format!("({position}) + index * {}", child.layout.size()),
            ),
            "",
        )?;
    }
    // Indexed access is also available on ordinary record arrays, so callers
    // can opt out of per-element view allocation without changing the wire type.
    indexed_fields(output, plan, node, element, &position)
}

fn indexed_fields(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    element: Id,
    position: &str,
) -> Result<(), GenerateError> {
    let child = &plan.nodes[element];
    if plan.folded_node(child).is_none()
        && let Kind::Record(fields) = &child.kind
    {
        for field in fields {
            let p = format!(
                "({position}) + index * {} + {}",
                child.layout.size(),
                field.offset
            );
            let api = IndexedAccessors::new(plan, node, &plan.nodes[element], field);
            numeric_component_accessors(output, plan, &api, field, &p)?;
            if let Some(getter) = &api.getter {
                let index = load::index_parameter(plan, field.ty, field.options.exposure);
                writeln!(output)?;
                method(
                    output,
                    &format!(
                        "public {}({index}: number): {}",
                        getter,
                        output_type(plan, field.ty, field.options.exposure)
                    ),
                    &load::body(
                        plan,
                        &load::Read {
                            id: field.ty,
                            offset: &p,
                            exposure: field.options.exposure,
                            tag: None,
                        },
                        None,
                    ),
                    &crate::documentation::indexed(&field.docs),
                )?;
            }
            if let Some(setter) = &api.setter {
                let mutation = &setter.mutation;
                if api.getter.is_none() {
                    writeln!(output)?;
                }
                method(
                    output,
                    &format!(
                        "public {}(index: number, value: {}): void",
                        setter.name,
                        input_type(plan, mutation.value)
                    ),
                    &mutation::body(plan, mutation, &p),
                    &crate::documentation::indexed(&field.docs),
                )?;
            }
        }
    }
    Ok(())
}

fn compute_size(plan: &TypeGraph, id: Id) -> String {
    let size = format!("{}.$[byte_size]", plan.view_name(id));
    if !plan.has_payload(id) {
        return format!("return {size};");
    }
    format!(
        "const cursor = new intrlay.Cursor({size});\n        $[measure:{id}](value, cursor);\n        return cursor.end;"
    )
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
        let mut prefix = format!("let position = {offset};\n");
        if matches!(plan.nodes[field.ty].kind, Kind::Sequence(_)) {
            prefix.push_str("position += this.$[storage].u32[position >>> 2]!;\n");
        }
        let lane = format!(
            "this.$[storage].{}[({}) + component]",
            storage_name(scalar),
            storage_index(scalar, "position")
        );
        let docs = crate::documentation::indexed_component(&field.docs);
        if let Some(getter) = &api.component_getter {
            writeln!(output)?;
            method(
                output,
                &format!(
                    "public {}(index: number, component: number): {}",
                    getter,
                    scalar_type(scalar)
                ),
                &format!("{prefix}return {lane}!;"),
                &docs,
            )?;
        }
        if let Some(setter) = &api.component_setter {
            if api.component_getter.is_none() {
                writeln!(output)?;
            }
            method(
                output,
                &format!(
                    "public {}(index: number, component: number, value: {}): void",
                    setter,
                    scalar_type(scalar)
                ),
                &format!("{prefix}{lane} = value;"),
                &docs,
            )?;
        }
    }
    Ok(())
}
