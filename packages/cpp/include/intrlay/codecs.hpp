#pragma once
#include "runtime.hpp"
#include <iterator>
#include <tuple>

namespace intrlay {

/// Requests zero initialization using the fixed array's schema length.
struct Zeroed {};

/// Requests sequence elements without allocating an intermediate collection.
struct ZeroedSequence {
    std::size_t length;
};

constexpr Zeroed zeroed() { return {}; }
constexpr ZeroedSequence zeroed(std::size_t length) { return {length}; }

namespace detail {
// Inspect only inline bytes. References end the traversal, including recursive
// schemas; their newly reserved payloads are initialized separately by writers.
template<class Codec>
consteval bool fully_writes_header(std::size_t size) {
    if constexpr (requires { Codec::fully_writes_header(size); }) return Codec::fully_writes_header(size);
    else return false;
}

template<class Codec>
inline constexpr bool zeroable = [] {
    if constexpr (requires { Codec::zeroable; }) return Codec::zeroable;
    else return false;
}();

// These are the integer spellings normalized by the C++ schema frontend.
// For example, long and int may denote the same i32 schema type on one ABI.
template<class T>
concept NativeSchemaInteger = std::same_as<T, char>
    || std::same_as<T, signed char> || std::same_as<T, unsigned char>
    || std::same_as<T, short> || std::same_as<T, unsigned short>
    || std::same_as<T, int> || std::same_as<T, unsigned int>
    || std::same_as<T, long> || std::same_as<T, unsigned long>
    || std::same_as<T, long long> || std::same_as<T, unsigned long long>;

template<class Source, class Schema>
concept SameScalarType = std::same_as<Source, Schema>
    || (NativeSchemaInteger<Source> && NativeSchemaInteger<Schema>
        && sizeof(Source) == sizeof(Schema)
        && std::numeric_limits<Source>::is_signed == std::numeric_limits<Schema>::is_signed
        && std::numeric_limits<Source>::digits == std::numeric_limits<Schema>::digits);

template<class Codec, class Source> struct SourcePair {};

template<class Codec>
inline constexpr bool adapts_source = [] {
    if constexpr (requires { Codec::adapts_source; }) return Codec::adapts_source;
    else return true;
}();

template<class Codec, class Source>
using CodecSource = std::conditional_t<adapts_source<Codec>, AdapterSource<SourceValue<Source>>,
    AdapterSourceIdentity<SourceValue<Source>>>;

// A repeated codec/source pair closes a recursive tree's type requirements.
// This inspects types only; it never visits runtime elements or allocates storage.
template<class Codec, class Source, class... Seen>
consteval bool source_compatible() {
    // Encoders read const sources, but volatile still changes which operations
    // are available (for example, std::string has no volatile data() overload).
    using Mapping = CodecSource<Codec, Source>;
    using Value = typename Mapping::type;
    using Pair = SourcePair<Codec, Value>;
    if constexpr (!Mapping::valid) return false;
    else if constexpr ((std::same_as<Pair, Seen> || ...)) return true;
    else if constexpr (requires { Codec::template accepts_source<Value, Seen..., Pair>(); })
        return Codec::template accepts_source<Value, Seen..., Pair>();
    else return false;
}

template<class V>
concept IndexedSourceRange = std::ranges::forward_range<const V>
    && std::ranges::sized_range<const V>
    && requires(const V& value, std::size_t index) {
        { std::size(value) } -> std::convertible_to<std::size_t>;
        value[index];
    };

template<class Element, class V, class... Seen>
consteval bool sequence_source() {
    if constexpr (IndexedSourceRange<V>) {
        return source_compatible<Element, decltype(std::declval<const V&>()[std::declval<std::size_t&>()]), Seen...>()
            && source_compatible<Element, std::ranges::range_reference_t<const V>, Seen...>();
    } else return false;
}

template<class T> struct VariantSource : std::false_type {};
template<class... T> struct VariantSource<std::variant<T...>> : std::true_type {};
} // namespace detail

/// Whether Source supplies every recursively required field/element for Codec.
/// Constraints are evaluated at compile time, before an encoder body is instantiated.
/// Containers and records may use different source representations; scalar leaves
/// must match their schema type, apart from cv/reference qualifications and
/// native integer spellings with the same width and signedness.
template<class Source, class Codec>
concept EncodeSourceFor = detail::source_compatible<Codec, Source>();

namespace detail {
template<class T> inline constexpr bool is_type_tuple = false;
template<class... T> inline constexpr bool is_type_tuple<std::tuple<T...>> = true;

template<class Sources, class Codecs>
    requires (is_type_tuple<Sources> && is_type_tuple<Codecs>)
consteval bool input_fields_compatible() {
    if constexpr (std::tuple_size_v<Sources> != std::tuple_size_v<Codecs>) return false;
    else return []<std::size_t... I>(std::index_sequence<I...>) {
        return (EncodeSourceFor<std::tuple_element_t<I, Sources>, std::tuple_element_t<I, Codecs>> && ...);
    }(std::make_index_sequence<std::tuple_size_v<Sources>>{});
}
} // namespace detail

/// Matches generated Input field sources against their positional schema codecs.
/// The empty pack selects the concrete owned-input specialization.
template<class Sources, class Codecs>
concept InputFieldsFor = detail::is_type_tuple<Sources> && detail::is_type_tuple<Codecs>
    && (std::tuple_size_v<Sources> == 0 || detail::input_fields_compatible<Sources, Codecs>());

/// Whether Source can update an existing descriptor value without relocating it.
/// This also covers a box/optional forwarding mutation to its existing payload.
template<class Source, class Descriptor>
concept SetSourceFor = requires(std::span<std::byte> bytes, std::size_t offset, const Source& value) {
    Descriptor::set(bytes, offset, value);
};

/// Shared construction entry points resolve adapters once per logical value.
/// Derived codecs consume the resolved source; their children repeat the same
/// operation for nested values. The default adapter is an inline identity call.
template<class Derived>
struct EncodingCodec {
    template<EncodeSourceFor<Derived> V>
    static void measure(const V& value, detail::Cursor& cursor) {
        if constexpr (detail::adapts_source<Derived>) {
            detail::with_adapter_source(value, [&](const auto& source) {
                Derived::measure_source(source, cursor);
            });
        } else Derived::measure_source(value, cursor);
    }
    template<EncodeSourceFor<Derived> V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor)
        noexcept(detail::CodecSource<Derived, V>::is_nothrow
            && noexcept(Derived::write_source(bytes, offset,
                std::declval<const typename detail::CodecSource<Derived, V>::type&>(), cursor))) {
        if constexpr (detail::adapts_source<Derived>) {
            detail::with_adapter_source(value, [&](const auto& source) {
                Derived::write_source(bytes, offset, source, cursor);
            });
        } else Derived::write_source(bytes, offset, value, cursor);
    }
};

