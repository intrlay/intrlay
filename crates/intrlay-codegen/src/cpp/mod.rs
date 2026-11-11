use std::{collections::BTreeSet, fmt::Write};

use intrlay_schema::Scalar;

use crate::{
    GenerateError,
    api::Accessor,
    plan::{Field, GenerationPlan, Id, Kind, Node, TypeGraph},
};

mod api;
mod collections;
mod documentation;
mod name_validation;
mod names;
mod options;
mod reflection;

use options::Context;
pub use options::CppOptions;

pub const BACKEND: crate::backend::Backend = crate::backend::Backend {
    language: intrlay_schema::Language::Cpp,
    api: &api::Api,
    naming_policy: names::policy,
    reserved_declarations: &["detail"],
    is_reserved_name: name_validation::is_reserved,
    generate,
    field_setter: crate::api::SetterSyntax::Method,
    root_setter: crate::api::SetterSyntax::Method,
    input_field_kind: crate::SymbolKind::Field,
    input_field_modifiers: &[],
    codec_function_kind: crate::SymbolKind::Function,
    codec_function_modifiers: &[],
    fixed_array_api: crate::backend::FixedArrayApi::Structural,
    native_numeric_array_views: false,
    runtime_numeric_array_writers: false,
};

pub fn generate(
    generation: &GenerationPlan,
    options: &crate::GenerateOptions,
) -> Result<String, GenerateError> {
    let plan = &generation.encoding.types;
    let context = &Context::new(&options.cpp, options.module_name())?;
    name_validation::validate(generation)?;
    let mut output = String::from(include_str!("prelude.txt"));
    let module = &context.namespace;
    writeln!(output, "\nnamespace {module} {{\n\n// Native values.")?;
    reflection::declarations(&mut output, &generation.views.types)?;
    input_declarations(&mut output, plan, context)?;
    let mut done = BTreeSet::new();
    writeln!(output)?;
    for id in 0..plan.nodes.len() {
        input(&mut output, plan, context, id, &mut done)?;
    }
    let view_plan = &generation.views.types;
    let views = &generation.views.emitted;
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
                "template<::intrlay::BufferByte Byte>\nusing Basic{name} = {}<{}, Byte{constructible}>;",
                collections::view_template(view_plan, context, custom_collections),
                reflection::reference(view_plan, id, Some(context))
            )?;
        } else {
            writeln!(
                output,
                "template<::intrlay::BufferByte Byte> class Basic{name};"
            )?;
        }
        writeln!(
            output,
            "using {name} = Basic{name}<{byte}>;\nusing {name}ReadOnly = Basic{name}<const ::std::byte>;"
        )?;
    }
    reflection::generate(&mut output, view_plan, context)?;
    // Accessors depend on Byte, so their bodies instantiate after all view types
    // are complete, including mutually recursive borrowed views.
    for &id in views {
        if !is_collection(&view_plan.nodes[id]) {
            view(&mut output, view_plan, context, id, &view_plan.nodes[id])?;
        }
    }
    export_aliases(&mut output, plan, context, views)?;
    writeln!(output, "}} // namespace {module}")?;
    view_traits(&mut output, view_plan, context, views, custom_collections)?;
    writeln!(output, "// NOLINTEND")?;
    Ok(output)
}

