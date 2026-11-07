use std::{collections::BTreeSet, fmt::Write};

use intrlay_schema::Scalar;

use crate::{
    GenerateError,
    api::Accessor,
    plan::{Field, GenerationPlan, Id, Kind, Node, TypeGraph},
};

pub(crate) mod collections;
mod documentation;
mod names;
pub(crate) mod reflection;

pub(crate) use names::OBJECT_MACROS;

pub const BACKEND: crate::backend::Backend = crate::backend::Backend {
    language: intrlay_schema::Language::Cpp,
    naming_policy: names::policy,
    keywords: names::KEYWORDS,
    reserved_declarations: &["detail"],
    generate,
    field_setter: crate::api::SetterSyntax::Method,
    root_setter: crate::api::SetterSyntax::Method,
    input_field_kind: crate::SymbolKind::Field,
    input_field_modifiers: &[],
    codec_function_kind: crate::SymbolKind::Function,
    codec_function_modifiers: &[],
    fixed_array_api: crate::backend::FixedArrayApi::Structural,
    native_numeric_array_views: false,
};

pub fn generate(generation: &GenerationPlan) -> Result<String, GenerateError> {
    let plan = &generation.encoding.types;
    let mut output = String::from(include_str!("prelude.txt"));
    let module = namespace(plan);
    validate_namespace(&module)?;
    writeln!(output, "\nnamespace {module} {{\n\n// Native values.")?;
    for node in &plan.nodes {
        if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) {
            writeln!(output, "struct {};", node.name)?;
        }
    }
    let mut done = BTreeSet::new();
    writeln!(output)?;
    for id in 0..plan.nodes.len() {
        input(&mut output, plan, id, &mut done)?;
    }
    let view_plan = &generation.views.types;
    let views = &generation.views.emitted;
    reflection::declarations(&mut output, view_plan)?;
    let custom_collections = collections::has_custom_names(view_plan)?;
    if custom_collections && views.iter().any(|&id| is_collection(&view_plan.nodes[id])) {
        collections::generate_facade(&mut output, view_plan)?;
    }
    writeln!(
        output,
        "\n// Mutable and const views share one implementation."
    )?;
    for &id in views {
        let name = view_plan.view_name(id);
        let byte = if view_plan.nodes[id].options.generation.setters {
            "::std::byte"
        } else {
            "const ::std::byte"
        };
        if is_collection(&view_plan.nodes[id]) {
            documentation::write(&mut output, &view_plan.nodes[id].docs, "")?;
            let constructible = if view_plan.nodes[id].options.generation.create {
                ""
            } else {
                ", false"
            };
            writeln!(
                output,
                "template<class Byte>\nusing Basic{name} = {}<{}, Byte{constructible}>;",
                collections::view_template(view_plan, custom_collections),
                reflection::reference(view_plan, id, true)
            )?;
        } else {
            writeln!(output, "template<class Byte> class Basic{name};")?;
        }
        writeln!(
            output,
            "using {name} = Basic{name}<{byte}>;\nusing {name}ReadOnly = Basic{name}<const ::std::byte>;"
        )?;
    }
    reflection::generate(&mut output, view_plan)?;
    // Accessors depend on Byte, so their bodies instantiate after all view types
    // are complete, including mutually recursive borrowed views.
    for &id in views {
        if !is_collection(&view_plan.nodes[id]) {
            view(&mut output, view_plan, id, &view_plan.nodes[id])?;
        }
    }
    for (alias, id) in &plan.exports {
        if *alias != plan.nodes[*id].name {
            writeln!(output, "using {alias}View = {};", plan.view_name(*id))?;
        }
    }
    writeln!(output, "}} // namespace {module}\n// NOLINTEND")?;
    Ok(output)
}

fn is_collection(node: &Node) -> bool {
    matches!(node.kind, Kind::Sequence(_) | Kind::Array { .. })
}

fn namespace(plan: &TypeGraph) -> String {
    plan.cpp_namespace.clone().unwrap_or_else(|| {
        format!(
            "intrlay_generated::{}",
            intrlay_schema::NameCase::SnakeCase.apply(&plan.module_name)
        )
    })
}