/// Scalar codecs operate on bytes, without requiring aligned or live T objects.
/// T is the logical value type; Storage is its encoded primitive representation.
template<class T, class Storage = T>
struct ScalarCodec : EncodingCodec<ScalarCodec<T, Storage>> {
    static constexpr bool zeroable = std::is_arithmetic_v<T>;
    static consteval bool fully_writes_header(std::size_t size) { return size == sizeof(Storage); }
    static constexpr std::size_t fixed_write_size() { return sizeof(Storage); }
    template<class V, class...>
    static consteval bool accepts_source() {
        return detail::SameScalarType<std::remove_cv_t<V>, T>;
    }
    static constexpr bool fixed_layout() { return true; }
    template<EncodeSourceFor<ScalarCodec> V>
    static void measure_source(const V&, detail::Cursor&) {}
    template<EncodeSourceFor<ScalarCodec> V>
    static void write_source(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor&) noexcept {
        detail::store<Storage>(bytes, offset, static_cast<Storage>(value));
    }
};

struct UnitCodec : EncodingCodec<UnitCodec> {
    static constexpr bool zeroable = true;
    static consteval bool fully_writes_header(std::size_t size) { return size == 0; }
    static constexpr std::size_t fixed_write_size() { return 0; }
    template<class V, class...>
    static consteval bool accepts_source() { return std::same_as<V, std::monostate>; }
    static constexpr bool fixed_layout() { return true; }
    template<EncodeSourceFor<UnitCodec> V>
    static void measure_source(const V&, detail::Cursor&) {}
    template<EncodeSourceFor<UnitCodec> V>
    static void write_source(std::span<std::byte>, std::size_t, const V&, detail::Cursor&) noexcept {}
};