fn input_declarations(
    output: &mut String,
    plan: &TypeGraph,
    context: &Context,
) -> Result<(), GenerateError> {
    for (id, node) in plan.nodes.iter().enumerate() {
        if node.is_value_enum() {
            writeln!(
                output,
                "enum class {} : ::std::uint32_t;",
                node.input_name()
            )?;
            continue;
        }
        match &node.kind {
            Kind::Record(fields) => {
                input_forward(output, plan, context, &node.input_name(), fields)?;
            }
            Kind::Enum(variants) => {
                let descriptor = reflection::reference(plan, id, Some(context));
                writeln!(
                    output,
                    "template<class... Sources>\n    requires ::intrlay::InputFieldsFor<::std::tuple<Sources...>, ::std::tuple<{descriptor}>>\nstruct {};",
                    node.input_name()
                )?;
                for variant in variants {
                    if !variant.synthetic_payload {
                        input_forward(
                            output,
                            plan,
                            context,
                            &node.variant_input_name(variant),
                            &variant.fields,
                        )?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Ownership uses runtime traits rather than the configurable public factories.
fn view_traits(
    output: &mut String,
    plan: &TypeGraph,
    context: &Context,
    views: &BTreeSet<Id>,
    custom_collections: bool,
) -> Result<(), GenerateError> {
    writeln!(output, "\nnamespace intrlay::detail {{")?;
    for &id in views {
        if is_collection(&plan.nodes[id]) {
            continue;
        }
        let view = context.qualified(&format!("Basic{}", plan.view_name(id)));
        let descriptor = reflection::reference(plan, id, Some(context));
        writeln!(
            output,
            "template<::intrlay::BufferByte Byte>\nstruct ViewTraits<{view}<Byte>>\n    : CheckedViewFactory<{view}<Byte>, {descriptor}> {{}};\n"
        )?;
    }
    if custom_collections && views.iter().any(|&id| is_collection(&plan.nodes[id])) {
        let view = collections::view_template(plan, context, true);
        writeln!(
            output,
            "template<ValidatableDescriptor Descriptor, ::intrlay::BufferByte Byte, bool Constructible>\nstruct ViewTraits<{view}<Descriptor, Byte, Constructible>>\n    : CheckedViewFactory<{view}<Descriptor, Byte, Constructible>, Descriptor> {{}};\n"
        )?;
    }
    writeln!(output, "}} // namespace intrlay::detail")?;
    Ok(())
}

fn export_aliases(
    output: &mut String,
    plan: &TypeGraph,
    context: &Context,
    views: &BTreeSet<Id>,
) -> Result<(), GenerateError> {
    for (alias, id) in &plan.exports {
        if *alias != plan.nodes[*id].name && views.contains(id) {
            writeln!(output, "using {alias}View = {};", plan.view_name(*id))?;
        }
        if *alias != plan.nodes[*id].name
            || !matches!(plan.nodes[*id].kind, Kind::Record(_) | Kind::Enum(_))
        {
            let node = &plan.nodes[*id];
            if matches!(node.kind, Kind::Record(_) | Kind::Enum(_)) && !node.is_value_enum() {
                writeln!(
                    output,
                    "template<class... Fields>\nusing {alias}Input = {}<Fields...>;",
                    context.qualified(&node.input_name())
                )?;
            } else {
                let suffix = if node.is_value_enum() { "" } else { "Input" };
                writeln!(
                    output,
                    "using {alias}{suffix} = {};",
                    input_type(plan, context, *id)
                )?;
            }
        }
    }
    Ok(())
}

fn is_collection(node: &Node) -> bool {
    matches!(node.kind, Kind::Sequence(_) | Kind::Array { .. })
}

fn input(
    output: &mut String,
    plan: &TypeGraph,
    context: &Context,
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
                input_dependency(output, plan, context, field.ty, done)?;
            }
            input_record(
                output,
                plan,
                context,
                &plan.input_name(id),
                fields,
                &node.docs,
            )?;
        }
        Kind::Enum(variants) => {
            if node.is_value_enum() {
                documentation::write(output, &node.docs, "")?;
                writeln!(
                    output,
                    "enum class {} : ::std::uint32_t {{",
                    node.input_name()
                )?;
                for variant in variants {
                    documentation::write(output, &variant.docs, "    ")?;
                    writeln!(output, "    {} = {},", variant.name, variant.tag)?;
                }
                writeln!(output, "}};\n")?;
                return Ok(());
            }
            for variant in variants {
                for field in &variant.fields {
                    input_dependency(output, plan, context, field.ty, done)?;
                }
                if !variant.synthetic_payload {
                    input_record(
                        output,
                        plan,
                        context,
                        &node.variant_input_name(variant),
                        &variant.fields,
                        &variant.docs,
                    )?;
                }
            }
            documentation::write(output, &node.docs, "")?;
            let name = node.input_name();
            writeln!(
                output,
                "template<>\nstruct {name}<> {{\n    using intrlay_input = ::intrlay::detail::GeneratedInputTag;"
            )?;
            writeln!(output, "    ::std::variant<")?;
            for (index, variant) in variants.iter().enumerate() {
                if index > 0 {
                    writeln!(output, ",")?;
                }
                let ty = if variant.synthetic_payload {
                    input_type(plan, context, variant.fields[0].ty)
                } else {
                    format!("{}<>", context.qualified(&node.variant_input_name(variant)))
                };
                write!(output, "        {ty}")?;
            }
            writeln!(output, "\n    > value;\n}};\n")?;
            writeln!(
                output,
                "template<class VariantSource>\n    requires ::intrlay::EncodeSourceFor<VariantSource, {}>\nstruct {name}<VariantSource> {{\n    using intrlay_input = ::intrlay::detail::GeneratedInputTag;\n    VariantSource value;\n}};\n\n{name}() -> {name}<>;\n",
                reflection::reference(plan, id, Some(context))
            )?;
            input_deduction_guide(output, &name)?;
        }
        _ => {}
    }
    Ok(())
}

fn input_dependency(
    output: &mut String,
    plan: &TypeGraph,
    context: &Context,
    id: Id,
    done: &mut BTreeSet<Id>,
) -> Result<(), GenerateError> {
    match plan.nodes[id].kind {
        Kind::Array { element, .. } => input_dependency(output, plan, context, element, done),
        Kind::Optional { value, .. } => input_dependency(output, plan, context, value, done),
        Kind::Record(_) | Kind::Enum(_) => input(output, plan, context, id, done),
        _ => Ok(()),
    }
}

fn input_forward(
    output: &mut String,
    plan: &TypeGraph,
    context: &Context,
    name: &str,
    fields: &[Field],
) -> Result<(), GenerateError> {
    let codecs = fields
        .iter()
        .map(|field| reflection::reference(plan, field.ty, Some(context)))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        output,
        "template<class... Sources>\n    requires ::intrlay::InputFieldsFor<::std::tuple<Sources...>, ::std::tuple<{codecs}>>\nstruct {name};"
    )?;
    Ok(())
}

fn input_record(
    output: &mut String,
    plan: &TypeGraph,
    context: &Context,
    name: &str,
    fields: &[Field],
    docs: &str,
) -> Result<(), GenerateError> {
    documentation::write(output, docs, "")?;
    writeln!(
        output,
        "template<>\nstruct {name}<> {{\n    using intrlay_input = ::intrlay::detail::GeneratedInputTag;"
    )?;
    for field in fields {
        documentation::write(output, &field.docs, "    ")?;
        // Class members already default-construct themselves. An explicit {}
        // can instantiate nested optional/unique_ptr payloads before a recursive
        // type is complete; scalar and enum fields/arrays need value initialization.
        let initializer = if needs_value_initialization(plan, field.ty) {
            "{}"
        } else {
            ""
        };
        writeln!(
            output,
            "    {} {}{initializer};",
            input_type(plan, context, field.ty),
            field.name
        )?;
    }
    writeln!(output, "}};\n")?;
    writeln!(output, "{name}() -> {name}<>;\n")?;
    if !fields.is_empty() {
        let mut taken = BTreeSet::new();
        let parameters: Vec<_> = fields
            .iter()
            .map(|field| {
                let mut name = plan.names.type_name(&format!("{}_source", field.name));
                while fields.iter().any(|field| field.name == name) || !taken.insert(name.clone()) {
                    name.push('_');
                }
                name
            })
            .collect();
        let declarations = parameters
            .iter()
            .map(|name| format!("class {name}"))
            .collect::<Vec<_>>()
            .join(", ");
        let arguments = parameters.join(", ");
        let constraints = fields
            .iter()
            .zip(&parameters)
            .map(|(field, parameter)| {
                format!(
                    "::intrlay::EncodeSourceFor<{parameter}, {}>",
                    reflection::reference(plan, field.ty, Some(context))
                )
            })
            .collect::<Vec<_>>()
            .join("\n        && ");
        writeln!(
            output,
            "// Field sources can be spans, string views, or nested inputs.\ntemplate<{declarations}>\n    requires ({constraints})\nstruct {name}<{arguments}> {{\n    using intrlay_input = ::intrlay::detail::GeneratedInputTag;"
        )?;
        for (field, parameter) in fields.iter().zip(&parameters) {
            documentation::write(output, &field.docs, "    ")?;
            writeln!(output, "    {parameter} {};", field.name)?;
        }
        writeln!(output, "}};\n")?;
        input_deduction_guide(output, name)?;
    }
    Ok(())
}

fn input_deduction_guide(output: &mut String, name: &str) -> Result<(), GenerateError> {
    // A matching pack guide keeps deduction through export aliases portable.
    // Only owned and full-field specializations are complete, rejecting other arities.
    writeln!(
        output,
        "template<class... Fields>\n{name}(Fields...) -> {name}<Fields...>;\n"
    )?;
    Ok(())
}

fn needs_value_initialization(plan: &TypeGraph, id: Id) -> bool {
    match plan.nodes[id].kind {
        Kind::Scalar(_) => true,
        Kind::Enum(_) => plan.nodes[id].is_value_enum(),
        Kind::Array { element, .. } => needs_value_initialization(plan, element),
        _ => false,
    }
}

fn view(
    output: &mut String,
    plan: &TypeGraph,
    context: &Context,
    id: Id,
    node: &Node,
) -> Result<(), GenerateError> {
    let name = plan.view_name(id);
    documentation::write(output, &node.docs, "")?;
    let offset_attribute = if node.layout.size() == 0 {
        "[[maybe_unused]] "
    } else {
        ""
    };
    writeln!(
        output,
        "\ntemplate<::intrlay::BufferByte Byte>\nclass Basic{name} {{\n    ::std::span<Byte> $[buffer];\n    {offset_attribute}::std::size_t $[offset];\n\npublic:\n    using $[metadata] = {};\n    static constexpr ::std::size_t $[byte_size] = {};\n    static constexpr ::std::size_t $[alignment] = {};\n\n    explicit Basic{name}(::std::span<Byte> bytes, ::std::size_t offset, ::intrlay::detail::Unchecked) noexcept\n        : $[buffer](bytes), $[offset](offset) {{}}\n\n    ::std::span<Byte> $[bytes]() const {{ return $[buffer]; }}",
        reflection::reference(plan, id, Some(context)),
        node.layout.size(),
        node.buffer_alignment
    )?;
    borrowing(output, &name)?;
    if node.options.generation.create {
        construction(output, plan, id)?;
    }
    let emitter = Methods { plan, context };
    view_members(output, plan, id, node, &emitter)?;
    writeln!(output, "}};")?;
    Ok(())
}

fn borrowing(output: &mut String, name: &str) -> Result<(), GenerateError> {
    writeln!(
        output,
        r"
    static ::intrlay::Expected<Basic{name}, ::intrlay::ViewError> $[from_bytes](::std::span<Byte> bytes) noexcept {{
        return $[from_parts](bytes, 0);
    }}

    static ::intrlay::Expected<Basic{name}, ::intrlay::ViewError> $[from_parts](::std::span<Byte> bytes, ::std::size_t offset) noexcept {{
        return ::intrlay::detail::CheckedViewFactory<Basic{name}, $[metadata]>::from_parts(bytes, offset);
    }}

    static Basic{name} $[from_bytes_unchecked](::std::span<Byte> bytes) noexcept {{
        return $[from_parts_unchecked](bytes, 0);
    }}

    static Basic{name} $[from_parts_unchecked](::std::span<Byte> bytes, ::std::size_t offset) noexcept {{
        return Basic{name}(bytes, offset, ::intrlay::detail::Unchecked{{}});
    }}
"
    )?;
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
        requires ::intrlay::EncodeSourceFor<V, $[metadata]>
    static ::std::size_t $[compute_byte_length](const V& value) {{
        return {factory}::compute_byte_length(value);
    }}

    template<class V>
        requires ::intrlay::EncodeSourceFor<V, $[metadata]>
    static ::intrlay::OwnedView<{name}> $[create](const V& value) {{
        return {factory}::create(value);
    }}

    template<class V>
        requires ::intrlay::EncodeSourceFor<V, $[metadata]>
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
        Methods::mutation(
            output,
            "$[replace]",
            "::intrlay::FixedEncodeSourceFor<V, $[metadata]>",
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
                let (ty, body) = if node.is_value_enum() {
                    (
                        output_type(plan, emitter.context, id),
                        "return $[metadata]::read($[buffer], $[offset]);",
                    )
                } else {
                    (
                        "::std::uint32_t".into(),
                        "return ::intrlay::detail::load<::std::uint32_t>($[buffer], $[offset]);",
                    )
                };
                Methods::reads(output, &ty, "$[tag]", body)?;
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
                Methods::reads(
                    output,
                    &output_type(plan, emitter.context, id),
                    "$[value]",
                    "return $[metadata]::read($[buffer], $[offset]);",
                )?;
            }
            if node.options.generation.setters && plan.mutation(id).is_some() {
                Methods::mutation(
                    output,
                    "$[set]",
                    "::intrlay::SetSourceFor<V, $[metadata]>",
                    "$[metadata]::set($[buffer], $[offset], value);",
                )?;
            }
        }
    }
    Ok(())
}

