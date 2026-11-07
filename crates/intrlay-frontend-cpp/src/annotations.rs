use std::collections::{BTreeMap, BTreeSet};

use clang::{Entity, EntityKind};
use intrlay_schema::{Exposure, Language, Scalar, TargetOptions};

use crate::FrontendError;

const LANGUAGES: [Language; 6] = [
    Language::Rust,
    Language::Cpp,
    Language::TypeScript,
    Language::CSharp,
    Language::Python,
    Language::Dart,
];

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Annotation {
    pub export: bool,
    pub name: Option<String>,
    pub alignment: Option<usize>,
    pub targets: BTreeMap<Language, TargetOptions>,
    pub transparent: bool,
    pub alternatives: BTreeMap<usize, String>,
}

impl Annotation {
    pub fn parse(entity: Entity<'_>) -> Result<Self, FrontendError> {
        let annotations: Vec<_> = entity
            .get_children()
            .into_iter()
            .filter(|child| child.get_kind() == EntityKind::AnnotateAttr)
            .filter_map(|attribute| attribute.get_display_name())
            .collect();
        Self::parse_text(annotations.iter().map(String::as_str), entity.get_kind())
    }

    fn parse_text<'a>(
        texts: impl Iterator<Item = &'a str>,
        scope: EntityKind,
    ) -> Result<Self, FrontendError> {
        let mut result = Self::default();
        let mut common = TargetOptions::default();
        let mut seen = BTreeSet::new();
        for text in texts {
            let Some(text) = text.strip_prefix("intrlay:") else {
                continue;
            };
            result.export = true;
            for option in split_options(text)? {
                if let Some(arguments) = option
                    .strip_prefix("alternative")
                    .and_then(|text| text.trim_start().strip_prefix('('))
                {
                    result.parse_alternative(arguments, scope)?;
                    continue;
                }
                let (key, value) = option
                    .split_once('=')
                    .map_or((option, "true"), |(key, value)| {
                        (key.trim(), value.trim().trim_matches('"'))
                    });
                let (language, key) = if let Some((language, key)) = key.split_once('.') {
                    (Some(parse_language(language)?), key)
                } else {
                    (None, key)
                };
                check_scope(scope, key, language)?;
                if !seen.insert((language, key.to_owned())) {
                    return Err(FrontendError(format!(
                        "duplicate or conflicting {key} annotation"
                    )));
                }
                if language.is_none() {
                    match key {
                        "name" => result.name = Some(value.into()),
                        "align" => {
                            let alignment: usize = value.parse().map_err(|error| {
                                FrontendError(format!("invalid alignment: {error}"))
                            })?;
                            if !alignment.is_power_of_two() {
                                return Err(FrontendError(
                                    "alignment must be a power of two".into(),
                                ));
                            }
                            result.alignment = Some(alignment);
                            continue;
                        }
                        "transparent" => {
                            result.transparent = boolean(value)?;
                            continue;
                        }
                        _ => {}
                    }
                }
                let target = if let Some(language) = language {
                    result.targets.entry(language).or_default()
                } else {
                    &mut common
                };
                parse_option(target, key, value)?;
            }
        }
        // Language-specific annotations override common defaults independent of order.
        for language in LANGUAGES {
            let target = result.targets.entry(language).or_default();
            target.name = target.name.take().or_else(|| common.name.clone());
            if !language.has_value_types() {
                target.exposure = common.exposure;
            }
            if language.supports_accessor_generation() {
                target.generate_getters = target.generate_getters.or(common.generate_getters);
                target.generate_setters = target.generate_setters.or(common.generate_setters);
            }
            target.create = target.create.or(common.create);
        }
        result
            .targets
            .retain(|_, value| *value != TargetOptions::default());
        Ok(result)
    }

    fn parse_alternative(
        &mut self,
        arguments: &str,
        scope: EntityKind,
    ) -> Result<(), FrontendError> {
        if !matches!(scope, EntityKind::TypeAliasDecl | EntityKind::TypedefDecl) {
            return Err(FrontendError(
                "alternative names must be declared on a std::variant alias".into(),
            ));
        }
        let arguments = arguments
            .strip_suffix(')')
            .ok_or_else(|| FrontendError("unterminated alternative annotation".into()))?;
        let arguments = split_options(arguments)?;
        let [index, name] = arguments.as_slice() else {
            return Err(FrontendError(
                "expected alternative(index, name = \"Name\")".into(),
            ));
        };
        let index = index
            .parse::<usize>()
            .map_err(|error| FrontendError(format!("invalid alternative index: {error}")))?;
        let name = name
            .split_once('=')
            .filter(|(key, _)| key.trim() == "name")
            .and_then(|(_, value)| value.trim().strip_prefix('"'))
            .and_then(|value| value.strip_suffix('"'))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| FrontendError("alternative requires a nonempty name string".into()))?;
        if self.alternatives.insert(index, name.into()).is_some() {
            return Err(FrontendError(format!(
                "duplicate alternative name annotation at index {index}"
            )));
        }
        Ok(())
    }
}