/// Names of schema types can also occur as fields or method/template parameters.
fn qualified(plan: &TypeGraph, name: &str) -> String {
    format!("::{}::{name}", namespace(plan))
}

fn validate_namespace(namespace: &str) -> Result<(), GenerateError> {
    for segment in namespace.split("::") {
        let mut characters = segment.chars();
        if !characters
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            || !characters.all(|c| c.is_ascii_alphanumeric() || c == '_')
            || names::KEYWORDS
                .split_whitespace()
                .any(|keyword| keyword == segment)
            || names::OBJECT_MACROS.contains(&segment)
        {
            return Err(GenerateError(format!(
                "invalid C++ namespace {namespace:?}"
            )));
        }
    }
    Ok(())
}

fn input(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    done: &mut BTreeSet<Id>,
) -> Result<(), GenerateError> {
    if !done.insert(id) {
        return Ok(());
    }
    let node = &plan.nodes[id];
    match &node.kind {
        Kind::Record(fields) => {
            for field in fields {
                input_dependency(output, plan, field.ty, done)?;
            }
            input_record(output, plan, &plan.input_name(id), fields, &node.docs)?;
        }
        Kind::Enum(variants) => {
            for variant in variants {
                for field in &variant.fields {
                    input_dependency(output, plan, field.ty, done)?;
                }
                if !variant.synthetic_payload {
                    input_record(
                        output,
                        plan,
                        &format!("{}{}", node.name, variant.name),
                        &variant.fields,
                        &variant.docs,
                    )?;
                }
            }
            documentation::write(output, &node.docs, "")?;
            writeln!(output, "struct {} {{", node.name)?;
            writeln!(output, "    ::std::variant<")?;
            for (index, variant) in variants.iter().enumerate() {
                if index > 0 {
                    writeln!(output, ",")?;
                }
                let ty = if variant.synthetic_payload {
                    input_type(plan, variant.fields[0].ty)
                } else {
                    qualified(plan, &format!("{}{}", node.name, variant.name))
                };
                write!(output, "        {ty}")?;
            }
            writeln!(output, "\n    > value;\n}};\n")?;
        }
        _ => {}
    }
    Ok(())
}

fn input_dependency(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    done: &mut BTreeSet<Id>,
) -> Result<(), GenerateError> {
    match plan.nodes[id].kind {
        Kind::Array { element, .. } => input_dependency(output, plan, element, done),
        Kind::Optional { value, .. } => input_dependency(output, plan, value, done),
        Kind::Record(_) | Kind::Enum(_) => input(output, plan, id, done),
        _ => Ok(()),
    }
}

fn input_record(
    output: &mut String,
    plan: &TypeGraph,
    name: &str,
    fields: &[Field],
    docs: &str,
) -> Result<(), GenerateError> {
    documentation::write(output, docs, "")?;
    writeln!(output, "struct {name} {{")?;
    for field in fields {
        documentation::write(output, &field.docs, "    ")?;
        // Class members already default-construct themselves. An explicit {}
        // can instantiate nested optional/unique_ptr payloads before a recursive
        // type is complete; only arithmetic fields/arrays need value initialization.
        let initializer = if needs_value_initialization(plan, field.ty) {
            "{}"
        } else {
            ""
        };
        writeln!(
            output,
            "    {} {}{initializer};",
            input_type(plan, field.ty),
            field.name
        )?;
    }
    writeln!(output, "}};\n")?;
    Ok(())
}

fn needs_value_initialization(plan: &TypeGraph, id: Id) -> bool {
    match plan.nodes[id].kind {
        Kind::Scalar(_) => true,
        Kind::Array { element, .. } => needs_value_initialization(plan, element),
        _ => false,
    }
}