struct Methods<'a> {
    plan: &'a TypeGraph,
    context: &'a Context,
}
impl Methods<'_> {
    fn reads(output: &mut String, ty: &str, name: &str, body: &str) -> Result<(), GenerateError> {
        Self::method(output, ty, &format!("{name}() const noexcept"), body)
    }

    fn mutation(
        output: &mut String,
        name: &str,
        constraint: &str,
        body: &str,
    ) -> Result<(), GenerateError> {
        writeln!(
            output,
            "    template<class V>\n        requires ({constraint})"
        )?;
        Self::method(
            output,
            "void",
            &format!("{name}(const V& value) const requires ::intrlay::WritableBufferByte<Byte>"),
            body,
        )
    }

    fn method(
        output: &mut String,
        ty: &str,
        signature: &str,
        body: &str,
    ) -> Result<(), GenerateError> {
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
            let ty = output_type(self.plan, self.context, field.ty);
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
            Self::reads(output, &ty, name, &format!("return {value};"))?;
        }
        if let Some(accessor) = &api.setter {
            let setter_name = &accessor.name;
            documentation::write(output, &field.docs, "    ")?;
            let (field_descriptor, setter) = if let Some(variant) = variant {
                (
                    format!("typename {descriptor}::Case<{variant}>::Field<{index}>"),
                    format!("{descriptor}::set_field<{variant}, {index}>"),
                )
            } else {
                (
                    format!("typename {descriptor}::Field<{index}>"),
                    format!("{descriptor}::Field<{index}>::set"),
                )
            };
            Self::mutation(
                output,
                setter_name,
                &format!("::intrlay::SetSourceFor<V, {field_descriptor}::Type>"),
                &format!("{setter}($[buffer], $[offset], value);"),
            )?;
        }
        Ok(())
    }
}

