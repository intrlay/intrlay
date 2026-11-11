//! Optional integrations for source-language library types.
//!
//! Enable the matching Cargo feature and select a codec with `#[intrlay(with = ...)]`.
//! The original fields retain their library types. `smol_str` and `smallvec` retain the ordinary
//! string and sequence representations; their native storage strategy is not
//! part of the schema. `glam` exposes named fields and explicit aligned layouts.

#[cfg(any(feature = "glam-0-33", feature = "glam-0-34"))]
#[path = "glam/size.rs"]
mod glam_size;

#[cfg(any(feature = "glam-0-33", feature = "glam-0-34"))]
macro_rules! glam_adapters {
    ($module:ident, $native:ident, $version:literal, $feature:literal) => {
        #[doc = concat!("Codecs for glam ", $version, ". Enable `", $feature, "` to use these native types.")]
        pub mod $module {
            use $native as native;
            include!("glam.rs");
        }
    };
}

#[cfg(feature = "glam-0-33")]
glam_adapters!(glam_0_33, glam_0_33, "0.33", "glam-0-33");
#[cfg(feature = "glam-0-34")]
glam_adapters!(glam_0_34, glam_0_34, "0.34", "glam-0-34");
#[cfg(feature = "smallvec")]
pub mod smallvec;
#[cfg(feature = "smol_str")]
pub mod smol_str;
