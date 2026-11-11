#pragma once
#include "collection_view.hpp"
#include "validation.hpp"

// C++20 cannot form a member expression from a string. This small accessor is
// the only token-level operation; field names, layouts, and traversal are types.
// Initialize a named inline constexpr variable so the closure types have one
// identity when descriptors are included from multiple translation units.
// A const bitfield binds to a temporary, not its storage. Reject it with sizeof
// before returning a reference, without invoking an overloaded address operator.
// Native offsets need both a member pointer and an ordinary bound address:
// an overloaded operator& on a static object can otherwise mimic a member pointer.
#define INTRLAY_MEMBER(name) ::intrlay::MemberAccessor{ \
    [](const auto& object) noexcept -> decltype((object.name)) \
        requires requires { sizeof(object.name); } { return (object.name); }, \
    []<class T>() consteval \
        requires (std::is_standard_layout_v<T> \
            && std::is_member_object_pointer_v<decltype(&T::name)> \
            && std::is_pointer_v<decltype(&std::declval<T&>().name)>) \
        { return offsetof(T, name); } }

namespace intrlay {

namespace detail {
struct GeneratedInputTag {};

template<class T>
concept GeneratedInput = requires {
    typename T::intrlay_input;
    requires std::same_as<typename T::intrlay_input, GeneratedInputTag>;
};
} // namespace detail

/// Reading a member does not imply that it occupies storage in its source object.
/// Only a callable native-offset callback can participate in the layout proof.
template<class Get, class Offset>
struct MemberAccessor {
    Get get;
    Offset native_offset;
    template<class T>
        requires requires(const Get& accessor, const T& value) { accessor(value); }
    constexpr auto operator()(const T& value) const noexcept(noexcept(get(value))) -> decltype(get(value)) { return get(value); }
    template<class T>
        requires (std::is_standard_layout_v<T>
            && requires(const Get& accessor, const Offset& offset, const T& value) {
                accessor(value);
                offset.template operator()<T>();
            })
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

/// Map a generated native field to its original C++ spelling when encoding an
/// existing source object. The source type need not have the same memory layout.
template<auto Generated, auto Source>
struct MappedMember {
    template<class T>
        requires detail::GeneratedInput<T>
    constexpr auto operator()(const T& value) const noexcept(noexcept(Generated(value))) -> decltype(Generated(value)) {
        return Generated(value);
    }
    template<class T>
        requires (!detail::GeneratedInput<T>)
    constexpr auto operator()(const T& value) const noexcept(noexcept(Source(value))) -> decltype(Source(value)) {
        return Source(value);
    }
};

/// An alternative is the payload itself, including records with a `value` field.
struct AlternativeValue {
    template<class T>
    constexpr decltype(auto) operator()(const T& value) const noexcept {
        return (value);
    }
};

/// Descriptor primitives share construction, validation, and borrowed access.
/// Reading assumes validation at construction or the caller's unchecked contract.
template<class T, class Storage = T>
struct Scalar : ScalarCodec<T, Storage> {
    static constexpr std::size_t byte_size = sizeof(Storage);
    static consteval bool all_bits_valid() { return !std::same_as<T, bool>; }
    template<class V>
    static constexpr bool native_layout = std::same_as<V, T>
        && std::same_as<T, Storage> && !std::same_as<T, bool>;
    static Expected<void, ViewError> validate_fields(Validator& validator, std::size_t offset) noexcept {
        if constexpr (std::same_as<T, bool>) {
            auto value = validator.read<Storage>(offset);
            if (!value) return Unexpected{value.error()};
            if (*value > 1) return Unexpected{ViewError::InvalidTag};
        }
        return {};
    }
    template<BufferByte Byte>
    static T read(std::span<Byte> bytes, std::size_t offset) noexcept {
        return static_cast<T>(detail::load<Storage>(bytes, offset));
    }
    template<EncodeSourceFor<Scalar> V>
    static void set(std::span<std::byte> bytes, std::size_t offset, const V& value) {
        intrlay::replace<ScalarCodec<T, Storage>>(bytes, offset, value);
    }
};

struct Unit : UnitCodec {
    static constexpr std::size_t byte_size = 0;
    static consteval bool all_bits_valid() { return true; }
    static Expected<void, ViewError> validate_fields(Validator&, std::size_t) noexcept { return {}; }
    template<EncodeSourceFor<Unit> V>
    static void set(std::span<std::byte>, std::size_t, const V&) noexcept {}
    template<BufferByte Byte>
    static std::monostate read(std::span<Byte>, std::size_t) noexcept { return {}; }
};

struct String : StringCodec {
    static constexpr std::size_t byte_size = 8;
    static consteval bool all_bits_valid() { return false; }
    static Expected<void, ViewError> validate_fields(Validator& validator, std::size_t offset) noexcept {
        auto count = validator.read<std::uint32_t>(offset + 4);
        if (!count) return Unexpected{count.error()};
        auto target = validator.payload(offset, 8, *count, 1);
        if (!target) return Unexpected{target.error()};
        return validator.utf8(*target, *count);
    }
    template<BufferByte Byte>
    static std::string_view read(std::span<Byte> bytes, std::size_t offset) noexcept {
        return detail::string_at(bytes, offset);
    }
};

template<class Value, std::size_t PayloadOffset, std::size_t Size>
struct Optional : OptionalCodec<Value, PayloadOffset> {
    static constexpr std::size_t byte_size = Size;
    static consteval bool all_bits_valid() { return false; }
    static Expected<void, ViewError> validate_fields(Validator& validator, std::size_t offset) noexcept {
        auto tag = validator.read<std::uint8_t>(offset);
        if (!tag) return Unexpected{tag.error()};
        if (*tag > 1) return Unexpected{ViewError::InvalidTag};
        if (*tag == 0) return {};
        return validator.field<Value>(offset + PayloadOffset);
    }
    template<BufferByte Byte>
    static auto read(std::span<Byte> bytes, std::size_t offset) noexcept {
        using Result = std::optional<decltype(Value::read(bytes, offset))>;
        if (detail::load<std::uint8_t>(bytes, offset) == 0) return Result{};
        return Result(std::in_place, Value::read(bytes, offset + PayloadOffset));
    }
    template<class T>
    static void set(std::span<std::byte> bytes, std::size_t offset, const T& value)
        requires (FixedEncodeSourceFor<T, OptionalCodec<Value, PayloadOffset>> || SetSourceFor<T, Value>) {
        if constexpr (FixedEncodeSourceFor<T, OptionalCodec<Value, PayloadOffset>>) {
            intrlay::replace<OptionalCodec<Value, PayloadOffset>>(bytes, offset, value);
        } else if constexpr (detail::fixed_encoding<OptionalCodec<Value, PayloadOffset>, T>) {
            Value::set(bytes, offset + PayloadOffset, value);
            detail::store<std::uint8_t>(bytes, offset, 1);
        } else if (detail::load<std::uint8_t>(bytes, offset) != 0) {
            // An indirect payload can only be updated while it already exists.
            Value::set(bytes, offset + PayloadOffset, value);
        }
    }
};

template<class Value, std::size_t Size, std::size_t Alignment>
struct Box : BoxCodec<Value, Size, Alignment> {
    static constexpr std::size_t byte_size = 4;
    static consteval bool all_bits_valid() { return false; }
    static Expected<void, ViewError> validate_fields(Validator& validator, std::size_t offset) noexcept {
        auto target = validator.payload(offset, 4, 1, Size);
        if (!target) return Unexpected{target.error()};
        return validator.field<Value>(*target);
    }
    template<BufferByte Byte>
    static auto read(std::span<Byte> bytes, std::size_t offset) noexcept {
        return Value::read(bytes, detail::dereference(bytes, offset));
    }
    template<SetSourceFor<Value> T>
    static void set(std::span<std::byte> bytes, std::size_t offset, const T& value) {
        Value::set(bytes, detail::dereference(bytes, offset), value);
    }
};

/// A field's resolved offset, wire descriptor, API name, and source accessor.
template<FieldName Name, std::size_t Offset, class Value, auto Access>
struct Field : FieldCodec<Offset, Value, Access> {
    static constexpr bool is_alternative_value = std::same_as<std::remove_cvref_t<decltype(Access)>, AlternativeValue>;
    static constexpr std::string_view name = Name;
    static constexpr std::size_t offset = Offset;
    using Type = Value;
    template<class T>
        requires requires(const T& value) { Access(value); }
    static constexpr decltype(auto) get(const T& value) { return Access(value); }
    template<class T>
    static consteval bool matches_native_layout() {
        using Member = decltype(Access(std::declval<const T&>()));
        if constexpr (detail::native_layout<Value, Member> && requires { Access.template offset<T>(); })
            return Access.template offset<T>() == Offset;
        else return false;
    }
    template<class T>
    static constexpr std::size_t native_size = sizeof(std::remove_cvref_t<decltype(Access(std::declval<const T&>()))>);
    static Expected<void, ViewError> validate_fields(Validator& validator, std::size_t base) noexcept {
        return validator.field<Value>(base + Offset);
    }
    template<BufferByte Byte>
    static auto read(std::span<Byte> bytes, std::size_t base) noexcept { return Value::read(bytes, base + Offset); }
    template<SetSourceFor<Value> T>
    static void set(std::span<std::byte> bytes, std::size_t base, const T& value) { Value::set(bytes, base + Offset, value); }
};

/// Metadata visitors receive field types as explicit template arguments.
template<class Visitor, class... Fields>
concept FieldVisitorFor = (requires(Visitor& visitor) { visitor.template operator()<Fields>(); } && ...);

template<class... Fields>
struct FieldList : RecordCodec<Fields...> {
    static constexpr std::size_t field_count = sizeof...(Fields);
    // Defer child metadata until use: descriptors may still be forward-declared.
    static consteval bool all_bits_valid() { return (Fields::Type::all_bits_valid() && ...); }
    template<std::size_t Index>
    using Field = std::tuple_element_t<Index, std::tuple<Fields...>>;
    static Expected<void, ViewError> validate_fields([[maybe_unused]] Validator& validator, [[maybe_unused]] std::size_t base) noexcept {
        return validate_next<0>(validator, base);
    }
    template<FieldVisitorFor<Fields...> Visitor>
    static constexpr void for_each_field(Visitor&& visitor) { (visitor.template operator()<Fields>(), ...); }

private:
    // Forward the first failure or the final field's result directly. A shared
    // Expected updated by every field inhibits inlining of mixed enum records.
    template<std::size_t Index> requires (Index <= field_count)
    static Expected<void, ViewError> validate_next(Validator& validator, std::size_t base) noexcept {
        if constexpr (Index == field_count) return {};
        else if constexpr (Index + 1 == field_count) return Field<Index>::validate_fields(validator, base);
        else {
            auto result = Field<Index>::validate_fields(validator, base);
            if (!result) return result;
            return validate_next<Index + 1>(validator, base);
        }
    }
};

template<template<class> class View>
struct BorrowedView {
    template<BufferByte Byte>
    static View<Byte> read(std::span<Byte> bytes, std::size_t offset) noexcept {
        return View<Byte>(bytes, offset, detail::Unchecked{});
    }
};

template<template<class> class View, std::size_t Size, class... Fields>
struct Record : FieldList<Fields...>, BorrowedView<View>, EncodingCodec<Record<View, Size, Fields...>> {
    static constexpr std::size_t byte_size = Size;
    static consteval bool fully_writes_header(std::size_t size) {
        std::size_t end = 0;
        bool complete = true;
        auto visit = [&]<class F>() {
            complete = complete && F::offset == end
                && detail::fully_writes_header<typename F::Type>(F::Type::byte_size);
            end = F::offset + F::Type::byte_size;
        };
        (visit.template operator()<Fields>(), ...);
        return complete && end == Size && size == Size;
    }
    using EncodingCodec<Record>::measure;
    using EncodingCodec<Record>::write;
    template<FixedEncodeSourceFor<FieldList<Fields...>> T>
    static void set(std::span<std::byte> bytes, std::size_t offset, const T& value) { intrlay::replace<Record>(bytes, offset, value); }
    template<class T>
    static constexpr bool native_layout = [] {
        if constexpr (FieldList<Fields...>::template accepts_source<T>() && !std::is_volatile_v<T>
                      && std::is_standard_layout_v<T> && std::is_trivially_copyable_v<T>) {
            return sizeof(T) == Size && sizeof(T) == (Fields::template native_size<T> + ... + 0)
                && (Fields::template matches_native_layout<T>() && ...);
        } else return false;
    }();
    template<EncodeSourceFor<FieldList<Fields...>> T>
    static void write_source(std::span<std::byte> bytes, std::size_t offset, const T& INTRLAY_RESTRICT value, detail::Cursor& cursor)
        noexcept(native_layout<T> || noexcept(FieldList<Fields...>::write_source(bytes, offset, value, cursor))) {
        if constexpr (native_layout<T>) std::memcpy(bytes.data() + offset, std::addressof(value), Size);
        else FieldList<Fields...>::write_source(bytes, offset, value, cursor);
    }
};

template<std::uint32_t Tag, class... Fields>
struct Variant : VariantCodec<Tag, FieldList<Fields...>,
    !(sizeof...(Fields) == 1 && (Fields::is_alternative_value && ...))> {};

/// Plain enums retain the schema's header size, which can exceed the u32 tag.
template<class T, std::size_t Size, std::uint32_t... Tags>
struct ValueEnum : EnumCodec<Variant<Tags>...> {
    static constexpr std::size_t byte_size = Size;
    static consteval bool fully_writes_header(std::size_t size) { return size == sizeof(std::uint32_t); }
    static consteval bool all_bits_valid() { return false; }
    static Expected<void, ViewError> validate_fields(Validator& validator, std::size_t offset) noexcept {
        auto tag = validator.read<std::uint32_t>(offset);
        if (!tag) return Unexpected{tag.error()};
        if (!((*tag == Tags) || ...)) return Unexpected{ViewError::InvalidTag};
        return {};
    }
    template<BufferByte Byte>
    static T read(std::span<Byte> bytes, std::size_t offset) noexcept { return static_cast<T>(detail::load<std::uint32_t>(bytes, offset)); }
    template<EncodeSourceFor<ValueEnum> V>
    static void set(std::span<std::byte> bytes, std::size_t offset, const V& value) { intrlay::replace<ValueEnum>(bytes, offset, value); }
    static constexpr std::size_t variant_count = sizeof...(Tags);
    template<std::size_t Index>
    using Case = std::tuple_element_t<Index, std::tuple<Variant<Tags>...>>;
};

template<template<class> class View, std::size_t Size, class... Cases>
struct Enum : EnumCodec<Cases...>, BorrowedView<View> {
    static constexpr std::size_t byte_size = Size;
    static consteval bool all_bits_valid() { return false; }
    static Expected<void, ViewError> validate_fields(Validator& validator, std::size_t offset) noexcept {
        auto tag = validator.read<std::uint32_t>(offset);
        if (!tag) return Unexpected{tag.error()};
        return validate_case<0>(*tag, validator, offset);
    }
    template<FixedEncodeSourceFor<EnumCodec<Cases...>> T>
    static void set(std::span<std::byte> bytes, std::size_t offset, const T& value) { intrlay::replace<EnumCodec<Cases...>>(bytes, offset, value); }
    static constexpr std::size_t variant_count = sizeof...(Cases);
    template<std::size_t Index>
    using Case = std::tuple_element_t<Index, std::tuple<Cases...>>;
    template<std::size_t CaseIndex, std::size_t FieldIndex, BufferByte Byte>
    static auto read_field(std::span<Byte> bytes, std::size_t offset) noexcept {
        using Field = typename Case<CaseIndex>::template Field<FieldIndex>;
        using Result = std::optional<decltype(Field::read(bytes, offset))>;
        if (detail::load<std::uint32_t>(bytes, offset) != Case<CaseIndex>::tag) return Result{};
        return Result(std::in_place, Field::read(bytes, offset));
    }
    template<std::size_t CaseIndex, std::size_t FieldIndex, class T>
        requires SetSourceFor<T, typename Case<CaseIndex>::template Field<FieldIndex>>
    static void set_field(std::span<std::byte> bytes, std::size_t offset, const T& value) {
        using Field = typename Case<CaseIndex>::template Field<FieldIndex>;
        if (detail::load<std::uint32_t>(bytes, offset) == Case<CaseIndex>::tag) Field::set(bytes, offset, value);
    }

private:
    // Only the active variant participates in validation. Return its result
    // directly so nested records do not carry a mutable Expected through dispatch.
    template<std::size_t Index> requires (Index <= variant_count)
    static Expected<void, ViewError> validate_case(std::uint32_t tag, Validator& validator, std::size_t offset) noexcept {
        if constexpr (Index == variant_count) return Unexpected{ViewError::InvalidTag};
        else {
            if (tag == Case<Index>::tag) return Case<Index>::validate_fields(validator, offset);
            return validate_case<Index + 1>(tag, validator, offset);
        }
    }
};

template<template<class> class View, class Element, std::size_t Stride, std::size_t Alignment>
struct Sequence : SequenceCodec<Element, Stride, Alignment>, BorrowedView<View> {
    static constexpr std::size_t extent = std::dynamic_extent;
    static consteval bool all_bits_valid() { return false; }
    static Expected<void, ViewError> validate_fields(Validator& validator, std::size_t offset) noexcept {
        auto count = validator.read<std::uint32_t>(offset + 4);
        if (!count) return Unexpected{count.error()};
        auto target = validator.payload(offset, 8, *count, Stride);
        if (!target) return Unexpected{target.error()};
        return validator.elements<Element>(*target, *count, Stride);
    }
    template<BufferByte Byte>
    static std::size_t size(std::span<Byte> bytes, std::size_t offset) noexcept { return detail::load<std::uint32_t>(bytes, offset + 4); }
    template<BufferByte Byte>
    static auto get(std::span<Byte> bytes, std::size_t offset, std::size_t index) noexcept {
        return Element::read(bytes, detail::dereference(bytes, offset) + index * Stride);
    }
    template<BufferByte Byte>
    static auto begin(std::span<Byte> bytes, std::size_t offset) noexcept {
        const auto count = size(bytes, offset);
        const auto first = count == 0 ? 0 : detail::dereference(bytes, offset);
        return ArrayIterator<Element, Byte, Stride>(bytes, first, count, detail::Unchecked{});
    }
    template<BufferByte Byte, class T, std::size_t Extent>
    static void copy_to(std::span<Byte> bytes, std::size_t offset, std::span<T, Extent> destination)
        requires requires { detail::copy_native_elements<Element, Stride>(bytes, offset, std::size_t{}, destination); } {
        detail::copy_native_elements<Element, Stride>(bytes, detail::dereference(bytes, offset), size(bytes, offset), destination);
    }
    template<SetSourceFor<Element> T>
    static void set(std::span<std::byte> bytes, std::size_t offset, std::size_t index, const T& value) {
        Element::set(bytes, detail::dereference(bytes, offset) + index * Stride, value);
    }
};

template<template<class> class View, class Element, std::size_t Stride, std::size_t Length>
struct Array : ArrayCodec<Element, Stride, Length>, BorrowedView<View> {
    static consteval bool all_bits_valid() { return Length == 0 || Element::all_bits_valid(); }
    static Expected<void, ViewError> validate_fields(Validator& validator, std::size_t offset) noexcept {
        return validator.elements<Element>(offset, Length, Stride);
    }
    template<FixedEncodeSourceFor<ArrayCodec<Element, Stride, Length>> T>
    static void set(std::span<std::byte> bytes, std::size_t offset, const T& value) { intrlay::replace<ArrayCodec<Element, Stride, Length>>(bytes, offset, value); }
    static constexpr std::size_t extent = Length;
    template<class T>
    static constexpr bool native_layout = [] {
        if constexpr (detail::NativeArray<T>::value) {
            return detail::native_layout<Element, typename detail::NativeArray<T>::Element>
                && detail::NativeArray<T>::length == Length && sizeof(T) == Stride * Length;
        } else return false;
    }();
    template<BufferByte Byte>
    static std::size_t size(std::span<Byte>, std::size_t) noexcept { return Length; }
    template<BufferByte Byte>
    static auto get(std::span<Byte> bytes, std::size_t offset, std::size_t index) noexcept { return Element::read(bytes, offset + index * Stride); }
    template<BufferByte Byte>
    static auto begin(std::span<Byte> bytes, std::size_t offset) noexcept {
        return ArrayIterator<Element, Byte, Stride>(bytes, offset, Length, detail::Unchecked{});
    }
    template<BufferByte Byte, class T, std::size_t Extent>
    static void copy_to(std::span<Byte> bytes, std::size_t offset, std::span<T, Extent> destination)
        requires requires { detail::copy_native_elements<Element, Stride>(bytes, offset, Length, destination); } {
        detail::copy_native_elements<Element, Stride>(bytes, offset, Length, destination);
    }
    template<SetSourceFor<Element> T>
    static void set(std::span<std::byte> bytes, std::size_t offset, std::size_t index, const T& value) { Element::set(bytes, offset + index * Stride, value); }
};
/// Projects a complete trusted encoding using the descriptor's borrowed type.
/// The caller guarantees valid bytes and retains their lifetime and invariants.
template<detail::ValidatableDescriptor Descriptor, BufferByte Byte>
    requires requires(std::span<Byte> bytes, std::size_t offset) { Descriptor::read(bytes, offset); }
auto view_unchecked(std::span<Byte> bytes, std::size_t offset = 0) noexcept {
    return Descriptor::read(bytes, offset);
}

/// Validates once and projects a value without reconstructing its owned input.
/// Mutable byte spans expose mutable projections; const spans expose read-only ones.
template<detail::ValidatableDescriptor Descriptor, BufferByte Byte>
    requires requires(std::span<Byte> bytes, std::size_t offset) { Descriptor::read(bytes, offset); }
auto view(std::span<Byte> bytes, std::size_t offset = 0) noexcept
    -> Expected<decltype(Descriptor::read(bytes, offset)), ViewError> {
    auto checked = Validator(bytes).template validate<Descriptor>(offset);
    if (!checked) return Unexpected{checked.error()};
    return view_unchecked<Descriptor>(bytes, offset);
}

} // namespace intrlay
