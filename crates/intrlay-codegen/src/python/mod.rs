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

pub(crate) use names::reference as reference_name;

pub const BACKEND: crate::backend::Backend = crate::backend::Backend {
    language: intrlay_schema::Language::Python,
    naming_policy: names::policy,
    keywords: names::KEYWORDS,
    reserved_declarations: &["_intrlay", "Some", "_abc", "_typing", "_builtins", "super"],
    generate,
    field_setter: crate::api::SetterSyntax::PropertyWithGetter,
    root_setter: crate::api::SetterSyntax::PropertyWithGetter,
    input_field_kind: crate::SymbolKind::Field,
    input_field_modifiers: &[],
    codec_function_kind: crate::SymbolKind::Function,
    codec_function_modifiers: &[],
    fixed_array_api: crate::backend::FixedArrayApi::Numeric,
    native_numeric_array_views: true,
};

pub fn generate(generation: &GenerationPlan) -> Result<String, GenerateError> {
    let plan = &generation.encoding.types;
    let mut output = String::from(include_str!("prelude.txt"));
    let encoders = &generation.encoding.encoders;
    for node in &plan.nodes {
        input(&mut output, plan, node)?;
    }
    for (id, node) in plan.nodes.iter().enumerate() {
        if encoders.contains(&id) {
            measure(&mut output, id, node, plan)?;
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
            writeln!(output, "{name}View = {}", plan.view_name(*id))?;
        }
        if *name != plan.nodes[*id].name
            || !matches!(plan.nodes[*id].kind, Kind::Record(_) | Kind::Enum(_))
        {
            writeln!(output, "type {name} = {}", input_type(plan, *id))?;
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
        .filter(|node| matches!(node.kind, Kind::Record(_) | Kind::Enum(_)))
        .map(|node| node.name.clone())
        .chain(
            generation
                .views
                .emitted
                .iter()
                .map(|&id| views.view_name(id)),
        )
        .collect::<Vec<_>>();
    for &id in &generation.encoding.encoders {
        for operation in ["measure", "write"] {
            if operation == "write" || plan.has_payload(id) {
                declarations.push(crate::symbols::codec_function(plan, operation, id)?);
            }
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

fn input(output: &mut String, plan: &TypeGraph, node: &Node) -> Result<(), GenerateError> {
    if let Some(field) = plan.folded_node(node) {
        writeln!(
            output,
            "\ntype {} = {}",
            node.name,
            input_type(plan, field.ty)
        )?;
        documentation::write(output, &node.docs, "")?;
        return Ok(());
    }
    match &node.kind {
        Kind::Record(fields) => input_record(output, plan, &node.name, fields, None, &node.docs)?,
        Kind::Enum(variants) => {
            for variant in variants {
                input_record(
                    output,
                    plan,
                    &format!("{}{}", node.name, variant.name),
                    &variant.fields,
                    Some(&variant.name),
                    &variant.docs,
                )?;
            }
            let variants = if variants.is_empty() {
                "_typing.Never".into()
            } else {
                variants
                    .iter()
                    .map(|variant| format!("{}{}", node.name, variant.name))
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
            writeln!(output, "\n\ntype {} = {variants}", node.name)?;
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
            writeln!(output, "    'kind': \"_typing.Literal['{variant}']\",")?;
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
        writeln!(output, "    kind: _typing.Literal['{variant}']")?;
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
    writeln!(
        output,
        "\n\ndef $[measure:{id}](value: {}, cursor: _intrlay.Cursor) -> None:",
        input_type(plan, id)
    )?;
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
                writeln!(output, "    {branch} value['kind'] == '{}':", variant.name)?;
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
    writeln!(
        output,
        "\n\ndef $[write:{id}](\n    storage: _intrlay.Storage, offset: _builtins.int, value: {}, cursor: _intrlay.Cursor{optional}\n) -> None:",
        input_type(plan, id)
    )?;
    if let Kind::Array { length, .. } = node.kind {
        writeln!(
            output,
            "    if _intrlay.count(value) != {length}:\n        raise _builtins.ValueError('fixed array length')"
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
        Kind::Enum(variants) => {
            for (index, variant) in variants.iter().enumerate() {
                let branch = if index == 0 { "if" } else { "elif" };
                writeln!(
                    output,
                    "    {branch} value['kind'] == '{}':\n        storage.u32[offset // 4] = {}",
                    variant.name, variant.tag
                )?;
                for field in &variant.fields {
                    writeln!(
                        output,
                        "        $[write:{}](storage, offset + {}, value['{}'], cursor)",
                        field.ty, field.offset, field.name
                    )?;
                }
            }
        }
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            if matches!(node.kind, Kind::Sequence(_)) {
                writeln!(
                    output,
                    "    length = _intrlay.count(value)\n    payload_offset = cursor.reserve(length * {}, {})\n    storage.u32[offset // 4] = payload_offset - offset\n    storage.u32[offset // 4 + 1] = length",
                    plan.nodes[*element].layout.size(),
                    plan.nodes[*element].layout.alignment()
                )?;
            } else {
                writeln!(output, "    payload_offset = offset")?;
            }
            writeln!(
                output,
                "    if not _builtins.isinstance(value, _builtins.int):\n        for index, item in _builtins.enumerate(value):\n            $[write:{element}](storage, payload_offset + index * {}, item, cursor)",
                plan.nodes[*element].layout.size()
            )?;
            if matches!(node.kind, Kind::Array { .. }) && plan.zeroable(*element) {
                writeln!(
                    output,
                    "    else:\n        storage.u8[offset:offset + {}] = _builtins.bytes({})",
                    node.layout.size(),
                    node.layout.size()
                )?;
            }
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

fn view(output: &mut String, id: Id, node: &Node, plan: &TypeGraph) -> Result<(), GenerateError> {
    let name = plan.view_name(id);
    writeln!(output, "\n\nclass {name}(_intrlay.View):")?;
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
    view_storage(output, node, &cached)?;
    if node.options.generation.create {
        let input = input_type(plan, id);
        let compute_size = compute_size(plan, id)?;
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
    def $[compute_byte_length](value: {input}) -> _builtins.int:
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
    view_members(output, id, node, plan)?;
    Ok(())
}

fn view_storage(
    output: &mut String,
    node: &Node,
    cached: &[(crate::plan::ViewCache, String)],
) -> Result<(), GenerateError> {
    let mut slots = cached
        .iter()
        .flat_map(|(cache, _)| std::iter::once(&cache.value).chain(cache.state.iter()))
        .map(|name| format!("\"{name}\","))
        .collect::<Vec<_>>()
        .join(" ");
    let sequence = matches!(node.kind, Kind::Sequence(_));
    if sequence {
        slots.push_str(" \"$[element_offset]\", \"$[element_count]\",");
    }
    writeln!(
        output,
        "    __slots__ = ({slots})\n    $[byte_size]: _typing.ClassVar[_builtins.int] = {}\n    $[alignment]: _typing.ClassVar[_builtins.int] = {}",
        node.layout.size(),
        node.buffer_alignment
    )?;
    if sequence {
        writeln!(
            output,
            "    $[element_offset]: _builtins.int\n    $[element_count]: _builtins.int"
        )?;
    }
    if sequence || !cached.is_empty() {
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
        if sequence {
            writeln!(
                output,
                "        self.$[element_offset] = offset + self._storage.u32[offset // 4]\n        self.$[element_count] = self._storage.u32[offset // 4 + 1]"
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
    Ok(())
}

fn view_members(
    output: &mut String,
    id: Id,
    node: &Node,
    plan: &TypeGraph,
) -> Result<(), GenerateError> {
    if node.options.generation.setters && !plan.has_payload(id) {
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
                property(output, plan, field, &api, None)?;
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
    Ok(())
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
    let has_getter = node.options.generation.getters
        || (node.options.generation.setters && plan.borrows_content(element));
    if has_getter {
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
    if node.options.generation.setters
        && plan.nodes[element].options.generation.setters
        && let Some(mutation) = plan.mutation(element)
    {
        if !has_getter {
            writeln!(output)?;
        }
        writeln!(
            output,
            "    def __setitem__(self, index: _builtins.int, value: {}) -> None:\n        if index < 0 or index >= _builtins.len(self):\n            raise _builtins.IndexError(index)\n        {}",
            input_type(plan, mutation.value),
            mutation::body(
                plan,
                &mutation,
                &format!(
                    "({position}) + index * {}",
                    plan.nodes[element].layout.size()
                )
            )?
        )?;
    }
    if has_getter {
        writeln!(
            output,
            "\n    def __iter__(self) -> _abc.Iterator[{}]:\n        for index in _builtins.range(_builtins.len(self)):\n            yield self[index]",
            output_type(plan, element)
        )?;
    }
    indexed_fields(output, plan, node, element, &position)
}

fn indexed_fields(
    output: &mut String,
    plan: &TypeGraph,
    node: &Node,
    element: Id,
    position: &str,
) -> Result<(), GenerateError> {
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
                writeln!(
                    output,
                    "\n    def {}(self, index: _builtins.int) -> {}:",
                    getter,
                    output_type(plan, field.ty),
                )?;
                documentation::write(
                    output,
                    &crate::documentation::indexed(&field.docs),
                    "        ",
                )?;
                writeln!(
                    output,
                    "        {}",
                    load::body(
                        plan,
                        &load::Read {
                            id: field.ty,
                            offset: &offset,
                            exposure: field.options.exposure,
                            tag: None
                        },
                        None
                    )
                    .replace('\n', "\n        ")
                )?;
            }
            if let Some(setter) = &api.setter {
                let mutation = &setter.mutation;
                if api.getter.is_none() {
                    writeln!(output)?;
                }
                writeln!(
                    output,
                    "    def {}(self, index: _builtins.int, value: {}) -> None:",
                    setter.name,
                    input_type(plan, mutation.value),
                )?;
                documentation::write(
                    output,
                    &crate::documentation::indexed(&field.docs),
                    "        ",
                )?;
                writeln!(
                    output,
                    "        {}",
                    mutation::body(plan, mutation, &offset)?
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
            return Ok(());
        }
        // Cached composite getters keep direct slot access on the hot path.
        if !plan.caches_view(field.ty)
            && (!generation.setters || plan.mutation(field.ty).is_none())
            && let Some(value) = load::expression(plan, field.ty, &offset, field.options.exposure)
        {
            writeln!(
                output,
                "\n    {name}: _intrlay.ReadField[{ty}] = _intrlay.read_field(\n        lambda self: {value}{doc}\n    )"
            )?;
            return Ok(());
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
            tag,
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
    Ok(())
}

fn input_type(plan: &TypeGraph, id: Id) -> String {
    match &plan.nodes[id].kind {
        Kind::Scalar(scalar) => scalar_type(*scalar).into(),
        Kind::Unit => "None".into(),
        Kind::String => "_builtins.str".into(),
        Kind::Record(_) | Kind::Enum(_) => names::reference(&plan.input_name(id)),
        Kind::Sequence(element) | Kind::Array { element, .. } => {
            let count = if plan.zeroable(*element) {
                " | _builtins.int"
            } else {
                ""
            };
            format!("_abc.Sequence[{}]{count}", input_type(plan, *element))
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
        names::reference(&plan.view_name(id))
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

fn numeric_component_accessors(
    output: &mut String,
    plan: &TypeGraph,
    api: &IndexedAccessors,
    field: &Field,
    offset: &str,
) -> Result<(), GenerateError> {
    if let Some(scalar) = plan.component_scalar(field) {
        let mut prefix = format!("position = {offset}\n");
        if matches!(plan.nodes[field.ty].kind, Kind::Sequence(_)) {
            prefix.push_str("position += self._storage.u32[position // 4]\n");
        }
        let lane = format!(
            "self._storage.{}[position // {} + component]",
            format!("{scalar:?}").to_lowercase(),
            scalar.layout().size()
        );
        let docs = crate::documentation::indexed_component(&field.docs);
        if let Some(getter) = &api.component_getter {
            writeln!(
                output,
                "\n    def {}(self, index: _builtins.int, component: _builtins.int) -> {}:",
                getter,
                scalar_type(scalar)
            )?;
            documentation::write(output, &docs, "        ")?;
            writeln!(
                output,
                "        {}return {lane}",
                prefix.replace('\n', "\n        ")
            )?;
        }
        if let Some(setter) = &api.component_setter {
            if api.component_getter.is_none() {
                writeln!(output)?;
            }
            writeln!(
                output,
                "    def {}(self, index: _builtins.int, component: _builtins.int, value: {}) -> None:",
                setter,
                scalar_type(scalar)
            )?;
            documentation::write(output, &docs, "        ")?;
            writeln!(
                output,
                "        {}{lane} = value",
                prefix.replace('\n', "\n        ")
            )?;
        }
    }
    Ok(())
}
