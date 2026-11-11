use std::collections::BTreeMap;

use intrlay_layout::Layout;
use serde::{Deserialize, Serialize};

/// Primitive values with an explicitly defined encoded width.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scalar {
    /// One-byte boolean, zero or one.
    Bool,
    /// Signed eight-bit integer.
    I8,
    /// Unsigned eight-bit integer.
    U8,
    /// Signed sixteen-bit integer.
    I16,
    /// Unsigned sixteen-bit integer.
    U16,
    /// Signed thirty-two-bit integer.
    I32,
    /// Unsigned thirty-two-bit integer.
    U32,
    /// Signed sixty-four-bit integer.
    I64,
    /// Unsigned sixty-four-bit integer.
    U64,
    /// IEEE-754 binary32.
    F32,
    /// IEEE-754 binary64.
    F64,
}

impl Scalar {
    /// Returns the scalar's portable layout, independent of the host ABI.
    #[must_use]
    pub const fn layout(self) -> Layout {
        let size = match self {
            Self::Bool | Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        };
        Layout::new(size, size)
    }
}

/// A field's semantic type; native pointers and containers are never encoded as structs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Type {
    /// A fixed-width primitive.
    Scalar(Scalar),
    /// A reference to a definition in the same schema.
    Named(String),
    /// A fixed-count inline array.
    Array {
        /// The type of each inline element.
        element: Box<Self>,
        /// Number of elements, independent of the runtime value.
        length: usize,
    },
    /// A relative offset and element count followed by fixed-stride element headers.
    Sequence(Box<Self>),
    /// A relative offset and UTF-8 byte count.
    String,
    /// A presence byte and an aligned, fixed-size inline value header.
    Optional(Box<Self>),
    /// A relative offset to one separately stored value.
    Boxed(Box<Self>),
    /// A zero-sized unit value.
    Unit,
}

/// A generated binding language, independent of the schema's source language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    /// Native Rust views.
    Rust,
    /// C++20 views.
    Cpp,
    /// JavaScript typed-array views with TypeScript declarations.
    TypeScript,
    /// C# memory views.
    CSharp,
    /// Python buffer-protocol views.
    Python,
    /// Dart typed-data views.
    Dart,
}

impl Language {
    /// Whether record and wrapper values can be used without allocating a reference object.
    /// Projections for reducing per-element objects apply only to other targets.
    #[must_use]
    pub const fn has_value_types(self) -> bool {
        matches!(self, Self::Rust | Self::Cpp | Self::CSharp)
    }

    /// Whether getter/setter emission can be trimmed to reduce generated source.
    /// Native value-type backends always emit their complete accessor API.
    #[must_use]
    pub const fn supports_accessor_generation(self) -> bool {
        !self.has_value_types()
    }
}

/// API shape for a value or collection; this never changes its encoded layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "scalar", rename_all = "snake_case")]
pub enum Exposure {
    /// Named field access, with typed child views.
    #[default]
    Fields,
    /// Inline record and fixed-array paths become accessors on the containing view.
    /// Each leaf retains its own type; this does not reinterpret numeric lanes.
    Flatten,
    /// One numeric view over the complete representation, including padding.
    TypedArray(Scalar),
    /// Record collection field access alongside ordinary element views.
    Indexed,
    /// Request field access only; a shared collection view retains element access
    /// if another reachable use needs it.
    IndexedOnly,
}

/// Which APIs to emit, independently of storage mutability or access permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiGeneration {
    /// Emit getters and collection read helpers in TypeScript, Python, and Dart.
    pub getters: bool,
    /// Emit setters for fixed-shape values in TypeScript, Python, and Dart.
    pub setters: bool,
    /// Generate size calculation and construction APIs.
    pub create: bool,
}

impl Default for ApiGeneration {
    fn default() -> Self {
        Self {
            getters: true,
            setters: true,
            create: true,
        }
    }
}