struct StringCodec : EncodingCodec<StringCodec> {
    static consteval bool fully_writes_header(std::size_t size) { return size == 8; }
    template<class V, class...>
    static consteval bool accepts_source() {
        return std::convertible_to<const V&, std::string_view>
            || requires(const V& value) { std::string_view(value.data(), value.size()); };
    }
    static constexpr bool fixed_layout() { return false; }
    static void measure_source(std::string_view value, detail::Cursor& cursor) {
        cursor.reserve(value.size(), 1);
    }
    static void write_source(std::span<std::byte> bytes, std::size_t offset, std::string_view value, detail::Cursor& cursor) {
        const auto payload = cursor.reserve(bytes, value.size(), 1);
        detail::reference(bytes, offset, payload);
        detail::store<std::uint32_t>(bytes, offset + 4, static_cast<std::uint32_t>(value.size()));
        if (!value.empty()) std::memcpy(bytes.data() + payload, value.data(), value.size());
    }

    // Character traits and allocators do not change the encoded character bytes.
    template<class V>
    requires requires(const V& value) { std::string_view(value.data(), value.size()); }
    static void measure_source(const V& value, detail::Cursor& cursor) {
        measure_source(std::string_view(value.data(), value.size()), cursor);
    }
    template<class V>
    requires requires(const V& value) { std::string_view(value.data(), value.size()); }
    static void write_source(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        write_source(bytes, offset, std::string_view(value.data(), value.size()), cursor);
    }
};

/// Get is a stateless compile-time accessor; Codec describes its wire value.
template<std::size_t Offset, class Codec, auto Get>
struct FieldCodec : EncodingCodec<FieldCodec<Offset, Codec, Get>> {
    // This is a projection of its containing record, not another encoded value.
    static constexpr bool adapts_source = false;
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        if constexpr (requires(const V& value) { Get(value); })
            return detail::source_compatible<Codec, decltype(Get(std::declval<const V&>())), Seen...>();
        else return false;
    }
    static constexpr bool fixed_layout() { return Codec::fixed_layout(); }
    static constexpr std::size_t fixed_write_size() { return Offset + Codec::fixed_write_size(); }
    template<EncodeSourceFor<FieldCodec> V>
    static void measure_source(const V& value, detail::Cursor& cursor) {
        Codec::measure(Get(value), cursor);
    }
    template<EncodeSourceFor<FieldCodec> V>
    static void write_source(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor)
        noexcept(noexcept(Codec::write(bytes, offset + Offset, Get(value), cursor))) {
        Codec::write(bytes, offset + Offset, Get(value), cursor);
    }
};

template<class... Fields>
struct RecordCodec : EncodingCodec<RecordCodec<Fields...>> {
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        return (detail::source_compatible<Fields, V, Seen...>() && ...);
    }
    static constexpr bool fixed_layout() { return (Fields::fixed_layout() && ...); }
    static constexpr std::size_t fixed_write_size() {
        return std::max({std::size_t{0}, Fields::fixed_write_size()...});
    }
    template<EncodeSourceFor<RecordCodec> V>
    static void measure_source(const V& value, detail::Cursor& cursor) {
        (Fields::measure_source(value, cursor), ...);
    }
    // Empty records have no field writes and do not consume these coordinates.
    template<EncodeSourceFor<RecordCodec> V>
    static void write_source([[maybe_unused]] std::span<std::byte> bytes,
                      [[maybe_unused]] std::size_t offset,
                      const V& INTRLAY_RESTRICT value, detail::Cursor& cursor)
        noexcept((noexcept(Fields::write_source(bytes, offset, value, cursor)) && ...)) {
        (Fields::write_source(bytes, offset, value, cursor), ...);
    }
};

