use std::collections::{BTreeMap, BTreeSet};

use clang::{Accessibility, Entity, EntityKind, Type as CppType, TypeKind};
use intrlay_schema::{Definition, DefinitionKind, Field, Scalar, Schema, Type, Variant};

use crate::{FrontendError, annotations::Annotation};

#[derive(Default)]
pub struct Extractor {
    schema: Schema,
    visiting: BTreeSet<String>,
    exports: BTreeMap<String, (String, Annotation)>,
    alternative_names: BTreeMap<String, BTreeMap<usize, String>>,
}

impl Extractor {
    pub fn extract(mut self, root: Entity<'_>) -> Result<Schema, FrontendError> {
        self.discover(root)?;
        if self.schema.exports.is_empty() {
            return Err(FrontendError(
                "no INTRLAY annotated export was found".into(),
            ));
        }
        self.name_alternatives()?;
        Ok(self.schema)
    }

    fn discover(&mut self, parent: Entity<'_>) -> Result<(), FrontendError> {
        for entity in parent.get_children() {
            if entity.is_in_system_header() {
                continue;
            }
            match entity.get_kind() {
                EntityKind::Namespace | EntityKind::LinkageSpec => self.discover(entity)?,
                EntityKind::StructDecl
                | EntityKind::ClassDecl
                | EntityKind::UnionDecl
                | EntityKind::EnumDecl
                | EntityKind::TypeAliasDecl
                | EntityKind::TypedefDecl => {
                    let annotation = Annotation::parse(entity)?;
                    if !annotation.export {
                        continue;
                    }
                    let name = annotation
                        .name
                        .clone()
                        .or_else(|| entity.get_name())
                        .ok_or_else(|| FrontendError("export must have a name".into()))?;
                    let identity = entity
                        .get_canonical_entity()
                        .get_usr()
                        .ok_or_else(|| {
                            FrontendError(format!("export {name} has no declaration identity"))
                        })?
                        .0;
                    if let Some((previous_name, previous_annotation)) = self.exports.get(&identity)
                    {
                        if *previous_name != name || *previous_annotation != annotation {
                            return Err(FrontendError(format!(
                                "conflicting annotations on redeclared export: {name}"
                            )));
                        }
                        continue;
                    }
                    let native = entity
                        .get_typedef_underlying_type()
                        .or_else(|| entity.get_type())
                        .ok_or_else(|| {
                            FrontendError(format!("export {name} has no concrete type"))
                        })?;
                    self.alternative_names(native, &annotation)?;
                    let ty = self.ty(native)?;
                    if self.schema.exports.insert(name.clone(), ty).is_some() {
                        return Err(FrontendError(format!("duplicate export: {name}")));
                    }
                    self.exports.insert(identity, (name, annotation));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn alternative_names(
        &mut self,
        native: CppType<'_>,
        annotation: &Annotation,
    ) -> Result<(), FrontendError> {
        if annotation.alternatives.is_empty() {
            return Ok(());
        }
        let native = native.get_canonical_type();
        let declaration = native.get_declaration().ok_or_else(|| {
            FrontendError("alternative names require a std::variant alias".into())
        })?;
        if declaration.get_name().as_deref() != Some("variant")
            || !in_standard_namespace(declaration)
        {
            return Err(FrontendError(
                "alternative names require a std::variant alias".into(),
            ));
        }
        let native = unqualified_named_type(native, declaration)?;
        let count = native
            .get_template_argument_types()
            .unwrap_or_default()
            .len();
        let names = self
            .alternative_names
            .entry(native.get_display_name())
            .or_default();
        for (&index, name) in &annotation.alternatives {
            if index >= count {
                return Err(FrontendError(format!(
                    "alternative index {index} exceeds std::variant's {count} alternatives"
                )));
            }
            if let Some(previous) = names.insert(index, name.clone())
                && previous != *name
            {
                return Err(FrontendError(format!(
                    "conflicting names for alternative {index}: {previous} and {name}"
                )));
            }
        }
        Ok(())
    }

    fn name_alternatives(&mut self) -> Result<(), FrontendError> {
        // Resolve after discovery: an annotated alias may follow a use of the
        // same canonical variant, and recursive payload records may be incomplete.
        let mut resolved = Vec::new();
        for (identity, definition) in &self.schema.definitions {
            let DefinitionKind::Enum(variants) = &definition.kind else {
                continue;
            };
            let mut names = BTreeSet::new();
            for (index, variant) in variants.iter().enumerate() {
                if !variant.synthetic_payload {
                    continue;
                }
                let name = self
                    .alternative_names
                    .get(identity)
                    .and_then(|names| names.get(&index))
                    .cloned()
                    .unwrap_or_else(|| self.type_name(&variant.fields[0].ty));
                if !names.insert(name.clone()) {
                    return Err(FrontendError(format!(
                        "duplicate alternative name {name} in {identity}; use alternative(index, name = \"Name\") on a std::variant alias"
                    )));
                }
                resolved.push((identity.clone(), index, name));
            }
        }
        for (identity, index, name) in resolved {
            if let Some(Definition {
                kind: DefinitionKind::Enum(variants),
                ..
            }) = self.schema.definitions.get_mut(&identity)
            {
                variants[index].name = name;
            }
        }
        Ok(())
    }

    fn type_name(&self, ty: &Type) -> String {
        match ty {
            Type::Scalar(scalar) => format!("{scalar:?}"),
            Type::Unit => "Monostate".into(),
            Type::String => "String".into(),
            Type::Named(identity) => self
                .schema
                .exports
                .iter()
                .find_map(|(name, exported)| (exported == ty).then(|| name.clone()))
                .unwrap_or_else(|| self.schema.definitions[identity].name.clone()),
            Type::Array { element, length } => {
                format!("Array{length}Of{}", self.type_name(element))
            }
            Type::Sequence(element) => format!("SequenceOf{}", self.type_name(element)),
            Type::Optional(element) => format!("Optional{}", self.type_name(element)),
            Type::Boxed(element) => format!("BoxOf{}", self.type_name(element)),
        }
    }

    fn ty(&mut self, native: CppType<'_>) -> Result<Type, FrontendError> {
        let native = native.get_canonical_type();
        let scalar = match native.get_kind() {
            TypeKind::Bool => Some(Scalar::Bool),
            TypeKind::SChar | TypeKind::CharS => Some(Scalar::I8),
            TypeKind::UChar | TypeKind::CharU => Some(Scalar::U8),
            TypeKind::Short | TypeKind::Int | TypeKind::Long | TypeKind::LongLong => {
                Some(integer(native, true)?)
            }
            TypeKind::UShort | TypeKind::UInt | TypeKind::ULong | TypeKind::ULongLong => {
                Some(integer(native, false)?)
            }
            TypeKind::Float => Some(Scalar::F32),
            TypeKind::Double => Some(Scalar::F64),
            _ => None,
        };
        if let Some(scalar) = scalar {
            return Ok(Type::Scalar(scalar));
        }
        match native.get_kind() {
            TypeKind::ConstantArray => Ok(Type::Array {
                element: Box::new(
                    self.ty(native
                        .get_element_type()
                        .ok_or_else(|| unsupported(native))?)?,
                ),
                length: native.get_size().ok_or_else(|| unsupported(native))?,
            }),
            TypeKind::Record | TypeKind::Enum | TypeKind::Unexposed => self.named(native),
            _ => Err(unsupported(native)),
        }
    }

    fn named(&mut self, native: CppType<'_>) -> Result<Type, FrontendError> {
        let entity = native
            .get_declaration()
            .ok_or_else(|| unsupported(native))?;
        let entity = entity.get_definition().unwrap_or(entity);
        if entity.get_kind() == EntityKind::UnionDecl {
            return Err(FrontendError(format!(
                "untagged union requires an explicit tagged representation such as std::variant: {}",
                native.get_display_name()
            )));
        }
        let native = unqualified_named_type(native, entity)?;
        let name = entity.get_name().ok_or_else(|| unsupported(native))?;
        if in_standard_namespace(entity) {
            return self.standard(native, &name);
        }
        let identity = native.get_display_name();
        if self.schema.definitions.contains_key(&identity)
            || !self.visiting.insert(identity.clone())
        {
            return Ok(Type::Named(identity));
        }
        let annotation = Annotation::parse(entity)?;
        let kind = if native.get_kind() == TypeKind::Enum {
            DefinitionKind::Enum(enum_variants(native, entity)?)
        } else {
            // libclang omits base cursors on instantiated classes. Their selected
            // template supplies the bases, unless this is an explicit specialization
            // with its own definition (which can remove the template's bases).
            let pattern = entity.get_template().filter(|pattern| {
                pattern.get_kind() == EntityKind::ClassTemplatePartialSpecialization
                    || !is_explicit_specialization(entity)
            });
            if pattern
                .unwrap_or(entity)
                .get_children()
                .iter()
                .any(|child| child.get_kind() == EntityKind::BaseSpecifier)
            {
                return Err(FrontendError(format!(
                    "inheritance requires an explicit mirror type: {identity}"
                )));
            }
            native.get_sizeof().map_err(|_| FrontendError(format!("incomplete type {identity}; instantiate concrete templates before export (for example with sizeof)")))?;
            let fields = native.get_fields().ok_or_else(|| unsupported(native))?;
            let mut result = Vec::with_capacity(fields.len());
            for field in fields {
                if field.is_bit_field()
                    || matches!(
                        field.get_accessibility(),
                        Some(Accessibility::Private | Accessibility::Protected)
                    )
                {
                    return Err(FrontendError(format!(
                        "field requires a public mirror: {}",
                        field.get_display_name().unwrap_or_default()
                    )));
                }
                let attributes = Annotation::parse(field)?;
                result.push(Field {
                    name: field.get_name().ok_or_else(|| unsupported(native))?,
                    ty: self.ty(field.get_type().ok_or_else(|| unsupported(native))?)?,
                    docs: crate::documentation::extract(field),
                    targets: attributes.targets,
                });
            }
            DefinitionKind::Record(result)
        };
        let alignment = annotation.alignment.unwrap_or_else(|| {
            if entity
                .get_children()
                .iter()
                .any(|child| child.get_kind() == EntityKind::AlignedAttr)
            {
                native.get_alignof().unwrap_or(1)
            } else {
                1
            }
        });
        self.schema.definitions.insert(
            identity.clone(),
            Definition {
                name: annotation.name.unwrap_or(name),
                docs: crate::documentation::extract(entity),
                kind,
                alignment,
                targets: annotation.targets,
                transparent: annotation.transparent,
            },
        );
        self.visiting.remove(&identity);
        Ok(Type::Named(identity))
    }

    fn standard(&mut self, native: CppType<'_>, name: &str) -> Result<Type, FrontendError> {
        let arguments = native.get_template_argument_types().unwrap_or_default();
        let first = |index| {
            arguments
                .get(index)
                .copied()
                .flatten()
                .ok_or_else(|| unsupported(native))
        };
        Ok(match name {
            "vector" => Type::Sequence(Box::new(self.ty(first(0)?)?)),
            "optional" => Type::Optional(Box::new(self.ty(first(0)?)?)),
            "unique_ptr" => Type::Boxed(Box::new(self.ty(first(0)?)?)),
            "basic_string" if matches!(first(0)?.get_kind(), TypeKind::CharS | TypeKind::CharU) => {
                Type::String
            }
            "monostate" => Type::Unit,
            "array" => {
                // libclang exposes type arguments but not integral class-template arguments.
                // Canonical spelling contains the compiler-evaluated, concrete array count.
                let spelling = native.get_display_name();
                let count = spelling
                    .rsplit_once(',')
                    .and_then(|(_, suffix)| suffix.trim().strip_suffix('>'))
                    .and_then(|count| count.trim().parse().ok())
                    .ok_or_else(|| unsupported(native))?;
                Type::Array {
                    element: Box::new(self.ty(first(0)?)?),
                    length: count,
                }
            }
            "variant" => {
                let identity = native.get_display_name();
                if !self.schema.definitions.contains_key(&identity)
                    && self.visiting.insert(identity.clone())
                {
                    let mut variants = Vec::with_capacity(arguments.len());
                    for (index, argument) in arguments.into_iter().enumerate() {
                        variants.push(Variant {
                            name: format!("Alternative{index}"),
                            tag: u32::try_from(index)
                                .map_err(|error| FrontendError(error.to_string()))?,
                            fields: vec![Field::new(
                                "value",
                                self.ty(argument.ok_or_else(|| unsupported(native))?)?,
                            )],
                            synthetic_payload: true,
                            docs: String::new(),
                            targets: BTreeMap::new(),
                        });
                    }
                    self.schema.definitions.insert(
                        identity.clone(),
                        Definition {
                            name: format!("Variant{}", self.schema.definitions.len()),
                            docs: String::new(),
                            kind: DefinitionKind::Enum(variants),
                            alignment: 1,
                            targets: BTreeMap::new(),
                            transparent: false,
                        },
                    );
                    self.visiting.remove(&identity);
                }
                Type::Named(identity)
            }
            _ => return Err(unsupported(native)),
        })
    }
}

fn is_explicit_specialization(entity: Entity<'_>) -> bool {
    let Some(range) = entity.get_range() else {
        return false;
    };
    let tokens = range.tokenize();
    let mut tokens = tokens
        .iter()
        .filter(|token| token.get_kind() != clang::token::TokenKind::Comment)
        .map(clang::token::Token::get_spelling)
        .peekable();
    if !tokens.any(|token| token == "template") {
        return false;
    }
    // Nested specializations have one header per enclosing template. Only the
    // final header describes this class; an outer `template <>` is insufficient.
    loop {
        if tokens.next().as_deref() != Some("<") {
            return false;
        }
        let empty = tokens.peek().is_some_and(|token| token == ">");
        let mut depth = 1_i32;
        for token in tokens.by_ref() {
            match token.as_str() {
                "<" => depth += 1,
                ">" => depth -= 1,
                ">>" => depth -= 2,
                _ => {}
            }
            if depth <= 0 {
                break;
            }
        }
        if depth != 0 {
            return false;
        }
        if tokens.peek().is_none_or(|token| token != "template") {
            return empty;
        }
        tokens.next();
    }
}

fn enum_variants(native: CppType<'_>, entity: Entity<'_>) -> Result<Vec<Variant>, FrontendError> {
    entity
        .get_children()
        .into_iter()
        .filter(|child| child.get_kind() == EntityKind::EnumConstantDecl)
        .map(|child| {
            let (_, tag) = child
                .get_enum_constant_value()
                .ok_or_else(|| unsupported(native))?;
            Ok(Variant {
                name: child.get_name().ok_or_else(|| unsupported(native))?,
                tag: u32::try_from(tag)
                    .map_err(|_| FrontendError("enum tags must fit u32".into()))?,
                fields: Vec::new(),
                synthetic_payload: false,
                docs: crate::documentation::extract(child),
                targets: Annotation::parse(child)?.targets,
            })
        })
        .collect()
}

fn unqualified_named_type<'tu>(
    native: CppType<'tu>,
    declaration: Entity<'tu>,
) -> Result<CppType<'tu>, FrontendError> {
    // Canonical types retain top-level cv qualifiers. The declaration's type
    // removes those qualifiers while preserving concrete template arguments.
    if native.is_const_qualified() || native.is_volatile_qualified() {
        Ok(declaration
            .get_type()
            .ok_or_else(|| unsupported(native))?
            .get_canonical_type())
    } else {
        Ok(native)
    }
}

fn in_standard_namespace(mut entity: Entity<'_>) -> bool {
    while let Some(parent) = entity.get_semantic_parent() {
        if parent.get_kind() == EntityKind::Namespace && parent.get_name().as_deref() == Some("std")
        {
            return true;
        }
        entity = parent;
    }
    false
}

fn integer(native: CppType<'_>, signed: bool) -> Result<Scalar, FrontendError> {
    Ok(
        match (
            native.get_sizeof().map_err(|_| unsupported(native))?,
            signed,
        ) {
            (1, true) => Scalar::I8,
            (1, false) => Scalar::U8,
            (2, true) => Scalar::I16,
            (2, false) => Scalar::U16,
            (4, true) => Scalar::I32,
            (4, false) => Scalar::U32,
            (8, true) => Scalar::I64,
            (8, false) => Scalar::U64,
            _ => return Err(unsupported(native)),
        },
    )
}

fn unsupported(native: CppType<'_>) -> FrontendError {
    FrontendError(format!(
        "unsupported C++ type: {} ({:?})",
        native.get_display_name(),
        native.get_kind()
    ))
}