fn parse_language(value: &str) -> Result<Language, FrontendError> {
    match value {
        "rust" => Ok(Language::Rust),
        "cpp" => Ok(Language::Cpp),
        "typescript" => Ok(Language::TypeScript),
        "csharp" => Ok(Language::CSharp),
        "python" => Ok(Language::Python),
        "dart" => Ok(Language::Dart),
        _ => Err(FrontendError(format!("unknown target language: {value}"))),
    }
}

fn parse_option(target: &mut TargetOptions, key: &str, value: &str) -> Result<(), FrontendError> {
    match key {
        "name" => target.name = Some(value.into()),
        "generate_getters" => target.generate_getters = Some(boolean(value)?),
        "generate_setters" => target.generate_setters = Some(boolean(value)?),
        "create" => target.create = Some(boolean(value)?),
        "project" => target.exposure = Some(projection(value)?),
        _ => return Err(FrontendError(format!("unknown Intrlay annotation: {key}"))),
    }
    Ok(())
}

fn projection(value: &str) -> Result<Exposure, FrontendError> {
    Ok(match value {
        "fields" => Exposure::Fields,
        "indexed" => Exposure::Indexed,
        _ => Exposure::TypedArray(match value {
            "i8" => Scalar::I8,
            "u8" => Scalar::U8,
            "i16" => Scalar::I16,
            "u16" => Scalar::U16,
            "i32" => Scalar::I32,
            "u32" => Scalar::U32,
            "i64" => Scalar::I64,
            "u64" => Scalar::U64,
            "f32" => Scalar::F32,
            "f64" => Scalar::F64,
            _ => {
                return Err(FrontendError(format!(
                    "unsupported projection: {value}; expected fields, indexed, or a numeric scalar (i8, u8, i16, u16, i32, u32, i64, u64, f32, f64)"
                )));
            }
        }),
    })
}

fn boolean(value: &str) -> Result<bool, FrontendError> {
    value
        .parse()
        .map_err(|error| FrontendError(format!("invalid boolean: {error}")))
}

fn check_scope(
    scope: EntityKind,
    key: &str,
    language: Option<Language>,
) -> Result<(), FrontendError> {
    if matches!(key, "generate_getters" | "generate_setters")
        && language.is_some_and(|language| !language.supports_accessor_generation())
    {
        return Err(FrontendError(
            "accessor generation options apply only to typescript, python, and dart".into(),
        ));
    }
    if key == "project" && language.is_some() {
        return Err(FrontendError(
            "project is a common option; omit the language prefix".into(),
        ));
    }
    let valid = match scope {
        // Aliases export concrete instantiations; they do not define new types.
        EntityKind::TypeAliasDecl | EntityKind::TypedefDecl => key == "name" && language.is_none(),
        EntityKind::FieldDecl => !matches!(key, "align" | "transparent" | "create"),
        EntityKind::EnumConstantDecl => {
            matches!(key, "name" | "generate_getters" | "generate_setters")
        }
        EntityKind::EnumDecl => key != "transparent",
        _ => true,
    };
    if !valid {
        return Err(FrontendError(format!(
            "{key} is not supported at this annotation location"
        )));
    }
    Ok(())
}

