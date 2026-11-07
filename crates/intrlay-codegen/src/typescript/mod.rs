use std::fmt::Write;

use intrlay_schema::{Exposure, Scalar};

use crate::{
    GenerateError,
    api::{Accessor, IndexedAccessors},
    plan::{Field, GenerationPlan, Id, Kind, Node, TypeGraph},
};

mod documentation;
mod load;
mod mutation;
mod names;

pub const BACKEND: crate::backend::Backend = crate::backend::Backend {
    language: intrlay_schema::Language::TypeScript,
    naming_policy: names::policy,
    keywords: names::KEYWORDS,
    reserved_declarations: &["intrlay", "Some", "globalThis"],
    generate,
    field_setter: crate::api::SetterSyntax::PropertyWithGetter,
    root_setter: crate::api::SetterSyntax::Property,
    input_field_kind: crate::SymbolKind::Field,
    input_field_modifiers: &[crate::Modifier::Readonly],
    codec_function_kind: crate::SymbolKind::Function,
    codec_function_modifiers: &[],
    fixed_array_api: crate::backend::FixedArrayApi::Numeric,
    native_numeric_array_views: true,
};

pub fn generate(generation: &GenerationPlan) -> Result<String, GenerateError> {
    let plan = &generation.encoding.types;
    let mut output = String::from(include_str!("prelude.txt"));
    let encoders = &generation.encoding.encoders;
    for (id, node) in plan.nodes.iter().enumerate() {
        if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            input(&mut output, plan, id, node)?;
        }
    }
    for (id, node) in plan.nodes.iter().enumerate() {
        if encoders.contains(&id) {
            construction(&mut output, plan, id, node)?;
        }
    }
    let view_plan = &generation.views.types;
    for &id in &generation.views.emitted {
        let node = &view_plan.nodes[id];
        view(&mut output, view_plan, id, node)?;
    }
    for (alias, id) in &plan.exports {
        if *alias != plan.nodes[*id].name {
            writeln!(
                output,
                "export {{ {} as {alias}View }};",
                plan.view_name(*id)
            )?;
        }
        if *alias != plan.nodes[*id].name
            || !matches!(plan.nodes[*id].kind, Kind::Record(_) | Kind::Enum(_))
        {
            writeln!(output, "export type {alias} = {};", input_type(plan, *id))?;
        }
    }
    Ok(output)
}

