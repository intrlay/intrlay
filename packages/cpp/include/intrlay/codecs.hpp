#pragma once
#include "runtime.hpp"
#include <iterator>
#include <tuple>

namespace intrlay {

/// Scalar codecs operate on bytes, without requiring aligned or live T objects.
template<class T>
struct ScalarCodec {
    static constexpr bool fixed_layout() { return true; }
    template<class V>
    static void measure(const V&, detail::Cursor&) {}
    template<class V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor&) {
        detail::store<T>(bytes, offset, static_cast<T>(value));
    }
};

struct UnitCodec {
    static constexpr bool fixed_layout() { return true; }
    template<class V>
    static void measure(const V&, detail::Cursor&) {}
    template<class V>
    static void write(std::span<std::byte>, std::size_t, const V&, detail::Cursor&) {}
};

struct StringCodec {
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
    static constexpr bool fixed_layout() { return Codec::fixed_layout(); }
    template<class V>
    static void measure(const V& value, detail::Cursor& cursor) {
        Codec::measure(Get(value), cursor);
    }
    template<class V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        Codec::write(bytes, offset + Offset, Get(value), cursor);
    }
};

template<class... Fields>
struct RecordCodec {
    static constexpr bool fixed_layout() { return (Fields::fixed_layout() && ...); }
    template<class V>
    static void measure(const V& value, detail::Cursor& cursor) {
        (Fields::measure(value, cursor), ...);
    }
    template<class V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        (Fields::write(bytes, offset, value, cursor), ...);
    }
};

/// Stride and alignment are supplied by the common layout implementation.
template<class Element, std::size_t Stride, std::size_t Alignment>
struct SequenceCodec {
    static constexpr bool fixed_layout() { return false; }
    template<class V>
    static void measure(const V& value, detail::Cursor& cursor) {
        cursor.reserve(detail::array_bytes(std::size(value), Stride), Alignment);
        for (const auto& item : value) Element::measure(item, cursor);
    }
    template<class V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        const auto count = std::size(value);
        const auto payload = cursor.reserve(detail::array_bytes(count, Stride), Alignment);
        detail::reference(bytes, offset, payload);
        detail::store<std::uint32_t>(bytes, offset + 4, static_cast<std::uint32_t>(count));
        if constexpr (std::ranges::contiguous_range<const V>
                      && detail::native_layout<Element, std::ranges::range_reference_t<const V>>) {
            if (count != 0) std::memcpy(bytes.data() + payload, std::ranges::data(value), count * Stride);
        } else {
            for (std::size_t i = 0; i < count; ++i) Element::write(bytes, payload + i * Stride, value[i], cursor);
        }
    }
};

template<class Element, std::size_t Stride, std::size_t Length>
struct ArrayCodec {
    static constexpr bool fixed_layout() {
        if constexpr (Length == 0) return true;
        else return Element::fixed_layout();
    }
    template<class V>
    static void measure(const V& value, detail::Cursor& cursor) {
        for (const auto& item : value) Element::measure(item, cursor);
    }
    template<class V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        if (std::size(value) != Length) throw std::length_error("fixed array length");
        if constexpr (std::ranges::contiguous_range<const V>
                      && detail::native_layout<Element, std::ranges::range_reference_t<const V>>) {
            if constexpr (Length != 0) std::memcpy(bytes.data() + offset, std::ranges::data(value), Length * Stride);
        } else {
            for (std::size_t i = 0; i < Length; ++i) Element::write(bytes, offset + i * Stride, value[i], cursor);
        }
    }
};

template<class Value, std::size_t PayloadOffset>
struct OptionalCodec {
    static constexpr bool fixed_layout() { return Value::fixed_layout(); }
    template<class V>
    static void measure(const V& value, detail::Cursor& cursor) {
        if (value.has_value()) Value::measure(*value, cursor);
    }
    template<class V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        detail::store<std::uint8_t>(bytes, offset, value.has_value() ? 1 : 0);
        if (value.has_value()) Value::write(bytes, offset + PayloadOffset, *value, cursor);
    }
};

template<class Value, std::size_t Size, std::size_t Alignment>
struct BoxCodec {
    static constexpr bool fixed_layout() { return false; }
    template<class V>
    static void measure(const V& value, detail::Cursor& cursor) {
        cursor.reserve(Size, Alignment);
        Value::measure(*value, cursor);
    }
    template<class V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        const auto payload = cursor.reserve(Size, Alignment);
        detail::reference(bytes, offset, payload);
        Value::write(bytes, payload, *value, cursor);
    }
};

template<std::uint32_t Tag, class Payload>
struct VariantCodec : Payload {
    static constexpr std::uint32_t tag = Tag;
    template<class V>
    static void write(std::span<std::byte> bytes, std::size_t offset, const V& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        detail::store<std::uint32_t>(bytes, offset, Tag);
        Payload::write(bytes, offset, value, cursor);
    }
};

template<class... Cases>
struct EnumCodec {
    static constexpr bool fixed_layout() { return (Cases::fixed_layout() && ...); }
private:
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
    template<class V>
    static void measure(const V& value, [[maybe_unused]] detail::Cursor& cursor) {
        if constexpr (std::is_enum_v<V>) {
            static_cast<void>(native_tag(value));
        } else {
            visit(detail::variant(value), [&]<class Case>(const auto& payload) {
                Case::measure(payload, cursor);
            });
        }
    }
    template<class V>
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

/// Fixed encodings need neither a measurement pass nor a payload allocation.
/// The input and its nested data must not overlap any destination bytes written.
template<class Codec, class T>
requires detail::fixed_encoding<Codec, T>
void replace(std::span<std::byte> bytes, std::size_t offset, const T& value) {
    detail::Cursor cursor{0};
    Codec::write(bytes, offset, value, cursor);
}

/// Construction is shared; the generated facade supplies names and capabilities.
/// External storage must not overlap the input or any of its nested data.
template<class View, class Codec, std::size_t HeaderSize, std::size_t Alignment, bool FullyWritten = false>
struct ViewFactory {
    template<class V>
    static std::size_t compute_byte_length(const V& value) {
        detail::Cursor cursor{HeaderSize};
        Codec::measure(value, cursor);
        return cursor.end;
    }
    template<class V>
    static OwnedView<View> create(const V& value) {
        OwnedView<View> result(compute_byte_length(value), Alignment, !FullyWritten);
        detail::Cursor cursor{HeaderSize};
        Codec::write(result.bytes(), 0, value, cursor);
        return result;
    }
    template<class V>
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
