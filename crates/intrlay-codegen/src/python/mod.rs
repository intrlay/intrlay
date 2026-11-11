use std::fmt::Write;

use intrlay_schema::{Exposure, Scalar};

use crate::{
    GenerateError,
    api::{Accessor, CollectionAccessors},
    plan::{Field, GenerationPlan, Id, Kind, Node, TypeGraph, WriterParameters},
};

mod api;
mod documentation;
mod field_helpers;
mod indexed;
mod load;
mod mutation;
mod name_validation;
mod names;
mod numeric_input;

pub(crate) use names::reference as reference_name;

pub const BACKEND: crate::backend::Backend = crate::backend::Backend {
    language: intrlay_schema::Language::Python,
    api: &api::Api,
    naming_policy: names::policy,
    reserved_declarations: &[
        "_intrlay",
        "Some",
        "_abc",
        "_typing",
        "_builtins",
        "_enum",
        "super",
    ],
    is_reserved_name: name_validation::is_reserved,
    generate,
    field_setter: crate::api::SetterSyntax::PropertyWithGetter,
    root_setter: crate::api::SetterSyntax::PropertyWithGetter,
    input_field_kind: crate::SymbolKind::Field,
    input_field_modifiers: &[],
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
    if plan
        .nodes
        .iter()
        .any(|node| matches!(node.kind, Kind::Enum(_)))
    {
        writeln!(output, "import enum as _enum")?;
    }
    // Plain enums have no declaration dependencies. Emit them before input
    // annotations so names such as int and len resolve to these types rather
    // than the builtins when a type checker visits an earlier parent record.
    for value_enums in [true, false] {
        for (id, node) in plan.nodes.iter().enumerate() {
            if node.is_value_enum() == value_enums {
                input(&mut output, plan, node, generation.encoding.needs_input(id))?;
            }
        }
    }
    for (id, node) in plan.nodes.iter().enumerate() {
        if generation.encoding.measures.contains(&id) {
            measure(&mut output, id, node, plan)?;
        }
        if generation.encoding.writers.contains(&id) {
            write(&mut output, id, node, plan)?;
        }
    }
    let view_plan = &generation.views.types;
    for &id in &generation.views.emitted {
        let node = &view_plan.nodes[id];
        view(&mut output, id, node, view_plan)?;
    }
    for (alias, original) in private_aliases(generation)? {
        writeln!(output, "\n{alias} = {original}")?;
    }
    for (name, id) in &plan.exports {
        if *name != plan.nodes[*id].name {
            if generation.views.emitted.contains(id) {
                writeln!(output, "{name}View = {}", plan.view_name(*id))?;
            }
            if matches!(plan.nodes[*id].kind, Kind::Enum(_)) && !plan.nodes[*id].is_value_enum() {
                writeln!(
                    output,
                    "{name}Kind = {}",
                    names::reference(&plan.nodes[*id].kind_name())
                )?;
            }
        }
        if generation.encoding.needs_input(*id)
            && (*name != plan.nodes[*id].name
                || !matches!(plan.nodes[*id].kind, Kind::Record(_) | Kind::Enum(_)))
        {
            if plan.nodes[*id].is_value_enum() {
                writeln!(
                    output,
                    "{name} = {}",
                    names::reference(&plan.nodes[*id].tag_type_name())
                )?;
            } else {
                writeln!(output, "type {name}Input = {}", input_type(plan, *id))?;
            }
        }
    }
    Ok(output)
}

/// Class-scope references to private types and module helpers need safe aliases.
pub(crate) fn private_aliases(
    generation: &GenerationPlan,
) -> Result<Vec<(String, String)>, GenerateError> {
    let plan = &generation.encoding.types;
    let views = &generation.views.types;
    let mut declarations = plan
        .nodes
        .iter()
        .enumerate()
        .filter(|(id, node)| {
            generation.encoding.needs_input(*id)
                && matches!(node.kind, Kind::Record(_) | Kind::Enum(_))
                && !node.is_enum_input_context()
        })
        .map(|(_, node)| node.input_name())
        .chain(
            generation
                .views
                .emitted
                .iter()
                .map(|&id| views.view_name(id)),
        )
        .collect::<Vec<_>>();
    declarations.extend(
        plan.nodes
            .iter()
            .filter(|node| matches!(node.kind, Kind::Enum(_)) && !node.is_value_enum())
            .map(Node::kind_name),
    );
    for (operation, helpers) in [
        ("measure", &generation.encoding.measures),
        ("write", &generation.encoding.writers),
    ] {
        for &id in helpers {
            declarations.push(crate::symbols::codec_function(plan, operation, id)?);
        }
    }
    Ok(declarations
        .into_iter()
        .filter_map(|name| {
            let alias = names::reference(&name);
            (alias != name).then_some((alias, name))
        })
        .collect())
}

