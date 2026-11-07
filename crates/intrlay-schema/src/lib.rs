//! Language-independent definitions used by every Intrlay source and output backend.
//!
//! Metadata is collected explicitly during generation. Reading an encoded object
//! never requires this schema or a runtime reflection registry.

#![forbid(unsafe_code)]

mod builtins;
mod layout;
mod names;
mod registry;
mod types;

pub use layout::{LayoutError, Layouts, RecordLayout, VariantLayout};
pub use names::NameCase;
pub use registry::{Describe, Registry};
pub use types::{
    ApiGeneration, Definition, DefinitionKind, Exposure, Field, Language, Scalar, Schema,
    TargetOptions, Type, Variant,
};
