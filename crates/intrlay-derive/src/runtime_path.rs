use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use syn::{Ident, Path};

/// Resolves Cargo dependency renames; an explicit path also supports reexports.
pub fn resolve(explicit: Option<&Path>) -> syn::Result<Path> {
    if let Some(path) = explicit {
        return Ok(path.clone());
    }
    match crate_name("intrlay") {
        Ok(FoundCrate::Itself) => Ok(syn::parse_quote!(::intrlay)),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            Ok(syn::parse_quote!(::#ident))
        }
        Err(error) => Err(syn::Error::new(
            Span::call_site(),
            format!(
                "cannot locate the intrlay dependency: {error}; use #[intrlay(crate = \"path::to::intrlay\")] for reexports"
            ),
        )),
    }
}