/// Target-specific API choices, separate from the binary representation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetOptions {
    /// Explicit target-language type or field name, before automatic casing.
    pub name: Option<String>,
    /// Projection override; absent fields inherit the referenced type's choice.
    pub exposure: Option<Exposure>,
    /// Emit getters in TypeScript, Python, and Dart, inheriting when absent.
    /// Other backends ignore this choice; returned buffers remain ordinary buffers.
    pub generate_getters: Option<bool>,
    /// Emit setters in TypeScript, Python, and Dart, inheriting when absent.
    /// This does not restrict writes through a returned child view or numeric array.
    pub generate_setters: Option<bool>,
    /// Emit numeric-lane helpers on ordinary fields and indexed collections.
    /// Getters/setters still follow their respective emission hints. Defaults to false.
    pub generate_component_accessors: Option<bool>,
    /// Emit byte offset and byte length helpers for each field's inline wire layout.
    /// Offsets are relative to the view's storage, and helpers allocate no child view.
    pub generate_byte_range_accessors: Option<bool>,
    /// Emit whole-value field and element accessors, independently of numeric helpers.
    /// Defaults to true; read/write emission hints still apply to every helper family.
    pub generate_value_accessors: Option<bool>,
    /// Whether to emit size calculation and construction APIs, inheriting when absent.
    pub create: Option<bool>,
}

/// One declaration-ordered field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    /// Source field name; tuple members use their numeric index.
    pub name: String,
    /// Semantic type after applying source adapters.
    pub ty: Type,
    /// Documentation copied to generated APIs.
    pub docs: String,
    /// Optional API overrides for this use of the type.
    pub targets: BTreeMap<Language, TargetOptions>,
}

impl Field {
    /// Creates a field with default API exposure and no documentation.
    #[must_use]
    pub fn new(name: impl Into<String>, ty: Type) -> Self {
        Self {
            name: name.into(),
            ty,
            docs: String::new(),
            targets: BTreeMap::new(),
        }
    }
}

/// One tagged enum variant and its declaration-ordered payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variant {
    /// Source variant name.
    pub name: String,
    /// Stable encoded discriminant.
    pub tag: u32,
    /// Variant-specific fields.
    pub fields: Vec<Field>,
    /// The sole field wraps a native alternative rather than a source member.
    ///
    /// C++ `std::variant` alternatives retain their own type as this field's
    /// payload. C++ inputs store it directly, Rust inputs use a tuple variant,
    /// and views use the alternative's name without the synthetic field suffix.
    /// This metadata does not change the encoded layout or field name.
    #[serde(default)]
    pub synthetic_payload: bool,
    /// Source documentation.
    pub docs: String,
    /// Per-language names and payload accessor generation choices.
    #[serde(default)]
    pub targets: BTreeMap<Language, TargetOptions>,
}

/// Definition of a named record or enum.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum DefinitionKind {
    /// Inline fields retaining source declaration order.
    Record(Vec<Field>),
    /// A `u32` tag followed by storage for the largest variant header.
    Enum(Vec<Variant>),
}

/// A source-independent named type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    /// Public name used for generated declarations.
    pub name: String,
    /// Source documentation.
    pub docs: String,
    /// Source-independent fields or variants.
    pub kind: DefinitionKind,
    /// Explicit minimum alignment; one means natural field alignment.
    pub alignment: usize,
    /// Per-language accessor generation and exposure choices.
    pub targets: BTreeMap<Language, TargetOptions>,
    /// Expose the sole field directly in reference-type target languages.
    /// Value-type backends retain the wrapper; the encoded layout is unchanged.
    #[serde(default)]
    pub transparent: bool,
}

/// Complete exported type graph; recursive edges may only cross indirection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    /// Named export entry points, each referencing an encoded type.
    pub exports: BTreeMap<String, Type>,
    /// Definitions keyed by source identity, not by their potentially shared short names.
    pub definitions: BTreeMap<String, Definition>,
}

impl Schema {
    /// Serializes generator metadata, not application payloads.
    ///
    /// # Errors
    /// Returns an error if metadata cannot be represented by the JSON serializer.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parses a schema exported by a source frontend.
    ///
    /// # Errors
    /// Returns an error for malformed metadata.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
