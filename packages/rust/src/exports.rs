/// Declares the concrete types exported by `intrlay generate --manifest-path`.
///
/// Invoke once at the library crate root. An entry without `=` uses that type's
/// identifier as its export name. `Name = Type` also supports qualified paths,
/// primitives, aliases, and concrete generic specializations. Entry attributes
/// such as `#[cfg(feature = "extra")]` follow the crate's normal compilation.
/// Raw identifiers retain their type resolution but omit `r#` from export names.
///
/// The macro generates a metadata entry point for the CLI's temporary collector.
/// It performs no registration or file writes during normal application startup.
///
/// ```
/// #[derive(intrlay::Intrlay)]
/// struct Command {
///     id: u32,
/// }
///
/// #[derive(intrlay::Intrlay)]
/// struct Message<T> {
///     value: T,
/// }
///
/// intrlay::export_types! {
///     Command,
///     IntegerMessage = Message<i32>,
/// }
/// ```
#[macro_export]
macro_rules! export_types {
    ($( $(#[$attribute:meta])* $name:ident $(= $ty:ty)? ),* $(,)?) => {
        #[doc(hidden)]
        #[must_use]
        pub fn __intrlay_export_schema() -> $crate::schema::Schema {
            #[allow(unused_mut, reason = "all export entries may be disabled by cfg")]
            let mut registry = $crate::schema::Registry::new();
            $(
                $(#[$attribute])*
                {
                    $crate::export_types!(@register registry, $name $(= $ty)?);
                }
            )*
            registry.finish()
        }
    };
    (@register $registry:ident, $name:ident = $ty:ty) => {
        $registry.export::<$ty>(
            ::core::stringify!($name)
                .strip_prefix("r#")
                .unwrap_or(::core::stringify!($name)),
        );
    };
    (@register $registry:ident, $name:ident) => {
        $crate::export_types!(@register $registry, $name = $name);
    };
}