/// Stride and alignment are supplied by the common layout implementation.
template<class Element, std::size_t Stride, std::size_t Alignment>
struct SequenceCodec : EncodingCodec<SequenceCodec<Element, Stride, Alignment>> {
    static consteval bool fully_writes_header(std::size_t size) { return size == 8; }
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        if constexpr (std::same_as<V, ZeroedSequence>) return detail::zeroable<Element>;
        else return detail::sequence_source<Element, V, Seen...>();
    }
    static constexpr bool fixed_layout() { return false; }
    template<EncodeSourceFor<SequenceCodec> V>
    static void measure_source(const V& value, detail::Cursor& cursor) {
        if constexpr (std::same_as<V, ZeroedSequence>) {
            cursor.reserve(detail::array_bytes(value.length, Stride), Alignment);
        } else {
            cursor.reserve(detail::array_bytes(std::size(value), Stride), Alignment);
            // Use the same customization points as the source constraint, including ADL ranges.
            const auto end = std::ranges::end(value);
            for (auto item = std::ranges::begin(value); item != end; ++item) {
                Element::measure(*item, cursor);
            }
        }
    }
    template<EncodeSourceFor<SequenceCodec> V>
    static void write_source(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        constexpr bool native_copy = [] {
            if constexpr (std::same_as<V, ZeroedSequence>) return false;
            else return std::ranges::contiguous_range<const V>
                && detail::native_layout<Element, std::ranges::range_reference_t<const V>>;
        }();
        constexpr bool clear_payload = std::same_as<V, ZeroedSequence>
            || (!native_copy && !detail::fully_writes_header<Element>(Stride));
        const std::size_t count = [&] {
            if constexpr (std::same_as<V, ZeroedSequence>) return value.length;
            else return static_cast<std::size_t>(std::size(value));
        }();
        // Initialize element headers together with any preceding alignment gap.
        // Their out-of-line data and native copies are written without clearing.
        const auto payload = cursor.reserve<clear_payload>(bytes, detail::array_bytes(count, Stride), Alignment);
        detail::reference(bytes, offset, payload);
        detail::store<std::uint32_t>(bytes, offset + 4, static_cast<std::uint32_t>(count));
        if constexpr (native_copy) {
            if (count != 0) std::memcpy(bytes.data() + payload, std::ranges::data(value), count * Stride);
        } else if constexpr (!std::same_as<V, ZeroedSequence>) {
            for (std::size_t i = 0; i < count; ++i) Element::write(bytes, payload + i * Stride, value[i], cursor);
        }
    }
};