fn input_type(plan: &TypeGraph, context: &Context, id: Id) -> String {
    if plan.nodes[id].is_value_enum() {
        return context.qualified(&plan.input_name(id));
    }
    match &plan.nodes[id].kind {
        Kind::Scalar(scalar) => scalar_type(*scalar).into(),
        Kind::Unit => "::std::monostate".into(),
        Kind::String => "::std::string".into(),
        Kind::Record(_) | Kind::Enum(_) => format!("{}<>", context.qualified(&plan.input_name(id))),
        Kind::Sequence(element) => {
            format!("::std::vector<{}>", input_type(plan, context, *element))
        }
        Kind::Array { element, length } => {
            format!(
                "::std::array<{}, {length}>",
                input_type(plan, context, *element)
            )
        }
        Kind::Optional { value, .. } => {
            format!("::std::optional<{}>", input_type(plan, context, *value))
        }
        Kind::Boxed(value) => format!("::std::unique_ptr<{}>", input_type(plan, context, *value)),
    }
}
fn output_type(plan: &TypeGraph, context: &Context, id: Id) -> String {
    if plan.nodes[id].is_value_enum() {
        return context.qualified(&plan.input_name(id));
    }
    match &plan.nodes[id].kind {
        Kind::Scalar(scalar) => scalar_type(*scalar).into(),
        Kind::Unit => "::std::monostate".into(),
        Kind::String => "::std::string_view".into(),
        Kind::Optional { value, .. } => {
            format!("::std::optional<{}>", output_type(plan, context, *value))
        }
        Kind::Boxed(value) => output_type(plan, context, *value),
        _ => format!(
            "{}<{}>",
            context.qualified(&format!("Basic{}", plan.view_name(id))),
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
