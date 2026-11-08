//! Module-wide configuration, separate from resolved types and wire layout.

use serde::{Deserialize, Serialize};

use crate::{CSharpOptions, CppOptions, NamingPolicy};

/// Settings applied to the entire output module.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GenerateOptions {
    /// Stable name of this generation unit, independent of its exported types.
    /// Defaults to `Schema`. C++/C# derive their default namespace from this name.
    pub module_name: Option<String>,
    /// Override the output language's conventional naming policy.
    pub naming: Option<NamingPolicy>,
    /// C++ declaration settings, interpreted only by the C++ backend.
    pub cpp: CppOptions,
    /// C# declaration settings, interpreted only by the C# backend.
    pub csharp: CSharpOptions,
}

impl GenerateOptions {
    pub(crate) fn module_name(&self) -> &str {
        self.module_name.as_deref().unwrap_or("Schema")
    }
}