template<class Element, std::size_t Stride, std::size_t Length>
struct ArrayCodec : EncodingCodec<ArrayCodec<Element, Stride, Length>> {
    static consteval bool fully_writes_header(std::size_t size) {
        if constexpr (Length == 0) return size == 0;
        else return size == Length * Stride && detail::fully_writes_header<Element>(Stride);
    }
    static constexpr std::size_t fixed_write_size() { return Length * Stride; }
    template<class V>
    static consteval bool nothrow_write() {
        if constexpr (std::same_as<V, Zeroed>) return true;
        else if constexpr (detail::NativeArray<V>::value) {
            return detail::NativeArray<V>::length == Length
                && noexcept(Element::write(std::declval<std::span<std::byte>>(), std::size_t{},
                    std::declval<const V&>()[0], std::declval<detail::Cursor&>()));
        } else return false;
    }
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        if constexpr (std::same_as<V, Zeroed>) return Length == 0 || detail::zeroable<Element>;
        else return detail::sequence_source<Element, V, Seen...>();
    }
    static constexpr bool fixed_layout() {
        if constexpr (Length == 0) return true;
        else return Element::fixed_layout();
    }
    template<EncodeSourceFor<ArrayCodec> V>
    static void measure_source(const V& value, detail::Cursor& cursor) {
        if constexpr (!std::same_as<V, Zeroed>) {
            const auto end = std::ranges::end(value);
            for (auto item = std::ranges::begin(value); item != end; ++item) {
                Element::measure(*item, cursor);
            }
        }
    }
    template<EncodeSourceFor<ArrayCodec> V>
    static void write_source(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor)
        noexcept(nothrow_write<V>()) {
        if constexpr (std::same_as<V, Zeroed>) {
            std::fill_n(bytes.data() + offset, Length * Stride, std::byte{});
        } else {
            if constexpr (!nothrow_write<V>()) {
                if (static_cast<std::size_t>(std::size(value)) != Length) throw std::length_error("fixed array length");
            }
            if constexpr (std::ranges::contiguous_range<const V>
                          && detail::native_layout<Element, std::ranges::range_reference_t<const V>>) {
                if constexpr (Length != 0) std::memcpy(bytes.data() + offset, std::ranges::data(value), Length * Stride);
            } else {
                for (std::size_t i = 0; i < Length; ++i) Element::write(bytes, offset + i * Stride, value[i], cursor);
            }
        }
    }
};

template<class Value, std::size_t PayloadOffset>
struct OptionalCodec : EncodingCodec<OptionalCodec<Value, PayloadOffset>> {
    static constexpr std::size_t fixed_write_size() {
        return std::max(std::size_t{1}, PayloadOffset + Value::fixed_write_size());
    }
    template<class V>
    static consteval bool nothrow_write() {
        if constexpr (std::same_as<std::remove_cv_t<V>, std::nullopt_t>) return true;
        else return noexcept(bool(std::declval<const V&>().has_value()))
            && noexcept(Value::write(std::declval<std::span<std::byte>>(), std::size_t{},
                *std::declval<const V&>(), std::declval<detail::Cursor&>()));
    }
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        if constexpr (std::same_as<std::remove_cv_t<V>, std::nullopt_t>) return true;
        else if constexpr (requires(const V& value) { { value.has_value() } -> std::convertible_to<bool>; *value; })
            return detail::source_compatible<Value, decltype(*std::declval<const V&>()), Seen...>();
        else return false;
    }
    static constexpr bool fixed_layout() { return Value::fixed_layout(); }
    template<EncodeSourceFor<OptionalCodec> V>
    static void measure_source(const V& value, detail::Cursor& cursor) {
        if constexpr (!std::same_as<std::remove_cv_t<V>, std::nullopt_t>) {
            if (value.has_value()) Value::measure(*value, cursor);
        }
    }
    template<EncodeSourceFor<OptionalCodec> V>
    static void write_source(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor)
        noexcept(nothrow_write<V>()) {
        if constexpr (std::same_as<std::remove_cv_t<V>, std::nullopt_t>) {
            detail::store<std::uint8_t>(bytes, offset, 0);
        } else {
            detail::store<std::uint8_t>(bytes, offset, value.has_value() ? 1 : 0);
            if (value.has_value()) Value::write(bytes, offset + PayloadOffset, *value, cursor);
        }
    }
};

template<class Value, std::size_t Size, std::size_t Alignment>
struct BoxCodec : EncodingCodec<BoxCodec<Value, Size, Alignment>> {
    static consteval bool fully_writes_header(std::size_t size) { return size == 4; }
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        if constexpr (requires(const V& value) { *value; })
            return detail::source_compatible<Value, decltype(*std::declval<const V&>()), Seen...>();
        else return false;
    }
    static constexpr bool fixed_layout() { return false; }
    template<EncodeSourceFor<BoxCodec> V>
    static void measure_source(const V& value, detail::Cursor& cursor) {
        cursor.reserve(Size, Alignment);
        Value::measure(*value, cursor);
    }
    template<EncodeSourceFor<BoxCodec> V>
    static void write_source(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        const auto payload = cursor.reserve<!detail::fully_writes_header<Value>(Size)>(bytes, Size, Alignment);
        detail::reference(bytes, offset, payload);
        Value::write(bytes, payload, *value, cursor);
    }
};

