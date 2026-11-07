#pragma once
#include "collection_view.hpp"

// C++20 cannot form a member expression from a string. This small accessor is
// the only token-level operation; field names, layouts, and traversal are types.
#define INTRLAY_MEMBER(name) ::intrlay::MemberAccessor{ \
    [](const auto& object) -> decltype(auto) { return (object.name); }, \
    []<class T>() consteval { return offsetof(T, name); } }

namespace intrlay {

/// The offset callback is instantiated only for standard-layout native records.
template<class Get, class Offset>
struct MemberAccessor {
    Get get;
    Offset native_offset;
    template<class T>
    constexpr decltype(auto) operator()(const T& value) const { return get(value); }
    template<class T>
    consteval std::size_t offset() const { return native_offset.template operator()<T>(); }
};

/// A field name stored in compile-time metadata, without a runtime allocation.
template<std::size_t N>
struct FieldName {
    char text[N];
    consteval FieldName(const char (&value)[N]) {
        std::copy_n(value, N, text);
    }
    constexpr operator std::string_view() const { return {text, N - 1}; }
};

namespace detail {
template<class> struct MemberOwner;
template<class Value, class Owner>
struct MemberOwner<Value Owner::*> { using Type = Owner; };
} // namespace detail

/// Map a generated native field to its original C++ spelling when encoding an
/// existing source object. The source type need not have the same memory layout.
template<auto Member, auto Source>
struct MappedMember {
    template<class T>
    constexpr decltype(auto) operator()(const T& value) const {
        if constexpr (std::is_same_v<std::remove_cvref_t<T>,
                                    typename detail::MemberOwner<decltype(Member)>::Type>) {
            return (value.*Member);
        } else {
            return Source(value);
        }
    }
};

/// An alternative is the payload itself, including records with a `value` field.
struct AlternativeValue {
    template<class T>
    constexpr decltype(auto) operator()(const T& value) const {
        return (value);
    }
};

/// Descriptor primitives reuse construction codecs and add borrowed access.
/// All offsets and strides come from Intrlay's common layout implementation.
template<class T, class Storage = T>
struct Scalar : ScalarCodec<Storage> {
    template<class V>
    static constexpr bool native_layout = std::is_same_v<V, T>
        && std::is_same_v<T, Storage> && !std::is_same_v<T, bool>;
    template<class Byte>
    static T read(std::span<Byte> bytes, std::size_t offset) {
        return static_cast<T>(detail::load<Storage>(bytes, offset));
    }
    static void set(std::span<std::byte> bytes, std::size_t offset, T value) {
        detail::store<Storage>(bytes, offset, static_cast<Storage>(value));
    }
};

struct Unit : UnitCodec {
    static void set(std::span<std::byte>, std::size_t, std::monostate) {}
    template<class Byte>
    static std::monostate read(std::span<Byte>, std::size_t) { return {}; }
};

struct String : StringCodec {
    template<class Byte>
    static std::string_view read(std::span<Byte> bytes, std::size_t offset) {
        return detail::string_at(bytes, offset);
    }
};

template<class Value, std::size_t PayloadOffset>
struct Optional : OptionalCodec<Value, PayloadOffset> {
    template<class Byte>
    static auto read(std::span<Byte> bytes, std::size_t offset) {
        using Result = std::optional<decltype(Value::read(bytes, offset))>;
        if (detail::load<std::uint8_t>(bytes, offset) == 0) return Result{};
        return Result(std::in_place, Value::read(bytes, offset + PayloadOffset));
    }
    template<class T>
    static void set(std::span<std::byte> bytes, std::size_t offset, const T& value)
        requires (detail::fixed_encoding<OptionalCodec<Value, PayloadOffset>, T> || requires { Value::set(bytes, offset, value); }) {
        if constexpr (OptionalCodec<Value, PayloadOffset>::fixed_layout()) {
            intrlay::replace<OptionalCodec<Value, PayloadOffset>>(bytes, offset, value);
        } else if (detail::load<std::uint8_t>(bytes, offset) != 0) {
            Value::set(bytes, offset + PayloadOffset, value);
        }
    }
};

template<class Value, std::size_t Size, std::size_t Alignment>
struct Box : BoxCodec<Value, Size, Alignment> {
    template<class Byte>
    static auto read(std::span<Byte> bytes, std::size_t offset) {
        const auto payload = detail::dereference(bytes, offset);
        return Value::read(bytes, payload);
    }
    template<class T>
    static void set(std::span<std::byte> bytes, std::size_t offset, const T& value)
        requires requires { Value::set(bytes, offset, value); } {
        Value::set(bytes, detail::dereference(bytes, offset), value);
    }
};

/// One reflected field: its API name, resolved byte offset, value descriptor,
/// and native accessor. The same metadata drives encoding and borrowed access.
template<FieldName Name, std::size_t Offset, class Value, auto Access>
struct Field : FieldCodec<Offset, Value, Access> {
    static constexpr std::string_view name = Name;
    static constexpr std::size_t offset = Offset;
    using Type = Value;
    template<class T>
    static constexpr decltype(auto) get(const T& value) { return Access(value); }
    template<class T>
    static consteval bool matches_native_layout() {
        using Member = decltype(Access(std::declval<const T&>()));
        if constexpr (detail::native_layout<Value, Member>
                      && requires { Access.template offset<T>(); }) {
            return Access.template offset<T>() == Offset;
        } else return false;
    }
    template<class T>
    static constexpr std::size_t native_size = sizeof(std::remove_cvref_t<decltype(Access(std::declval<const T&>()))>);
    template<class Byte>
    static auto read(std::span<Byte> bytes, std::size_t base) {
        return Value::read(bytes, base + Offset);
    }
    template<class T>
    static void set(std::span<std::byte> bytes, std::size_t base, const T& value) {
        Value::set(bytes, base + Offset, value);
    }
};

template<class... Fields>
struct FieldList : RecordCodec<Fields...> {
    static constexpr std::size_t field_count = sizeof...(Fields);
    template<std::size_t Index>
    using Field = std::tuple_element_t<Index, std::tuple<Fields...>>;

