#pragma once
#include <concepts>
#include <type_traits>
#include <utility>

namespace intrlay {

/// Maps a native type to a portable schema and a repeatable encoding source.
/// Specializations provide schema_type and source(const T&). The source may
/// borrow the original value, or return a small value/proxy; it must remain
/// stable through measurement and writing and must not alias destination bytes.
/// Containers in schema_type recursively use their elements' adapters.
template<class T>
struct Adapter {
    using schema_type = T;

    /// Allows native-copy proofs to inspect the original object. A specialization
    /// may opt in only if source returns that exact, unchanged object by reference.
    /// Size, field offsets, padding, and scalar layouts are still checked separately.
    static constexpr bool preserves_native_layout = true;

    template<class V>
        requires std::same_as<std::remove_cvref_t<V>, T>
    static constexpr decltype(auto) source(V&& value) noexcept {
        return std::forward<V>(value);
    }
};

namespace detail {
// The frontend instantiates this alias for concrete reachable types. Resolving
// schema chains in C++ honors specialization and template-depth diagnostics.
template<class T, class... Seen>
struct AdapterSchema {
    static constexpr bool repeated = (std::same_as<T, Seen> || ...);
    static_assert(!repeated, "cyclic adapter schema_type");
    using Next = std::remove_cv_t<typename Adapter<T>::schema_type>;
    using Tail = std::conditional_t<std::same_as<T, Next> || repeated,
        std::type_identity<T>, AdapterSchema<Next, Seen..., T>>;
    using type = typename Tail::type;
};

template<class T>
using SourceValue = std::remove_const_t<std::remove_reference_t<T>>;

template<class T>
struct AdapterSourceIdentity {
    using type = T;
    static constexpr bool valid = true;
    static constexpr bool is_nothrow = true;
};

template<class T>
concept HasAdapterSource = requires(const T& value) {
    typename Adapter<std::remove_cv_t<T>>::schema_type;
    Adapter<std::remove_cv_t<T>>::source(value);
    requires (!std::is_void_v<typename Adapter<std::remove_cv_t<T>>::schema_type>);
    requires (!std::is_void_v<decltype(Adapter<std::remove_cv_t<T>>::source(value))>);
};

// Resolve only adapter-to-adapter edges here. Ordinary recursive trees are
// handled independently by the codec/source-pair check in codecs.hpp.
template<class T, class... Seen>
struct AdapterSource {
    using type = void;
    static constexpr bool valid = false;
    static constexpr bool is_nothrow = false;
};

template<class T, class... Seen>
    requires HasAdapterSource<T> && (!(std::same_as<T, Seen> || ...))
struct AdapterSource<T, Seen...> {
    using Next = SourceValue<decltype(Adapter<std::remove_cv_t<T>>::source(std::declval<const T&>()))>;
    using Tail = std::conditional_t<std::same_as<T, Next>, std::type_identity<T>, AdapterSource<Next, Seen..., T>>;
    using type = typename Tail::type;
    static constexpr bool valid = !std::is_void_v<type>;
    // A value-returning adapter also destroys its intermediate source after writing.
    static constexpr bool is_nothrow = [] {
        using Result = decltype(Adapter<std::remove_cv_t<T>>::source(std::declval<const T&>()));
        constexpr bool local = noexcept(Adapter<std::remove_cv_t<T>>::source(std::declval<const T&>()))
            && (std::is_reference_v<Result> || std::is_nothrow_destructible_v<Result>);
        if constexpr (std::same_as<T, Next>) return local;
        else return local && Tail::is_nothrow;
    }();
};

template<class T>
inline constexpr bool adapter_preserves_native_layout = [] {
    using Value = std::remove_cvref_t<T>;
    using Mapping = Adapter<Value>;
    if constexpr (HasAdapterSource<Value> && requires { Mapping::preserves_native_layout; }) {
        return Mapping::preserves_native_layout
            && std::same_as<decltype(Mapping::source(std::declval<const Value&>())), const Value&>;
    } else return false;
}();

// Each intermediate source stays alive until the consumer finishes. Returning
// only the final reference would dangle when an adapter returns an owned proxy.
template<class T, class Consume>
    requires AdapterSource<SourceValue<T>>::valid
decltype(auto) with_adapter_source(const T& value, Consume&& consume) {
    using Mapping = Adapter<std::remove_cv_t<T>>;
    const auto& source = Mapping::source(value);
    if constexpr (std::same_as<SourceValue<T>, SourceValue<decltype(source)>>) {
        return std::forward<Consume>(consume)(source);
    } else {
        return with_adapter_source(source, std::forward<Consume>(consume));
    }
}
} // namespace detail
} // namespace intrlay

// C++20 variadic expansion: no runtime reflection or generated bridge header.
#define INTRLAY_DETAIL_EXPAND1(...) __VA_ARGS__
#define INTRLAY_DETAIL_EXPAND2(...) INTRLAY_DETAIL_EXPAND1(INTRLAY_DETAIL_EXPAND1(__VA_ARGS__))
#define INTRLAY_DETAIL_EXPAND4(...) INTRLAY_DETAIL_EXPAND2(INTRLAY_DETAIL_EXPAND2(__VA_ARGS__))
#define INTRLAY_DETAIL_EXPAND8(...) INTRLAY_DETAIL_EXPAND4(INTRLAY_DETAIL_EXPAND4(__VA_ARGS__))
#define INTRLAY_DETAIL_EXPAND16(...) INTRLAY_DETAIL_EXPAND8(INTRLAY_DETAIL_EXPAND8(__VA_ARGS__))
#define INTRLAY_DETAIL_EXPAND32(...) INTRLAY_DETAIL_EXPAND16(INTRLAY_DETAIL_EXPAND16(__VA_ARGS__))
#define INTRLAY_DETAIL_EXPAND64(...) INTRLAY_DETAIL_EXPAND32(INTRLAY_DETAIL_EXPAND32(__VA_ARGS__))
#define INTRLAY_DETAIL_PARENS ()
#define INTRLAY_DETAIL_FIELDS_AGAIN() INTRLAY_DETAIL_FIELDS
#define INTRLAY_DETAIL_FIELDS(member, ...) \
    ::std::remove_cvref_t<decltype(source_type::member)> member; \
    __VA_OPT__(INTRLAY_DETAIL_FIELDS_AGAIN INTRLAY_DETAIL_PARENS (__VA_ARGS__))

/// Defines Adapter<Native> at global namespace scope using public field names.
/// SchemaName names the generated record; listed fields determine wire order.
/// Use a type alias for Native when its spelling contains template commas.
#if defined(_MSVC_TRADITIONAL) && _MSVC_TRADITIONAL
#define INTRLAY_ADAPT_STRUCT(...) \
    static_assert(false, "INTRLAY_ADAPT_STRUCT requires the standard C++20 preprocessor; enable /Zc:preprocessor")
#else
#define INTRLAY_ADAPT_STRUCT(Native, SchemaName, ...) \
    template<> struct intrlay::Adapter<Native> { \
        using source_type = Native; \
        struct SchemaName { __VA_OPT__(INTRLAY_DETAIL_EXPAND64(INTRLAY_DETAIL_FIELDS(__VA_ARGS__))) }; \
        using schema_type = SchemaName; \
        static constexpr bool preserves_native_layout = true; \
        static constexpr const source_type& source(const source_type& value) noexcept { return value; } \
    }
#endif