/// Commas inside strings or nested options do not separate outer options.
fn split_options(text: &str) -> Result<Vec<&str>, FrontendError> {
    let mut result = Vec::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0_usize;
    let mut start = 0;
    for (index, character) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    FrontendError("unexpected closing parenthesis in annotation".into())
                })?;
            }
            ',' if !quoted && depth == 0 => {
                result.push(text[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    if quoted {
        return Err(FrontendError("unterminated annotation string".into()));
    }
    if depth != 0 {
        return Err(FrontendError("unterminated annotation options".into()));
    }
    result.push(text[start..].trim());
    result.retain(|value| !value.is_empty());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_options_override_defaults_in_any_order() -> Result<(), FrontendError> {
        for language in ["typescript", "python", "dart"] {
            for text in [
                format!(
                    "intrlay:{language}.name=custom,name=common,{language}.generate_setters=true,generate_setters=false"
                ),
                format!(
                    "intrlay:name=common,generate_setters=false,{language}.name=custom,{language}.generate_setters=true"
                ),
            ] {
                let annotation =
                    Annotation::parse_text([text.as_str()].into_iter(), EntityKind::StructDecl)?;
                let options = &annotation.targets[&parse_language(language)?];
                assert_eq!(options.name.as_deref(), Some("custom"));
                assert_eq!(options.generate_setters, Some(true));
            }
        }
        Ok(())
    }

    #[test]
    fn common_accessor_options_apply_only_to_reference_targets() -> Result<(), FrontendError> {
        let annotation = Annotation::parse_text(
            ["intrlay:generate_getters=false,generate_setters=false"].into_iter(),
            EntityKind::StructDecl,
        )?;
        for language in LANGUAGES {
            if language.supports_accessor_generation() {
                assert_eq!(annotation.targets[&language].generate_getters, Some(false));
                assert_eq!(annotation.targets[&language].generate_setters, Some(false));
            } else {
                assert!(!annotation.targets.contains_key(&language));
            }
        }
        Ok(())
    }

    #[test]
    fn common_projection_preserves_value_type_targets() -> Result<(), FrontendError> {
        for (value, exposure) in [
            ("fields", Exposure::Fields),
            ("indexed", Exposure::Indexed),
            ("f32", Exposure::TypedArray(Scalar::F32)),
        ] {
            for scope in [EntityKind::StructDecl, EntityKind::FieldDecl] {
                let text = format!("intrlay:project=\"{value}\"");
                let annotation = Annotation::parse_text([text.as_str()].into_iter(), scope)?;
                for language in LANGUAGES {
                    if language.has_value_types() {
                        assert!(!annotation.targets.contains_key(&language));
                    } else {
                        assert_eq!(annotation.targets[&language].exposure, Some(exposure));
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn rejects_ignored_or_conflicting_options() {
        for text in [
            "intrlay:generate_setters=false,generate_setters=true",
            "intrlay:project=fields,project=indexed",
            "intrlay:project=bool",
            "intrlay:project=quaternion",
            "intrlay:rust.project=f32",
            "intrlay:cpp.project=f32",
            "intrlay:csharp.project=f32",
            "intrlay:typescript.project=f32",
            "intrlay:python.project=f32",
            "intrlay:dart.project=f32",
            "intrlay:typescript.fields,typescript.indexed",
            "intrlay:typescript.typed_array=f32",
            "intrlay:python.typed_array=f32",
            "intrlay:dart.typed_array=f32",
            "intrlay:align=3",
            "intrlay:unknown=true",
            "intrlay:rust.typed_array=f32",
            "intrlay:cpp.typed_array=f32",
            "intrlay:csharp.typed_array=f32",
            "intrlay:cpp.indexed",
            "intrlay:csharp.indexed",
            "intrlay:typed_array=f32",
            "intrlay:read=false",
            "intrlay:write=false",
            "intrlay:rust.generate_getters=false",
            "intrlay:cpp.generate_setters=false",
            "intrlay:csharp.generate_setters=false",
        ] {
            assert!(
                Annotation::parse_text([text].into_iter(), EntityKind::StructDecl).is_err(),
                "{text}"
            );
        }
        assert!(
            Annotation::parse_text(["intrlay:transparent"].into_iter(), EntityKind::FieldDecl)
                .is_err()
        );
        for text in [
            "intrlay:generate_setters=false",
            "intrlay:typescript.name=Alias",
            "intrlay:transparent",
            "intrlay:project=fields",
        ] {
            assert!(Annotation::parse_text([text].into_iter(), EntityKind::TypeAliasDecl).is_err());
        }
        assert!(
            Annotation::parse_text(
                ["intrlay:project=indexed"].into_iter(),
                EntityKind::EnumConstantDecl,
            )
            .is_err()
        );
    }

    #[test]
    fn nested_alternative_options_keep_their_own_commas() -> Result<(), FrontendError> {
        let annotation = Annotation::parse_text(
            ["intrlay:name=Range,alternative(0, name = \"Start\"), alternative (1, name = \"End\")"].into_iter(),
            EntityKind::TypeAliasDecl,
        )?;
        assert_eq!(annotation.name.as_deref(), Some("Range"));
        assert_eq!(annotation.alternatives[&0], "Start");
        assert_eq!(annotation.alternatives[&1], "End");
        for text in [
            "intrlay:alternative(0, name = \"Start\"),alternative(0, name = \"End\")",
            "intrlay:alternative(-1, name = \"Start\")",
            "intrlay:alternative(0, name = \"\")",
            "intrlay:alternative(0, tag = 1)",
            "intrlay:alternative(0, name = \"Start\"",
            "intrlay:alternative(0, name = \"Start\"))",
        ] {
            assert!(
                Annotation::parse_text([text].into_iter(), EntityKind::TypeAliasDecl).is_err(),
                "{text}"
            );
        }
        assert!(
            Annotation::parse_text(
                ["intrlay:alternative(0, name = \"Start\")"].into_iter(),
                EntityKind::FieldDecl,
            )
            .is_err()
        );
        Ok(())
    }
}