fn view(output: &mut String, plan: &TypeGraph, id: Id, node: &Node) -> Result<(), GenerateError> {
    let name = plan.view_name(id);
    documentation::write(output, &node.docs, "")?;
    let offset_attribute = if node.layout.size() == 0 {
        "[[maybe_unused]] "
    } else {
        ""
    };
    writeln!(
        output,
        "\ntemplate<class Byte>\nclass Basic{name} {{\n    ::std::span<Byte> $[buffer];\n    {offset_attribute}::std::size_t $[offset];\n\npublic:\n    using $[metadata] = {};\n    static constexpr ::std::size_t $[byte_size] = {};\n    static constexpr ::std::size_t $[alignment] = {};\n\n    explicit Basic{name}(::std::span<Byte> bytes, ::std::size_t offset = 0)\n        : $[buffer](bytes), $[offset](offset) {{}}\n\n    ::std::span<Byte> $[bytes]() const {{ return $[buffer]; }}",
        reflection::reference(plan, id, true),
        node.layout.size(),
        node.buffer_alignment
    )?;
    if node.options.generation.create {
        construction(output, plan, id)?;
    }
    let emitter = Methods { plan };
    view_members(output, plan, id, node, &emitter)?;
    writeln!(output, "}};")?;
    Ok(())
}

fn construction(output: &mut String, plan: &TypeGraph, id: Id) -> Result<(), GenerateError> {
    let name = format!("Basic{}<Byte>", plan.view_name(id));
    let factory = format!(
        "::intrlay::ViewFactory<\n            {name}, $[metadata], $[byte_size], $[alignment], {}>",
        plan.writes_all_bytes(id)
    );
    writeln!(
        output,
        r"
    template<class V>
    static ::std::size_t $[compute_byte_length](const V& value) {{
        return {factory}::compute_byte_length(value);
    }}

    template<class V>
    static ::intrlay::OwnedView<{name}> $[create](const V& value) {{
        return {factory}::create(value);
    }}

    template<class V>
    static {name} $[create_in_place](::std::span<::std::byte> bytes, const V& value) {{
        return {factory}::create_in_place(bytes, value);
    }}
"
    )?;
    Ok(())
}

fn view_members(
    output: &mut String,
    plan: &TypeGraph,
    id: Id,
    node: &Node,
    emitter: &Methods<'_>,
) -> Result<(), GenerateError> {
    if node.options.generation.setters && !plan.has_payload(id) {
        Methods::method(
            output,
            "void",
            &format!("$[replace](const {}& value) const", input_type(plan, id)),
            "::intrlay::replace<$[metadata]>($[buffer], $[offset], value);",
        )?;
    }
    match &node.kind {
        Kind::Record(fields) => {
            for (index, field) in fields.iter().enumerate() {
                let api = Accessor::field(plan, node, field, None);
                emitter.field(output, field, index, &api, None)?;
            }
        }
        Kind::Enum(variants) => {
            if node.options.generation.getters {
                Methods::method(
                    output,
                    "::std::uint32_t",
                    "$[tag]() const",
                    "return ::intrlay::detail::load<::std::uint32_t>($[buffer], $[offset]);",
                )?;
            }
            for (variant_index, variant) in variants.iter().enumerate() {
                for (field_index, field) in variant.fields.iter().enumerate() {
                    let api = Accessor::field(plan, node, field, Some(variant));
                    emitter.field(output, field, field_index, &api, Some(variant_index))?;
                }
            }
        }
        Kind::Sequence(_) | Kind::Array { .. } => unreachable!("collections use shared views"),
        _ => {
            if node.options.generation.getters {
                Methods::method(
                    output,
                    &output_type(plan, id),
                    "$[value]() const",
                    "return $[metadata]::read($[buffer], $[offset]);",
                )?;
            }
            if node.options.generation.setters
                && let Some(mutation) = plan.mutation(id)
            {
                Methods::method(
                    output,
                    "void",
                    &format!("$[set]({} value) const", input_type(plan, mutation.value)),
                    "$[metadata]::set($[buffer], $[offset], value);",
                )?;
            }
        }
    }
    Ok(())
}