template<std::uint32_t Tag, class Payload, bool AdaptSource = true>
struct VariantCodec : Payload, EncodingCodec<VariantCodec<Tag, Payload, AdaptSource>> {
    using EncodingCodec<VariantCodec>::measure;
    using EncodingCodec<VariantCodec>::write;
    static constexpr bool adapts_source = AdaptSource;
    static constexpr std::uint32_t tag = Tag;
    static constexpr std::size_t fixed_write_size() {
        return std::max(sizeof(std::uint32_t), Payload::fixed_write_size());
    }
    template<class V>
        requires (Payload::template accepts_source<V>())
    static void write_source(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        detail::store<std::uint32_t>(bytes, offset, Tag);
        Payload::write_source(bytes, offset, value, cursor);
    }
};

template<class... Cases>
struct EnumCodec : EncodingCodec<EnumCodec<Cases...>> {
    static constexpr bool fixed_layout() { return (Cases::fixed_layout() && ...); }
    static constexpr std::size_t fixed_write_size() {
        return std::max({sizeof(std::uint32_t), Cases::fixed_write_size()...});
    }
private:
    template<class V, class... Seen, std::size_t... Index>
    static consteval bool variant_source(std::index_sequence<Index...>) {
        return (detail::source_compatible<std::tuple_element_t<Index, std::tuple<Cases...>>,
                    std::variant_alternative_t<Index, V>, Seen...>() && ...);
    }
    template<class V>
    static std::uint32_t native_tag(V value) {
        // Widen first: negative and wide values must not alias a wire tag.
        const auto tag = static_cast<std::uintmax_t>(value);
        if (((tag == Cases::tag) || ...)) return static_cast<std::uint32_t>(tag);
        throw std::invalid_argument("unknown enum tag");
    }

    template<std::size_t Index = 0, class V, class Visit>
    static void visit(const V& value, Visit&& visitor) {
        if constexpr (Index < sizeof...(Cases)) {
            if (value.index() == Index) {
                visitor.template operator()<std::tuple_element_t<Index, std::tuple<Cases...>>>(std::get<Index>(value));
            } else {
                visit<Index + 1>(value, std::forward<Visit>(visitor));
            }
        } else {
            throw std::bad_variant_access{};
        }
    }
public:
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        if constexpr (std::is_enum_v<V>) {
            // Native enums have no payload fields. A union-shaped schema must
            // still receive its active payload, not just a numeric tag.
            return ((Cases::field_count == 0) && ...);
        } else if constexpr (requires { detail::variant(std::declval<const V&>()); }) {
            using Variant = std::remove_const_t<std::remove_reference_t<decltype(
                detail::variant(std::declval<const V&>()))>>;
            if constexpr (detail::VariantSource<Variant>::value) {
                if constexpr (std::variant_size_v<Variant> == sizeof...(Cases))
                    return variant_source<Variant, Seen...>(std::index_sequence_for<Cases...>{});
                else return false;
            } else return false;
        } else return false;
    }
    template<EncodeSourceFor<EnumCodec> V>
    static void measure_source(const V& value, [[maybe_unused]] detail::Cursor& cursor) {
        if constexpr (std::is_enum_v<V>) {
            static_cast<void>(native_tag(value));
        } else {
            visit(detail::variant(value), [&]<class Case>(const auto& payload) {
                Case::measure(payload, cursor);
            });
        }
    }
    template<EncodeSourceFor<EnumCodec> V>
    static void write_source(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, [[maybe_unused]] detail::Cursor& cursor) {
        if constexpr (std::is_enum_v<V>) {
            detail::store<std::uint32_t>(bytes, offset, native_tag(value));
        } else {
            visit(detail::variant(value), [&]<class Case>(const auto& payload) {
                Case::write(bytes, offset, payload, cursor);
            });
        }
    }
};