    /// Visitor receives each field's type; no field values or views are copied.
    template<class Visitor>
    static constexpr void for_each_field(Visitor&& visitor) {
        (visitor.template operator()<Fields>(), ...);
    }
};

template<template<class> class View>
struct BorrowedView {
    template<class Byte>
    static View<Byte> read(std::span<Byte> bytes, std::size_t offset) {
        return View<Byte>(bytes, offset);
    }
};

template<template<class> class View, std::size_t Size, class... Fields>
struct Record : FieldList<Fields...>, BorrowedView<View> {
    template<class T>
    static void set(std::span<std::byte> bytes, std::size_t offset, const T& value)
        requires detail::fixed_encoding<FieldList<Fields...>, T> {
        intrlay::replace<Record>(bytes, offset, value);
    }

    template<class T>
    static constexpr bool native_layout = [] {
        if constexpr (!std::is_volatile_v<T> && std::is_standard_layout_v<T> && std::is_trivially_copyable_v<T>) {
            return sizeof(T) == Size && sizeof(T) == (Fields::template native_size<T> + ... + 0)
                && (Fields::template matches_native_layout<T>() && ...);
        } else return false;
    }();

    template<class T>
    static void write(std::span<std::byte> bytes, std::size_t offset, const T& INTRLAY_RESTRICT value, detail::Cursor& cursor) {
        if constexpr (native_layout<T>) {
            std::memcpy(bytes.data() + offset, std::addressof(value), Size);
        } else FieldList<Fields...>::write(bytes, offset, value, cursor);
    }
};

template<std::uint32_t Tag, class... Fields>
struct Variant : VariantCodec<Tag, FieldList<Fields...>> {};

template<template<class> class View, class... Cases>
struct Enum : EnumCodec<Cases...>, BorrowedView<View> {
    template<class T>
    static void set(std::span<std::byte> bytes, std::size_t offset, const T& value)
        requires detail::fixed_encoding<EnumCodec<Cases...>, T> {
        intrlay::replace<EnumCodec<Cases...>>(bytes, offset, value);
    }
    static constexpr std::size_t variant_count = sizeof...(Cases);
    template<std::size_t Index>
    using Case = std::tuple_element_t<Index, std::tuple<Cases...>>;

    template<std::size_t CaseIndex, std::size_t FieldIndex, class Byte>
    static auto read_field(std::span<Byte> bytes, std::size_t offset) {
        using Field = typename Case<CaseIndex>::template Field<FieldIndex>;
        using Result = std::optional<decltype(Field::read(bytes, offset))>;
        if (detail::load<std::uint32_t>(bytes, offset) != Case<CaseIndex>::tag) return Result{};
        return Result(std::in_place, Field::read(bytes, offset));
    }
    template<std::size_t CaseIndex, std::size_t FieldIndex, class T>
    static void set_field(std::span<std::byte> bytes, std::size_t offset, const T& value) {
        using Field = typename Case<CaseIndex>::template Field<FieldIndex>;
        if (detail::load<std::uint32_t>(bytes, offset) == Case<CaseIndex>::tag) {
            Field::set(bytes, offset, value);
        }
    }
};

template<template<class> class View, class Element, std::size_t Stride, std::size_t Alignment>
struct Sequence : SequenceCodec<Element, Stride, Alignment>, BorrowedView<View> {
    static constexpr std::size_t extent = std::dynamic_extent;

    template<class Byte>
    static std::size_t size(std::span<Byte> bytes, std::size_t offset) {
        return detail::load<std::uint32_t>(bytes, offset + 4);
    }
    template<class Byte>
    static auto get(std::span<Byte> bytes, std::size_t offset, std::size_t index) {
        return Element::read(bytes, detail::dereference(bytes, offset) + index * Stride);
    }
    template<class T>
    static void set(std::span<std::byte> bytes, std::size_t offset, std::size_t index, const T& value)
        requires requires { Element::set(bytes, offset, value); } {
        Element::set(bytes, detail::dereference(bytes, offset) + index * Stride, value);
    }
};

template<template<class> class View, class Element, std::size_t Stride, std::size_t Length>
struct Array : ArrayCodec<Element, Stride, Length>, BorrowedView<View> {
    template<class T>
    static void set(std::span<std::byte> bytes, std::size_t offset, const T& value)
        requires detail::fixed_encoding<ArrayCodec<Element, Stride, Length>, T> {
        intrlay::replace<ArrayCodec<Element, Stride, Length>>(bytes, offset, value);
    }
    static constexpr std::size_t extent = Length;
    template<class T>
    static constexpr bool native_layout = [] {
        if constexpr (detail::NativeArray<T>::value) {
            return detail::native_layout<Element, typename detail::NativeArray<T>::Element>
                && detail::NativeArray<T>::length == Length
                && sizeof(T) == Stride * Length;
        } else return false;
    }();

    template<class Byte>
    static std::size_t size(std::span<Byte>, std::size_t) { return Length; }
    template<class Byte>
    static auto get(std::span<Byte> bytes, std::size_t offset, std::size_t index) {
        return Element::read(bytes, offset + index * Stride);
    }
    template<class T>
    static void set(std::span<std::byte> bytes, std::size_t offset, std::size_t index, const T& value)
        requires requires { Element::set(bytes, offset, value); } {
        Element::set(bytes, offset + index * Stride, value);
    }
};
} // namespace intrlay