struct Methods<'a> {
    plan: &'a TypeGraph,
}
impl Methods<'_> {
    fn method(
        output: &mut String,
        ty: &str,
        signature: &str,
        body: &str,
    ) -> Result<(), GenerateError> {
        let signature = if ty == "void" {
            format!("{signature} requires (!::std::is_const_v<Byte>)")
        } else {
            signature.to_owned()
        };
        let body = body.replace('\n', "\n        ");
        if ty.len() + signature.len() > 100 {
            writeln!(
                output,
                "    {ty}\n    {signature} {{\n        {body}\n    }}"
            )?;
        } else {
            writeln!(output, "    {ty} {signature} {{\n        {body}\n    }}")?;
        }
        Ok(())
    }

    fn field(
        &self,
        output: &mut String,
        field: &Field,
        index: usize,
        api: &Accessor,
        variant: Option<usize>,
    ) -> Result<(), GenerateError> {
        let name = &api.name;
        let descriptor = "$[metadata]";
        if api.getter {
            documentation::write(output, &field.docs, "    ")?;
            let ty = output_type(self.plan, field.ty);
            let (ty, value) = if let Some(variant) = variant {
                (
                    format!("::std::optional<{ty}>"),
                    format!("{descriptor}::read_field<{variant}, {index}>($[buffer], $[offset])"),
                )
            } else {
                (
                    ty,
                    format!("{descriptor}::Field<{index}>::read($[buffer], $[offset])"),
                )
            };
            Self::method(
                output,
                &ty,
                &format!("{name}() const"),
                &format!("return {value};"),
            )?;
        }
        if let Some(accessor) = &api.setter {
            let mutation = &accessor.mutation;
            let setter_name = &accessor.name;
            documentation::write(output, &field.docs, "    ")?;
            let setter = if let Some(variant) = variant {
                format!("{descriptor}::set_field<{variant}, {index}>")
            } else {
                format!("{descriptor}::Field<{index}>::set")
            };
            Self::method(
                output,
                "void",
                &format!(
                    "{setter_name}(const {}& value) const",
                    input_type(self.plan, mutation.value)
                ),
                &format!("{setter}($[buffer], $[offset], value);"),
            )?;
        }
        Ok(())
    }
}

fn input_type(plan: &TypeGraph, id: Id) -> String {
    match &plan.nodes[id].kind {
        Kind::Scalar(scalar) => scalar_type(*scalar).into(),
        Kind::Unit => "::std::monostate".into(),
        Kind::String => "::std::string".into(),
        Kind::Record(_) | Kind::Enum(_) => qualified(plan, &plan.input_name(id)),
        Kind::Sequence(element) => format!("::std::vector<{}>", input_type(plan, *element)),
        Kind::Array { element, length } => {
            format!("::std::array<{}, {length}>", input_type(plan, *element))
        }
        Kind::Optional { value, .. } => format!("::std::optional<{}>", input_type(plan, *value)),
        Kind::Boxed(value) => format!("::std::unique_ptr<{}>", input_type(plan, *value)),
    }
}
fn output_type(plan: &TypeGraph, id: Id) -> String {
    match &plan.nodes[id].kind {
        Kind::Scalar(scalar) => scalar_type(*scalar).into(),
        Kind::Unit => "::std::monostate".into(),
        Kind::String => "::std::string_view".into(),
        Kind::Optional { value, .. } => format!("::std::optional<{}>", output_type(plan, *value)),
        Kind::Boxed(value) => output_type(plan, *value),
        _ => format!(
            "{}<{}>",
            qualified(plan, &format!("Basic{}", plan.view_name(id))),
            if plan.nodes[id].options.generation.setters {
                "Byte"
            } else {
                "const ::std::byte"
            }
        ),
    }
}
fn scalar_type(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Bool => "bool",
        Scalar::I8 => "::std::int8_t",
        Scalar::U8 => "::std::uint8_t",
        Scalar::I16 => "::std::int16_t",
        Scalar::U16 => "::std::uint16_t",
        Scalar::I32 => "::std::int32_t",
        Scalar::U32 => "::std::uint32_t",
        Scalar::I64 => "::std::int64_t",
        Scalar::U64 => "::std::uint64_t",
        Scalar::F32 => "float",
        Scalar::F64 => "double",
    }
}
