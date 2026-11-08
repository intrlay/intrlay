use serde::{Deserialize, Serialize};

use crate::GenerateError;

/// Configuration for a generated C++ module.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CppOptions {
    /// Full namespace, such as `app::protocol`.
    /// Defaults to `intrlay_generated::<module_name>` using snake case.
    pub namespace: Option<String>,
}

/// Resolved module settings shared by C++ emission helpers.
pub(super) struct Context {
    pub namespace: String,
}

impl Context {
    pub fn new(options: &CppOptions, module_name: &str) -> Result<Self, GenerateError> {
        let namespace = options.namespace.clone().unwrap_or_else(|| {
            format!(
                "intrlay_generated::{}",
                intrlay_schema::NameCase::SnakeCase.apply(module_name)
            )
        });
        super::name_validation::namespace(&namespace)?;
        Ok(Self { namespace })
    }

    /// Schema names may also occur as fields or template parameters.
    pub fn qualified(&self, name: &str) -> String {
        format!("::{}::{name}", self.namespace)
    }
}