fn input(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    needed: bool,
) -> Result<(), GenerateError> {
    if node.is_enum_input_context() {
        return Ok(());
    }
    if !needed && !matches!(node.kind, Kind::Enum(_)) {
        return Ok(());
    }
    if let Some(ty) = numeric_input::input_type(plan, node)
        && matches!(node.kind, Kind::Record(_))
    {
        writeln!(output, "\n\ntype {} = {ty}", node.input_name())?;
        documentation::write(output, &node.docs, "")?;
        return Ok(());
    }
    if let Some(field) = plan.folded_node(node) {
        writeln!(
            output,
            "\ntype {} = {}",
            node.input_name(),
            input_type(plan, field.ty)
        )?;
        documentation::write(output, &node.docs, "")?;
        return Ok(());
    }
    match &node.kind {
        Kind::Record(fields) => {
            input_record(output, plan, &node.input_name(), fields, None, &node.docs)?;
        }
        Kind::Enum(variants) => {
            let kind = node.tag_type_name();
            writeln!(output, "\n\nclass {kind}(_enum.IntEnum):")?;
            documentation::write(output, &node.docs, "    ")?;
            for variant in variants {
                writeln!(output, "    {} = {}", variant.name, variant.tag)?;
                documentation::write(output, &variant.docs, "    ")?;
            }
            if node.is_value_enum() || !needed {
                return Ok(());
            }
            for variant in variants {
                input_record(
                    output,
                    plan,
                    &node.variant_input_name(variant),
                    &variant.fields,
                    Some(&format!("{}.{}", names::reference(&kind), variant.name)),
                    &variant.docs,
                )?;
            }
            let variants = if variants.is_empty() {
                "_typing.Never".into()
            } else {
                variants
                    .iter()
                    .map(|variant| node.variant_input_name(variant))
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
            writeln!(output, "\n\ntype {} = {variants}", node.input_name())?;
            documentation::write(output, &node.docs, "")?;
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
    variant: Option<&str>,
    docs: &str,
) -> Result<(), GenerateError> {
    if fields
        .iter()
        .any(|field| names::reference(&field.name) != field.name)
    {
        // Functional TypedDict declarations preserve private dictionary keys.
        // String annotations also retain forward and recursive references.
        writeln!(output, "\n\n{name} = _typing.TypedDict('{name}', {{")?;
        if let Some(variant) = variant {
            writeln!(output, "    'kind': \"_typing.Literal[{variant}]\",")?;
        }
        for field in fields {
            for line in crate::documentation::escape_symbols(&field.docs).lines() {
                writeln!(output, "    # {line}")?;
            }
            let ty = serde_json::to_string(&input_type(plan, field.ty))
                .map_err(|error| GenerateError(error.to_string()))?;
            writeln!(output, "    '{}': {ty},", field.name)?;
        }
        writeln!(output, "}})")?;
        documentation::write(output, docs, "")?;
        return Ok(());
    }
    writeln!(output, "\n\nclass {name}(_typing.TypedDict):")?;
    documentation::write(output, docs, "    ")?;
    if let Some(variant) = variant {
        writeln!(output, "    kind: _typing.Literal[{variant}]")?;
    } else if fields.is_empty() && docs.is_empty() {
        writeln!(output, "    pass")?;
    }
    for field in fields {
        writeln!(output, "    {}: {}", field.name, input_type(plan, field.ty))?;
        documentation::write(output, &field.docs, "    ")?;
    }
    Ok(())
}

fn measure(
    output: &mut String,
    id: Id,
    node: &Node,
    plan: &TypeGraph,
) -> Result<(), GenerateError> {
    if !plan.has_payload(id) {
        return Ok(());
    }
    let value = plan.measure_parameter(id);
    writeln!(
        output,
        "\n\ndef $[measure:{id}]({value}: {}, cursor: _intrlay.Cursor) -> None:",
        input_type(plan, id)
    )?;
    if let Some(numeric) = node.numeric_input {
        return numeric_input::measure(output, plan, node, numeric);
    }
    match &node.kind {
        Kind::String => writeln!(
            output,
            "    cursor.reserve(_builtins.len(value.encode('utf-8')), 1)"
        )?,
        Kind::Record(fields) => {
            for field in fields.iter().filter(|field| plan.has_payload(field.ty)) {
                writeln!(
                    output,
                    "    $[measure:{}]({}, cursor)",
                    field.ty,
                    input_member(plan, node, field)
                )?;
            }
        }
        Kind::Enum(variants) => {
            for (index, variant) in variants
                .iter()
                .filter(|variant| {
                    variant
                        .fields
                        .iter()
                        .any(|field| plan.has_payload(field.ty))
                })
                .enumerate()
            {
                let branch = if index == 0 { "if" } else { "elif" };
                writeln!(
                    output,
                    "    {branch} value['kind'] == {}.{}:",
                    names::reference(&node.kind_name()),
                    variant.name
                )?;
                for field in variant
                    .fields
                    .iter()
                    .filter(|field| plan.has_payload(field.ty))
                {
                    writeln!(
                        output,
                        "        $[measure:{}](value['{}'], cursor)",
                        field.ty, field.name
                    )?;
                }
            }
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            if matches!(node.kind, Kind::Sequence(_)) {
                writeln!(
                    output,
                    "    cursor.reserve(_intrlay.count(value) * {}, {})",
                    plan.nodes[*element].layout.size(),
                    plan.nodes[*element].layout.alignment()
                )?;
            }
            if plan.has_payload(*element) {
                writeln!(
                    output,
                    "    if not _builtins.isinstance(value, _builtins.int):\n        for item in value:\n            $[measure:{element}](item, cursor)"
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
                "    if value is not None:\n        $[measure:{value}]({payload}, cursor)"
            )?;
        }
        Kind::Boxed(value) => {
            writeln!(
                output,
                "    cursor.reserve({}, {})",
                plan.nodes[*value].layout.size(),
                plan.nodes[*value].layout.alignment()
            )?;
            if plan.has_payload(*value) {
                writeln!(output, "    $[measure:{value}](value, cursor)")?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn write(output: &mut String, id: Id, node: &Node, plan: &TypeGraph) -> Result<(), GenerateError> {
    let optional = if plan.has_payload(id) { "" } else { " | None" };
    let WriterParameters {
        storage,
        offset,
        value,
        cursor,
    } = plan.writer_parameters(id);
    writeln!(
        output,
        "\n\ndef $[write:{id}](\n    {storage}: _intrlay.Storage, {offset}: _builtins.int, {value}: {}, {cursor}: _intrlay.Cursor{optional}\n) -> None:",
        input_type(plan, id)
    )?;
    if let Some(numeric) = node.numeric_input {
        return numeric_input::write(output, plan, node, numeric);
    }
    if let Kind::Array { length, .. } = node.kind {
        if plan.zeroable(id) {
            writeln!(
                output,
                "    if _builtins.isinstance(value, _intrlay.Zeroed):\n        storage.u8[offset:offset + {}] = _builtins.bytes({})\n        return",
                node.layout.size(),
                node.layout.size()
            )?;
        }
        writeln!(
            output,
            "    if _builtins.len(value) != {length}:\n        raise _builtins.ValueError('fixed array length')"
        )?;
    }
    match &node.kind {
        Kind::Scalar(scalar) => writeln!(
            output,
            "    {} = value",
            scalar_access(*scalar, "storage", "offset")
        )?,
        Kind::Array { length: 0, .. } => {}
        Kind::String => writeln!(
            output,
            "    data = value.encode('utf-8')\n    payload_offset = cursor.reserve(_builtins.len(data), 1)\n    storage.u32[offset // 4] = payload_offset - offset\n    storage.u32[offset // 4 + 1] = _builtins.len(data)\n    storage.bytes[payload_offset:payload_offset + _builtins.len(data)] = data"
        )?,
        Kind::Record(fields) => {
            if fields.is_empty() {
                writeln!(output, "    pass")?;
            }
            for field in fields {
                writeln!(
                    output,
                    "    $[write:{}](storage, offset + {}, {}, cursor)",
                    field.ty,
                    field.offset,
                    input_member(plan, node, field)
                )?;
            }
        }
        Kind::Enum(variants) if node.is_value_enum() => {
            writeln!(output, "    if (")?;
            for (index, variant) in variants.iter().enumerate() {
                let conjunction = if index == 0 { "" } else { "and " };
                writeln!(
                    output,
                    "        {conjunction}value != {}.{}",
                    names::reference(&node.input_name()),
                    variant.name
                )?;
            }
            writeln!(
                output,
                "    ):\n        raise _builtins.ValueError('Unknown enum tag.')"
            )?;
            writeln!(output, "    storage.u32[offset // 4] = value")?;
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
                "    storage.u8[offset] = 0 if value is None else 1\n    if value is not None:\n        $[write:{value}](storage, offset + {offset}, {payload}, cursor)"
            )?;
        }
        Kind::Boxed(value) => writeln!(
            output,
            "    payload_offset = cursor.reserve({}, {})\n    storage.u32[offset // 4] = payload_offset - offset\n    $[write:{value}](storage, payload_offset, value, cursor)",
            plan.nodes[*value].layout.size(),
            plan.nodes[*value].layout.alignment()
        )?,
        Kind::Unit => writeln!(output, "    pass")?,
    }
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
            "    length = _intrlay.count(value)\n    payload_offset = cursor.reserve(length * {}, {})\n    storage.u32[offset // 4] = payload_offset - offset\n    storage.u32[offset // 4 + 1] = length",
            element_node.layout.size(),
            element_node.layout.alignment()
        )?;
    } else {
        writeln!(output, "    payload_offset = offset")?;
    }
    writeln!(
        output,
        "    if not _builtins.isinstance(value, _builtins.int):"
    )?;
    // CPython 3.13: dispatch has no consistent win for 1–2 lanes.
    if let Kind::Scalar(scalar) = element_node.kind
        && scalar != Scalar::Bool
        && !matches!(node.kind, Kind::Array { length, .. } if length <= 2)
    {
        writeln!(
            output,
            "        if _intrlay.try_copy_numeric_array(storage.{}, payload_offset // {}, value):\n            return",
            format!("{scalar:?}").to_lowercase(),
            scalar.layout().size(),
        )?;
    }
    writeln!(
        output,
        "        for index, item in _builtins.enumerate(value):\n            $[write:{element}](storage, payload_offset + index * {}, item, cursor)",
        element_node.layout.size()
    )?;
    Ok(())
}

fn write_enum(
    output: &mut String,
    node: &Node,
    variants: &[crate::plan::Variant],
) -> Result<(), GenerateError> {
    for (index, variant) in variants.iter().enumerate() {
        let branch = if index == 0 { "if" } else { "elif" };
        writeln!(
            output,
            "    {branch} value['kind'] == {}.{}:\n        storage.u32[offset // 4] = {}",
            names::reference(&node.kind_name()),
            variant.name,
            variant.tag
        )?;
        for field in &variant.fields {
            writeln!(
                output,
                "        $[write:{}](storage, offset + {}, value['{}'], cursor)",
                field.ty, field.offset, field.name
            )?;
        }
    }
    writeln!(
        output,
        "    else:\n        raise _builtins.ValueError('Unknown enum tag.')"
    )?;
    Ok(())
}

fn view(output: &mut String, id: Id, node: &Node, plan: &TypeGraph) -> Result<(), GenerateError> {
    let name = plan.view_name(id);
    let mut members = String::new();
    let descriptors = view_members(&mut members, id, node, plan)?;
    let base = if descriptors || members.contains("self._offset") {
        "View"
    } else {
        "BufferView"
    };
    writeln!(output, "\n\nclass {name}(_intrlay.{base}):")?;
    documentation::write(output, &node.docs, "    ")?;
    let fields: Vec<_> = match &node.kind {
        Kind::Record(fields) => fields.iter().map(|field| (field, None)).collect(),
        Kind::Enum(variants) => variants
            .iter()
            .flat_map(|variant| {
                variant
                    .fields
                    .iter()
                    .map(move |field| (field, Some(variant)))
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut cached: Vec<_> = fields
        .into_iter()
        .filter_map(|(field, variant)| {
            let api = Accessor::field(plan, node, field, variant);
            api.cache.map(|cache| {
                let ty = if variant.is_some() {
                    optional_type(plan, field.ty, false)
                } else {
                    projected_type(plan, field.ty, field.options.exposure)
                };
                (cache, ty)
            })
        })
        .collect();
    if matches!(node.kind, Kind::Optional { .. } | Kind::Boxed(_))
        && let Some(cache) = Accessor::root(plan, id)?.cache
    {
        cached.push((cache, output_type(plan, id)));
    }
    view_storage(output, node, &cached, &members)?;
    if node.options.generation.create {
        let input = input_type(plan, id);
        let compute_size = compute_size(plan, id)?;
        let unused_value = if plan.has_payload(id) {
            ""
        } else {
            "  # noqa: ARG004 - Keep the public value keyword for fixed-size inputs."
        };
        let writer = names::reference(&crate::symbols::codec_function(plan, "write", id)?);
        let check_extent = if plan.has_payload(id) {
            ", check_extent=True"
        } else {
            ""
        };
        writeln!(
            output,
            r"
    @_builtins.staticmethod
    def $[compute_byte_length](value: {input}) -> _builtins.int:{unused_value}
        {compute_size}

    @_builtins.classmethod
    def $[create](cls, value: {input}) -> _typing.Self:
        size = cls.$[compute_byte_length](value)
        return _intrlay.initialize(
            cls, _intrlay.allocate(size, cls.$[alignment]), value, size,
            cls.$[byte_size], cls.$[alignment], {writer}{check_extent}
        )

    @_builtins.classmethod
    def $[create_in_place](cls, buffer: _abc.Buffer, value: {input}) -> _typing.Self:
        return _intrlay.initialize(
            cls, buffer, value, cls.$[compute_byte_length](value),
            cls.$[byte_size], cls.$[alignment], {writer}{check_extent}
        )"
        )?;
    }
    output.push_str(&members);
    Ok(())
}

fn view_storage(
    output: &mut String,
    node: &Node,
    cached: &[(crate::plan::ViewCache, String)],
    members: &str,
) -> Result<(), GenerateError> {
    let mut slots = cached
        .iter()
        .flat_map(|(cache, _)| std::iter::once(&cache.value).chain(cache.state.iter()))
        .map(|name| format!("\"{name}\","))
        .collect::<Vec<_>>()
        .join(" ");
    let collection_fields: Vec<_> = ["element_offset", "element_count"]
        .into_iter()
        .filter(|key| members.contains(&format!("$[{key}]")))
        .collect();
    for key in &collection_fields {
        write!(slots, " \"$[{key}]\",")?;
    }
    writeln!(
        output,
        "    __slots__ = ({slots})\n    $[byte_size]: _typing.ClassVar[_builtins.int] = {}\n    $[alignment]: _typing.ClassVar[_builtins.int] = {}",
        node.layout.size(),
        node.buffer_alignment
    )?;
    for key in &collection_fields {
        writeln!(output, "    $[{key}]: _builtins.int")?;
    }
    if !collection_fields.is_empty() || !cached.is_empty() {
        for (cache, ty) in cached {
            writeln!(output, "    {}: {ty} | _intrlay.CacheState", cache.value)?;
            if let Some(state) = &cache.state {
                writeln!(output, "    {state}: _builtins.int")?;
            }
        }
        writeln!(
            output,
            "\n    def __init__(self, buffer: _abc.Buffer | _intrlay.Storage, offset: _builtins.int = 0) -> None:\n        super().__init__(buffer, offset)"
        )?;
        if collection_fields.contains(&"element_offset") {
            writeln!(
                output,
                "        self.$[element_offset] = offset + self._storage.u32[offset // 4]"
            )?;
        }
        if collection_fields.contains(&"element_count") {
            writeln!(
                output,
                "        self.$[element_count] = self._storage.u32[offset // 4 + 1]"
            )?;
        }
        for (cache, _) in cached {
            writeln!(
                output,
                "        self.{} = _intrlay.CacheState.UNINITIALIZED",
                cache.value
            )?;
            if let Some(state) = &cache.state {
                writeln!(output, "        self.{state} = -1")?;
            }
        }
    }
    // Keep runtime descriptors on the direct private slot. The public property
    // follows naming policy without adding a property lookup to field reads.
    writeln!(
        output,
        "\n    @_builtins.property\n    def $[storage](self) -> _intrlay.Storage:\n        \"\"\"Backing bytes and numeric projections shared by this view.\"\"\"\n        return self._storage"
    )?;
    Ok(())
}

fn view_members(
    output: &mut String,
    id: Id,
    node: &Node,
    plan: &TypeGraph,
) -> Result<bool, GenerateError> {
    // Runtime property descriptors read the record offset indirectly.
    let mut descriptors = false;
    if node.options.generation.setters && node.options.values() && !plan.has_payload(id) {
        let writer = names::reference(&crate::symbols::codec_function(plan, "write", node.codec)?);
        writeln!(
            output,
            "\n    def $[replace](self, value: {}) -> None:\n        \"\"\"Replace the inline value without reallocating its storage.\"\"\"\n        {writer}(self._storage, self._offset, value, None)",
            input_type(plan, id)
        )?;
    }
    match &node.kind {
        Kind::Record(fields) => {
            for field in fields {
                let api = Accessor::field(plan, node, field, None);
                descriptors |= property(output, plan, field, &api, None)?;
            }
        }
        Kind::Enum(variants) => {
            if node.options.generation.getters {
                writeln!(
                    output,
                    "\n    @_builtins.property\n    def $[tag](self) -> _builtins.int:\n        return self._storage.u32[self._offset // 4]"
                )?;
            }
            for variant in variants {
                for field in &variant.fields {
                    let api = Accessor::field(plan, node, field, Some(variant));
                    descriptors |= property(output, plan, field, &api, Some(variant.tag))?;
                }
            }
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            sequence_view(output, plan, node, *element)?;
        }
        _ => {
            let api = Accessor::root(plan, id)?;
            if api.getter {
                let name = &api.name;
                let read = load::Read {
                    id,
                    offset: "self._offset",
                    exposure: node.options.exposure,
                    tag: None,
                };
                writeln!(
                    output,
                    "\n    @_builtins.property\n    def {name}(self) -> {}:",
                    output_type(plan, id)
                )?;
                writeln!(
                    output,
                    "        {}",
                    load::body(plan, &read, api.cache.as_ref()).replace('\n', "\n        ")
                )?;
            }
            if let Some(setter) = &api.setter {
                let mutation = &setter.mutation;
                if !api.getter {
                    writeln!(output)?;
                }
                if setter.property {
                    writeln!(output, "    @$[value].setter")?;
                }
                let setter = &setter.name;
                writeln!(
                    output,
                    "    def {setter}(self, value: {}) -> None:\n        {}",
                    input_type(plan, mutation.value),
                    mutation::body(plan, mutation, "self._offset")?
                )?;
            }
        }
    }
    Ok(descriptors)
}

fn sequence_view(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    element: Id,
) -> Result<(), GenerateError> {
    let (position, count) = if let Kind::Array { length, .. } = node.kind {
        ("self._offset".to_owned(), length.to_string())
    } else {
        (
            "self.$[element_offset]".into(),
            "self.$[element_count]".into(),
        )
    };
    writeln!(
        output,
        "\n    def __len__(self) -> _builtins.int:\n        return {count}"
    )?;
    let api = CollectionAccessors::new(plan, node, element);
    if api.getter {
        writeln!(
            output,
            "\n    def __getitem__(self, index: _builtins.int) -> {}:\n        if index < 0 or index >= _builtins.len(self):\n            raise _builtins.IndexError(index)\n        {}",
            output_type(plan, element),
            load::body(
                plan,
                &load::Read {
                    id: element,
                    offset: &format!(
                        "({position}) + index * {}",
                        plan.nodes[element].layout.size()
                    ),
                    exposure: plan.nodes[element].options.exposure,
                    tag: None
                },
                None
            )
            .replace('\n', "\n        ")
        )?;
    }
    if let Some(mutation) = &api.setter {
        if !api.getter {
            writeln!(output)?;
        }
        writeln!(
            output,
            "    def __setitem__(self, index: _builtins.int, value: {}) -> None:\n        if index < 0 or index >= _builtins.len(self):\n            raise _builtins.IndexError(index)\n        {}",
            input_type(plan, mutation.value),
            mutation::body(
                plan,
                mutation,
                &format!(
                    "({position}) + index * {}",
                    plan.nodes[element].layout.size()
                )
            )?
        )?;
    }
    if api.getter {
        writeln!(
            output,
            "\n    def __iter__(self) -> _abc.Iterator[{}]:\n        for index in _builtins.range(_builtins.len(self)):\n            yield self[index]",
            output_type(plan, element)
        )?;
    }
    indexed::fields(output, plan, node, element, &position)
}

fn property(
    output: &mut String,
    plan: &TypeGraph,
    field: &Field,
    api: &Accessor,
    tag: Option<u32>,
) -> Result<bool, GenerateError> {
    let offset = format!("self._offset + {}", field.offset);
    field_helpers::emit(
        output,
        plan,
        &api.helpers,
        field,
        &offset,
        tag.map(|tag| (tag, "self._offset")),
        false,
    )?;
    value_property(output, plan, field, api, tag)
}

fn value_property(
    output: &mut String,
    plan: &TypeGraph,
    field: &Field,
    api: &Accessor,
    tag: Option<u32>,
) -> Result<bool, GenerateError> {
    let generation = api.generation;
    let name = &api.name;
    let offset = format!("self._offset + {}", field.offset);
    if generation.getters && tag.is_none() {
        let ty = projected_type(plan, field.ty, field.options.exposure);
        let doc = if field.docs.is_empty() {
            String::new()
        } else {
            let text = crate::documentation::escape_symbols(&field.docs);
            let literal =
                serde_json::to_string(&text).map_err(|error| GenerateError(error.to_string()))?;
            format!(", doc={literal}")
        };
        if let Some(scalar) = plan.scalar_view(field.ty)
            && ty == scalar_type(scalar)
            && generation.setters
            && plan.mutation(field.ty).is_some()
        {
            let factory = format!("{scalar:?}_field").to_lowercase();
            let value = format!("_intrlay.{factory}({}{doc})", field.offset);
            writeln!(output, "\n    {name} = {value}")?;
            return Ok(true);
        }
        // Cached composite getters keep direct slot access on the hot path.
        if !plan.caches_view(field.ty)
            && (!generation.setters || plan.mutation(field.ty).is_none())
            && let Some(value) = load::expression(plan, field.ty, &offset, field.options.exposure)
                .filter(|value| value.contains("self._offset"))
        {
            writeln!(
                output,
                "\n    {name}: _intrlay.ReadField[{ty}] = _intrlay.read_field(\n        lambda self: {value}{doc}\n    )"
            )?;
            return Ok(true);
        }
    }
    let has_getter = api.getter;
    if has_getter {
        let ty = if tag.is_some() {
            if let Exposure::TypedArray(scalar) = field.options.exposure {
                format!("{} | None", numeric_type(scalar))
            } else {
                optional_type(plan, field.ty, false)
            }
        } else {
            projected_type(plan, field.ty, field.options.exposure)
        };
        writeln!(
            output,
            "\n    @_builtins.property\n    def {name}(self) -> {ty}:"
        )?;
        documentation::write(output, &field.docs, "        ")?;
        let read = load::Read {
            id: field.ty,
            offset: &offset,
            exposure: field.options.exposure,
            tag: tag.map(|tag| (tag, "self._offset")),
        };
        writeln!(
            output,
            "        {}",
            load::body(plan, &read, api.cache.as_ref()).replace('\n', "\n        ")
        )?;
    }
    if let Some(setter) = &api.setter {
        let mutation = &setter.mutation;
        if !has_getter {
            writeln!(output)?;
        }
        let setter = if setter.property {
            writeln!(output, "    @{name}.setter")?;
            name.clone()
        } else {
            setter.name.clone()
        };
        writeln!(
            output,
            "    def {setter}(self, value: {}) -> None:",
            input_type(plan, mutation.value)
        )?;
        documentation::write(output, &field.docs, "        ")?;
        if let Some(tag) = tag {
            writeln!(
                output,
                "        if self._storage.u32[self._offset // 4] != {tag}:\n            return"
            )?;
        }
        writeln!(
            output,
            "        {}",
            mutation::body(plan, mutation, &offset)?
        )?;
    }
    Ok(false)
}

fn input_type(plan: &TypeGraph, id: Id) -> String {
    if let Some(ty) = numeric_input::input_type(plan, &plan.nodes[id]) {
        return ty;
    }
    match &plan.nodes[id].kind {
        Kind::Scalar(scalar) => scalar_type(*scalar).into(),
        Kind::Unit => "None".into(),
        Kind::String => "_builtins.str".into(),
        Kind::Record(_) | Kind::Enum(_) => names::reference(&plan.input_name(id)),
        Kind::Sequence(element) => {
            let count = if plan.zeroable(*element) {
                " | _intrlay.ZeroedSequence"
            } else {
                ""
            };
            format!("_abc.Sequence[{}]{count}", input_type(plan, *element))
        }
        Kind::Array { element, .. } => {
            let zeroed = if plan.zeroable(id) {
                " | _intrlay.Zeroed"
            } else {
                ""
            };
            format!("_abc.Sequence[{}]{zeroed}", input_type(plan, *element))
        }
        Kind::Optional { value, .. } => optional_type(plan, *value, true),
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
        Kind::Enum(_) if plan.nodes[id].is_value_enum() => {
            names::reference(&plan.nodes[id].tag_type_name())
        }
        Kind::Unit => "None".into(),
        Kind::String => "_builtins.str".into(),
        Kind::Optional { value, .. } => optional_type(plan, *value, false),
        Kind::Boxed(value) => output_type(plan, *value),
        _ => names::reference(&plan.view_name(id)),
    }
}

fn optional_type(plan: &TypeGraph, value: Id, input: bool) -> String {
    let payload = if input {
        input_type(plan, value)
    } else {
        output_type(plan, value)
    };
    if plan.nullable_value(value) {
        format!("_intrlay.Some[{payload}] | None")
    } else {
        format!("{payload} | None")
    }
}

fn scalar_type(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Bool => "_builtins.bool",
        Scalar::F32 | Scalar::F64 => "_builtins.float",
        _ => "_builtins.int",
    }
}

fn scalar_access(scalar: Scalar, storage: &str, offset: &str) -> String {
    let array = match scalar {
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
    };
    if scalar.layout().size() == 1 {
        format!("{storage}.{array}[{offset}]")
    } else {
        format!(
            "{storage}.{array}[({offset}) // {}]",
            scalar.layout().size()
        )
    }
}

fn compute_size(plan: &TypeGraph, id: Id) -> Result<String, GenerateError> {
    let size = plan.nodes[id].layout.size();
    if !plan.has_payload(id) {
        return Ok(format!("return {size}"));
    }
    let measure = names::reference(&crate::symbols::codec_function(plan, "measure", id)?);
    Ok(format!(
        "cursor = _intrlay.Cursor({size})\n        {measure}(value, cursor)\n        return cursor.end"
    ))
}

fn projected_type(plan: &TypeGraph, id: Id, exposure: Exposure) -> String {
    if let Exposure::TypedArray(scalar) = exposure {
        numeric_type(scalar)
    } else if matches!(plan.nodes[id].options.exposure, Exposure::TypedArray(_)) {
        names::reference(&if plan.nodes[id].is_value_enum() {
            plan.nodes[id].tag_type_name()
        } else {
            plan.view_name(id)
        })
    } else {
        output_type(plan, id)
    }
}

fn numeric_type(scalar: Scalar) -> String {
    if matches!(scalar, Scalar::F32 | Scalar::F64) {
        "_intrlay.FloatView"
    } else {
        "_intrlay.IntView"
    }
    .into()
}

fn input_member(plan: &TypeGraph, node: &Node, field: &Field) -> String {
    if plan.folded_node(node).is_some() {
        "value".into()
    } else {
        format!("value['{}']", field.name)
    }
}