fn input(output: &mut String, plan: &TypeGraph, id: Id, node: &Node) -> Result<(), GenerateError> {
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
            writeln!(output, "export type {name} =")?;
            for variant in variants {
                writeln!(output, "    | {{")?;
                documentation::write(output, &variant.docs, "        ")?;
                writeln!(output, "        readonly kind: \"{}\";", variant.name)?;
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

fn construction(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
) -> Result<(), GenerateError> {
    if !plan.has_payload(id) {
        return write_function(output, plan, id, node);
    }
    writeln!(
        output,
        "\nfunction $[measure:{id}](value: {}, cursor: intrlay.Cursor): void {{",
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
                writeln!(output, "        case \"{}\":", variant.name)?;
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
    write_function(output, plan, id, node)
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
    writeln!(
        output,
        "\nfunction $[write:{id}](storage: intrlay.Storage, offset: number, value: {}, cursor: intrlay.Cursor{optional}): void {{",
        input_type(plan, id)
    )?;
    if let Kind::Array { length, .. } = node.kind {
        writeln!(
            output,
            "    if ((typeof value === \"number\" ? value : value.length) !== {length}) {{\n        throw new globalThis.RangeError(\"fixed array length\");\n    }}"
        )?;
    }
    match &node.kind {
        Kind::Scalar(scalar) => writeln!(output, "    {};", store(*scalar, "offset", "value"))?,
        Kind::Unit | Kind::Array { length: 0, .. } => {}
        Kind::String => writeln!(
            output,
            "    const length = intrlay.utf8Length(value);\n    const payloadOffset = cursor.reserve(length, 1);\n    storage.u32[offset / 4] = payloadOffset - offset;\n    storage.u32[offset / 4 + 1] = length;\n    intrlay.encodeString(value, storage.bytes.subarray(payloadOffset, payloadOffset + length));"
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
        Kind::Enum(variants) => {
            writeln!(output, "    switch (value.kind) {{")?;
            for variant in variants {
                writeln!(
                    output,
                    "        case \"{}\":\n            storage.u32[offset / 4] = {};",
                    variant.name, variant.tag
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
            writeln!(output, "    }}")?;
        }
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
                "    const payloadOffset = cursor.reserve({}, {});\n    storage.u32[offset / 4] = payloadOffset - offset;\n    $[write:{value}](storage, payloadOffset, value, cursor);",
                child.layout.size(),
                child.layout.alignment()
            )?;
        }
    }
    writeln!(output, "}}")?;
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
            "    const length = intrlay.count(value);\n    const payloadOffset = cursor.reserve(length * {}, {});\n    storage.u32[offset / 4] = payloadOffset - offset;\n    storage.u32[offset / 4 + 1] = length;",
            element_node.layout.size(),
            element_node.layout.alignment()
        )?;
    } else {
        writeln!(output, "    const payloadOffset = offset;")?;
    }
    writeln!(
        output,
        "    if (typeof value !== \"number\") {{\n        for (let index = 0; index < value.length; ++index) {{\n            $[write:{element}](storage, payloadOffset + index * {}, value[index]!, cursor);\n        }}\n    }}",
        element_node.layout.size()
    )?;
    if matches!(node.kind, Kind::Array { .. }) && plan.zeroable(element) {
        writeln!(
            output,
            "    else {{\n        storage.u8.fill(0, offset, offset + {});\n    }}",
            node.layout.size()
        )?;
    }
    Ok(())
}

fn view(output: &mut String, plan: &TypeGraph, id: Id, node: &Node) -> Result<(), GenerateError> {
    let name = plan.view_name(id);
    writeln!(output)?;
    documentation::write(
        output,
        &crate::documentation::with_note(
            &node.docs,
            "Borrowed fixed-layout view. The caller retains the backing buffer.",
        ),
        "",
    )?;
    let (collection_fields, collection_init) = if matches!(node.kind, Kind::Sequence(_)) {
        (
            "\n    private readonly $[element_offset]: number;\n    private readonly $[element_count]: number;",
            "\n        this.$[element_offset] = offset + this.$[storage].u32[offset / 4]!;\n        this.$[element_count] = this.$[storage].u32[offset / 4 + 1]!;",
        )
    } else {
        ("", "")
    };
    writeln!(
        output,
        "export class {name} {{\n    public static readonly $[byte_size] = {};\n    public static readonly $[alignment] = {};\n    private readonly $[storage]: intrlay.Storage;\n    private readonly $[offset]: number;{collection_fields}\n\n    public constructor(bytes: globalThis.Uint8Array | intrlay.Storage, offset = 0) {{\n        this.$[storage] = bytes instanceof intrlay.Storage ? bytes : new intrlay.Storage(bytes);\n        this.$[offset] = offset;{collection_init}\n    }}\n\n    public get $[bytes](): globalThis.Uint8Array {{ return this.$[storage].bytes; }}",
        node.layout.size(),
        node.buffer_alignment
    )?;
    if node.options.generation.create {
        construction_methods(output, plan, id)?;
    }
    view_members(output, plan, id, node)?;
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
    public static $[compute_byte_length](value: {input}): number {{
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
                    "\n    public get $[tag](): number {{ return this.$[storage].u32[this.$[offset] / 4]!; }}"
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
            format!("if (this.$[storage].u32[this.$[offset] / 4] !== {tag}) {{\n    return;\n}}\n")
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
        "storage.{}[({offset}) / {}] = {value}",
        storage_name(scalar),
        scalar.layout().size()
    )
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
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            let count = if plan.zeroable(*element) {
                " | number"
            } else {
                ""
            };
            format!(
                "globalThis.ArrayLike<{}>{count}",
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
    let has_getter = node.options.generation.getters
        || (node.options.generation.setters && plan.borrows_content(element));
    if has_getter {
        writeln!(output)?;
        method(
            output,
            &format!(
                "public $[get](index: number): {}",
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
    if node.options.generation.setters
        && plan.nodes[element].options.generation.setters
        && let Some(mutation) = plan.mutation(element)
    {
        if !has_getter {
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
                &mutation,
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
                writeln!(output)?;
                method(
                    output,
                    &format!(
                        "public {}(index: number): {}",
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
            prefix.push_str("position += this.$[storage].u32[position / 4]!;\n");
        }
        let lane = format!(
            "this.$[storage].{}[position / {} + component]",
            storage_name(scalar),
            scalar.layout().size()
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
