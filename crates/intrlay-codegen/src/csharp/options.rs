use serde::{Deserialize, Serialize};

use crate::{GenerateError, names::Names};

/// Configuration for a generated C# module.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CSharpOptions {
    /// Full namespace, such as `App.Protocol`.
    /// Defaults to `IntrlayGenerated.<ModuleName>` using the naming policy.
    pub namespace: Option<String>,
    /// Minimum C# language version available to the generated module.
    /// Defaults to 14. Versions 15 and later add non-boxing union patterns to
    /// payload-enum inputs and require the .NET 11 union interfaces.
    pub language_version: Option<u16>,
}

/// Resolved settings used by both C# name validation and metadata emission.
pub(super) struct Context {
    pub namespace: String,
    pub language_version: u16,
}

impl Context {
    pub fn new(
        options: &CSharpOptions,
        module_name: &str,
        names: &Names,
    ) -> Result<Self, GenerateError> {
        let namespace = options
            .namespace
            .clone()
            .unwrap_or_else(|| format!("IntrlayGenerated.{}", names.type_name(module_name)));
        super::name_validation::namespace(&namespace)?;
        let language_version = options.language_version.unwrap_or(14);
        if language_version < 14 {
            return Err(GenerateError(format!(
                "C# {language_version} is unsupported; generated bindings require C# 14 or later"
            )));
        }
        Ok(Self {
            namespace,
            language_version,
        })
    }
}
