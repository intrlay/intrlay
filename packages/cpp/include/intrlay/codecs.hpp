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

// A repeated codec/source pair closes a recursive tree's type requirements.
// This inspects types only; it never visits runtime elements or allocates storage.
template<class Codec, class Source, class... Seen>
consteval bool source_compatible() {
    // Encoders read const sources, but volatile still changes which operations
    // are available (for example, std::string has no volatile data() overload).
    using Value = std::remove_const_t<std::remove_reference_t<Source>>;
    using Pair = SourcePair<Codec, Value>;
    if constexpr ((std::same_as<Pair, Seen> || ...)) return true;
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

/// Whether Source can update an existing descriptor value without relocating it.
/// This also covers a box/optional forwarding mutation to its existing payload.
template<class Source, class Descriptor>
concept SetSourceFor = requires(std::span<std::byte> bytes, std::size_t offset, const Source& value) {
    Descriptor::set(bytes, offset, value);
};

/// Scalar codecs operate on bytes, without requiring aligned or live T objects.
/// T is the logical value type; Storage is its encoded primitive representation.
template<class T, class Storage = T>
struct ScalarCodec {
    static constexpr bool zeroable = std::is_arithmetic_v<T>;
    template<class V, class...>
    static consteval bool accepts_source() {
        return detail::SameScalarType<std::remove_cv_t<V>, T>;
    }
    static constexpr bool fixed_layout() { return true; }
    template<EncodeSourceFor<ScalarCodec> V>
    static void measure(const V&, detail::Cursor&) {}
    template<EncodeSourceFor<ScalarCodec> V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor&) {
        detail::store<Storage>(bytes, offset, static_cast<Storage>(value));
    }
};

struct UnitCodec {
    static constexpr bool zeroable = true;
    template<class V, class...>
    static consteval bool accepts_source() { return std::same_as<V, std::monostate>; }
    static constexpr bool fixed_layout() { return true; }
    template<EncodeSourceFor<UnitCodec> V>
    static void measure(const V&, detail::Cursor&) {}
    template<EncodeSourceFor<UnitCodec> V>
    static void write(std::span<std::byte>, std::size_t, const V&, detail::Cursor&) {}
};

struct StringCodec {
    template<class V, class...>
    static consteval bool accepts_source() {
        return std::convertible_to<const V&, std::string_view>
            || requires(const V& value) { std::string_view(value.data(), value.size()); };
    }
    static constexpr bool fixed_layout() { return false; }
    static void measure(std::string_view value, detail::Cursor& cursor) {
        cursor.reserve(value.size(), 1);
    }
    static void write(std::span<std::byte> bytes, std::size_t offset, std::string_view value, detail::Cursor& cursor) {
        const auto payload = cursor.reserve(value.size(), 1);
        detail::reference(bytes, offset, payload);
        detail::store<std::uint32_t>(bytes, offset + 4, static_cast<std::uint32_t>(value.size()));
        if (!value.empty()) std::memcpy(bytes.data() + payload, value.data(), value.size());
    }

    // Character traits and allocators do not change the encoded character bytes.
    template<class V>
    requires requires(const V& value) { std::string_view(value.data(), value.size()); }
    static void measure(const V& value, detail::Cursor& cursor) {
        measure(std::string_view(value.data(), value.size()), cursor);
    }
    template<class V>
    requires requires(const V& value) { std::string_view(value.data(), value.size()); }
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        write(bytes, offset, std::string_view(value.data(), value.size()), cursor);
    }
};

/// Get is a stateless compile-time accessor; Codec describes its wire value.
template<std::size_t Offset, class Codec, auto Get>
struct FieldCodec {
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        if constexpr (requires(const V& value) { Get(value); })
            return detail::source_compatible<Codec, decltype(Get(std::declval<const V&>())), Seen...>();
        else return false;
    }
    static constexpr bool fixed_layout() { return Codec::fixed_layout(); }
    template<EncodeSourceFor<FieldCodec> V>
    static void measure(const V& value, detail::Cursor& cursor) {
        Codec::measure(Get(value), cursor);
    }
    template<EncodeSourceFor<FieldCodec> V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        Codec::write(bytes, offset + Offset, Get(value), cursor);
    }
};

template<class... Fields>
struct RecordCodec {
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        return (detail::source_compatible<Fields, V, Seen...>() && ...);
    }
    static constexpr bool fixed_layout() { return (Fields::fixed_layout() && ...); }
    template<EncodeSourceFor<RecordCodec> V>
    static void measure(const V& value, detail::Cursor& cursor) {
        (Fields::measure(value, cursor), ...);
    }
    // Empty records have no field writes and do not consume these coordinates.
    template<EncodeSourceFor<RecordCodec> V>
    static void write([[maybe_unused]] std::span<std::byte> bytes,
                      [[maybe_unused]] std::size_t offset,
                      const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        (Fields::write(bytes, offset, value, cursor), ...);
    }
};