namespace detail {
// Delay recursive descriptor inspection until the calling template's input is known.
// Some descriptors are only forward-declared while their parent is being defined.
template<class Codec, class Input>
inline constexpr bool fixed_encoding = Codec::fixed_layout();
}

/// A compatible input whose complete encoding fits the existing fixed layout.
/// Defer descriptor inspection until Source is known: recursive descriptors can
/// still be incomplete while their enclosing view is being declared.
template<class Source, class Codec>
concept FixedEncodeSourceFor = EncodeSourceFor<Source, Codec> && detail::fixed_encoding<Codec, Source>;

/// Fixed encodings need no measurement pass. Fallible writes are staged so an
/// exception cannot invalidate an existing view or change its previous value.
/// The input and its nested data must not overlap any destination bytes written.
template<class Codec, FixedEncodeSourceFor<Codec> T>
void replace(std::span<std::byte> bytes, std::size_t offset, const T& value) {
    detail::Cursor cursor{0};
    if constexpr (noexcept(Codec::write(bytes, offset, value, cursor))) {
        Codec::write(bytes, offset, value, cursor);
    } else {
        // Bound stack use independently of schema size. Fixed encodings have no
        // out-of-line references, so their writer can use a temporary base of zero.
        constexpr auto size = Codec::fixed_write_size();
        auto scratch = [] {
            if constexpr (size <= 1024) return std::array<std::byte, size>{};
            else return std::make_unique_for_overwrite<std::byte[]>(size);
        }();
        const auto data = [&] {
            if constexpr (size <= 1024) return scratch.data();
            else return scratch.get();
        }();
        // Keep padding and inactive payload bytes as they were before replacement.
        if constexpr (size != 0) std::memcpy(data, bytes.data() + offset, size);
        Codec::write(std::span<std::byte>(data, size), 0, value, cursor);
        if constexpr (size != 0) std::memcpy(bytes.data() + offset, data, size);
    }
}

/// Construction is shared; the generated facade supplies names and capabilities.
/// External storage must not overlap the input or any of its nested data.
template<class View, class Codec, std::size_t HeaderSize, std::size_t Alignment, bool FullyWritten = false>
struct ViewFactory {
    template<class V>
        requires EncodeSourceFor<V, Codec>
    static std::size_t compute_byte_length(const V& value) {
        detail::Cursor cursor{HeaderSize};
        Codec::measure(value, cursor);
        return cursor.end;
    }
    template<class V>
        requires EncodeSourceFor<V, Codec>
    static OwnedView<View> create(const V& value) {
        OwnedView<View> result(compute_byte_length(value), Alignment, false);
        if constexpr (!FullyWritten && !detail::fully_writes_header<Codec>(HeaderSize))
            std::fill_n(result.bytes().data(), HeaderSize, std::byte{});
        detail::Cursor cursor{HeaderSize};
        Codec::write(result.bytes(), 0, value, cursor);
        return result;
    }
    template<class V>
        requires EncodeSourceFor<V, Codec>
    static View create_in_place(std::span<std::byte> bytes, const V& value) {
        const auto size = compute_byte_length(value);
        if (bytes.size() < size || reinterpret_cast<std::uintptr_t>(bytes.data()) % Alignment != 0)
            throw std::invalid_argument("buffer capacity or alignment");
        bytes = bytes.first(size);
        if constexpr (!FullyWritten && !detail::fully_writes_header<Codec>(HeaderSize))
            std::fill_n(bytes.data(), HeaderSize, std::byte{});
        detail::Cursor cursor{HeaderSize};
        Codec::write(bytes, 0, value, cursor);
        return View(bytes, 0, detail::Unchecked{});
    }
};
} // namespace intrlay