/// Stride and alignment are supplied by the common layout implementation.
template<class Element, std::size_t Stride, std::size_t Alignment>
struct SequenceCodec {
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        if constexpr (std::same_as<V, ZeroedSequence>) return detail::zeroable<Element>;
        else return detail::sequence_source<Element, V, Seen...>();
    }
    static constexpr bool fixed_layout() { return false; }
    template<EncodeSourceFor<SequenceCodec> V>
    static void measure(const V& value, detail::Cursor& cursor) {
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
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        const std::size_t count = [&] {
            if constexpr (std::same_as<V, ZeroedSequence>) return value.length;
            else return static_cast<std::size_t>(std::size(value));
        }();
        const auto payload = cursor.reserve(detail::array_bytes(count, Stride), Alignment);
        detail::reference(bytes, offset, payload);
        detail::store<std::uint32_t>(bytes, offset + 4, static_cast<std::uint32_t>(count));
        if constexpr (std::same_as<V, ZeroedSequence>) {
            std::fill_n(bytes.data() + payload, count * Stride, std::byte{});
        } else if constexpr (std::ranges::contiguous_range<const V>
                      && detail::native_layout<Element, std::ranges::range_reference_t<const V>>) {
            if (count != 0) std::memcpy(bytes.data() + payload, std::ranges::data(value), count * Stride);
        } else {
            for (std::size_t i = 0; i < count; ++i) Element::write(bytes, payload + i * Stride, value[i], cursor);
        }
    }
};

template<class Element, std::size_t Stride, std::size_t Length>
struct ArrayCodec {
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
    static void measure(const V& value, detail::Cursor& cursor) {
        if constexpr (!std::same_as<V, Zeroed>) {
            const auto end = std::ranges::end(value);
            for (auto item = std::ranges::begin(value); item != end; ++item) {
                Element::measure(*item, cursor);
            }
        }
    }
    template<EncodeSourceFor<ArrayCodec> V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        if constexpr (std::same_as<V, Zeroed>) {
            std::fill_n(bytes.data() + offset, Length * Stride, std::byte{});
        } else {
            if (static_cast<std::size_t>(std::size(value)) != Length) throw std::length_error("fixed array length");
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
struct OptionalCodec {
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        if constexpr (std::same_as<std::remove_cv_t<V>, std::nullopt_t>) return true;
        else if constexpr (requires(const V& value) { { value.has_value() } -> std::convertible_to<bool>; *value; })
            return detail::source_compatible<Value, decltype(*std::declval<const V&>()), Seen...>();
        else return false;
    }
    static constexpr bool fixed_layout() { return Value::fixed_layout(); }
    template<EncodeSourceFor<OptionalCodec> V>
    static void measure(const V& value, detail::Cursor& cursor) {
        if constexpr (!std::same_as<std::remove_cv_t<V>, std::nullopt_t>) {
            if (value.has_value()) Value::measure(*value, cursor);
        }
    }
    template<EncodeSourceFor<OptionalCodec> V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        if constexpr (std::same_as<std::remove_cv_t<V>, std::nullopt_t>) {
            detail::store<std::uint8_t>(bytes, offset, 0);
        } else {
            detail::store<std::uint8_t>(bytes, offset, value.has_value() ? 1 : 0);
            if (value.has_value()) Value::write(bytes, offset + PayloadOffset, *value, cursor);
        }
    }
};

template<class Value, std::size_t Size, std::size_t Alignment>
struct BoxCodec {
    template<class V, class... Seen>
    static consteval bool accepts_source() {
        if constexpr (requires(const V& value) { *value; })
            return detail::source_compatible<Value, decltype(*std::declval<const V&>()), Seen...>();
        else return false;
    }
    static constexpr bool fixed_layout() { return false; }
    template<EncodeSourceFor<BoxCodec> V>
    static void measure(const V& value, detail::Cursor& cursor) {
        cursor.reserve(Size, Alignment);
        Value::measure(*value, cursor);
    }
    template<EncodeSourceFor<BoxCodec> V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        const auto payload = cursor.reserve(Size, Alignment);
        detail::reference(bytes, offset, payload);
        Value::write(bytes, payload, *value, cursor);
    }
};

template<std::uint32_t Tag, class Payload>
struct VariantCodec : Payload {
    static constexpr std::uint32_t tag = Tag;
    template<EncodeSourceFor<Payload> V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        detail::store<std::uint32_t>(bytes, offset, Tag);
        Payload::write(bytes, offset, value, cursor);
    }
};

template<class... Cases>
struct EnumCodec {
    static constexpr bool fixed_layout() { return (Cases::fixed_layout() && ...); }
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
    static void measure(const V& value, [[maybe_unused]] detail::Cursor& cursor) {
        if constexpr (std::is_enum_v<V>) {
            static_cast<void>(native_tag(value));
        } else {
            visit(detail::variant(value), [&]<class Case>(const auto& payload) {
                Case::measure(payload, cursor);
            });
        }
    }
    template<EncodeSourceFor<EnumCodec> V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, [[maybe_unused]] detail::Cursor& cursor) {
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

/// Fixed encodings need neither a measurement pass nor a payload allocation.
/// The input and its nested data must not overlap any destination bytes written.
template<class Codec, FixedEncodeSourceFor<Codec> T>
void replace(std::span<std::byte> bytes, std::size_t offset, const T& value) {
    detail::Cursor cursor{0};
    Codec::write(bytes, offset, value, cursor);
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
        OwnedView<View> result(compute_byte_length(value), Alignment, !FullyWritten);
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
        if constexpr (!FullyWritten) std::fill(bytes.begin(), bytes.end(), std::byte{});
        detail::Cursor cursor{HeaderSize};
        Codec::write(bytes, 0, value, cursor);
        return View(bytes);
    }
};
} // namespace intrlay
