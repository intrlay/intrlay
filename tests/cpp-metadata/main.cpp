#ifdef NDEBUG
#undef NDEBUG
#endif
#include "bindings.hpp"
#include "custom.hpp"
#include <cassert>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <memory_resource>

namespace native {
struct CharacterTraits : std::char_traits<char> {};
template<class T> struct Text { T value; };
enum class Mode : std::uint64_t { On = 7, Off = 9 };
enum class SignedMode : std::int64_t { On = 7, Off = 9 };
struct Child {
    std::uint32_t value;
    std::uint32_t marker;
};
struct V { std::uint32_t native_value; };
struct Names {
    Child first;
    Child second;
    std::uint32_t factory;
    std::uint32_t element_access;
    V source;
};
struct Value {
    std::optional<std::uint32_t> value;
    std::uint32_t native_extra;
};
using OrdinaryChoice = std::variant<Value, std::monostate>;
using WholeChoice = std::variant<std::monostate, Child, std::array<std::optional<std::uint32_t>, 2>>;
struct ThrowingValue {
    std::optional<std::uint32_t> value;
    std::uint32_t native_extra = 0;
    explicit ThrowingValue(int) { throw std::runtime_error("construction failed"); }
    ThrowingValue(ThrowingValue&&) { throw std::runtime_error("move failed"); }
};
}

using NamesMetadata = cpp_metadata::NamesView::Metadata;
static_assert(NamesMetadata::field_count == 5);
static_assert(std::is_same_v<NamesMetadata, cpp_metadata::NamesViewReadOnly::Metadata>);
static_assert(NamesMetadata::Field<0>::name == "Child");
static_assert(NamesMetadata::Field<0>::offset == 0);
static_assert(NamesMetadata::Field<1>::offset == 8);
static_assert(NamesMetadata::Field<4>::offset == 24);
static_assert(std::is_same_v<
    decltype(NamesMetadata::Field<0>::read(std::span<const std::byte>{}, 0)),
    cpp_metadata::ChildViewReadOnly>);

constexpr native::Names reflected_input{{3, 5}, {7, 11}, 13, 17, {19}};
static_assert(NamesMetadata::Field<2>::get(reflected_input) == 13);
static_assert(std::is_same_v<decltype(NamesMetadata::Field<0>::get(reflected_input)), const native::Child&>);
constexpr auto reflected_names = [] {
    std::array<std::string_view, NamesMetadata::field_count> names{};
    std::size_t index = 0;
    NamesMetadata::for_each_field([&]<class Field>() { names[index++] = Field::name; });
    return names;
}();
static_assert(reflected_names == std::array<std::string_view, 5>{"Child", "second", "Factory", "ElementAccess", "source"});

using OrdinaryMetadata = cpp_metadata::OrdinaryChoiceView::Metadata;
static_assert(OrdinaryMetadata::variant_count == 2);
static_assert(OrdinaryMetadata::Case<0>::tag == 0);
static_assert(OrdinaryMetadata::Case<0>::field_count == 2);
static_assert(OrdinaryMetadata::Case<0>::Field<0>::name == "value");
static_assert(OrdinaryMetadata::Case<0>::Field<1>::name == "renamed_extra");
static_assert(OrdinaryMetadata::Case<1>::tag == 1);
static_assert(OrdinaryMetadata::Case<1>::field_count == 0);
using WholeMetadata = cpp_metadata::WholeChoiceView::Metadata;
static_assert(WholeMetadata::variant_count == 3);
static_assert(WholeMetadata::Case<1>::Field<0>::name == "payload");
static_assert(std::is_same_v<
    decltype(WholeMetadata::Case<1>::Field<0>::get(reflected_input.first)),
    const native::Child&>);
using CustomMetadata = cpp_metadata::custom::ApiChildView::ApiMetadata;
static_assert(intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, native::Child>);
static_assert(intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, cpp_metadata::Child>);
struct PaddedChild { std::uint32_t value; std::uint64_t marker; };
struct ReorderedChild { std::uint32_t marker; std::uint32_t value; };
struct VolatileChild { volatile std::uint32_t value; std::uint32_t marker; };
struct VolatileNames {
    volatile native::Child first;
    native::Child second;
    std::uint32_t factory;
    std::uint32_t element_access;
    native::V source;
};
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, PaddedChild>);
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, ReorderedChild>);
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, volatile native::Child>);
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, VolatileChild>);
static_assert(!intrlay::detail::native_layout<NamesMetadata, VolatileNames>);
static_assert(intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, const native::Child&>);
template<class Byte> struct DummyArrayView {};
using FixedFour = intrlay::Array<DummyArrayView, intrlay::Scalar<float>, 4, 4>;
static_assert(intrlay::detail::native_layout<FixedFour, std::array<float, 4>>);
static_assert(!intrlay::detail::native_layout<FixedFour, std::span<float>>);
static_assert(!intrlay::detail::native_layout<FixedFour, std::array<volatile float, 4>>);
static_assert(!intrlay::detail::native_layout<FixedFour, volatile float[4]>);
static_assert(CustomMetadata::field_count == 2);
static_assert(CustomMetadata::Field<0>::name == "native_value");
static_assert(CustomMetadata::Field<1>::name == "native_marker");
static_assert(CustomMetadata::Field<1>::offset == 4);

std::vector<std::byte> read_bytes(const std::filesystem::path& path) {
    std::ifstream input(path, std::ios::binary | std::ios::ate);
    assert(input);
    const auto length = input.tellg();
    assert(length >= 0);
    std::vector<std::byte> bytes(static_cast<std::size_t>(length));
    input.seekg(0);
    input.read(reinterpret_cast<char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
    assert(input);
    return bytes;
}

template<class T>
concept CanSetFactory = requires(T view) { view.set_Factory(1); };

int main(int argc, char** argv) {
    namespace generated = cpp_metadata;
    const std::filesystem::path directory = argc > 1 ? argv[1] : "target/cpp-metadata";

    volatile native::Child volatile_root{41, 43};
    auto root = generated::ChildView::create(volatile_root);
    assert(root.view().value() == 41 && root.view().marker() == 43);
    auto member = generated::ChildView::create(VolatileChild{47, 53});
    assert(member.view().value() == 47);
    auto volatile_nested = generated::NamesView::create(VolatileNames{{3, 5}, {7, 11}, 13, 17, {19}});
    assert(volatile_nested.view().Child().value() == 3);
    std::array<volatile float, 4> volatile_array{1, 2, 3, 4};
    std::array<std::byte, 24> encoded{};
    intrlay::detail::Cursor fixed_cursor{16};
    FixedFour::write(encoded, 0, volatile_array, fixed_cursor);
    assert(intrlay::detail::load<float>(encoded, 12) == 4);
    using Floats = intrlay::SequenceCodec<intrlay::Scalar<float>, 4, 4>;
    intrlay::detail::Cursor sequence_cursor{8};
    Floats::write(encoded, 0, std::span<volatile float>(volatile_array), sequence_cursor);
    assert(intrlay::detail::load<float>(encoded, 20) == 4);

    const native::Names native_names{{3, 5}, {7, 11}, 13, 17, {19}};
    const generated::Names generated_names{{3, 5}, {7, 11}, 13, 17, {19}};
    auto names = generated::NamesView::create(native_names);
    auto reordered = generated::ChildView::create(ReorderedChild{17, 13});
    assert(reordered.view().value() == 13 && reordered.view().marker() == 17);
    auto padded_child = generated::ChildView::create(PaddedChild{19, 23});
    assert(padded_child.view().value() == 19 && padded_child.view().marker() == 23);
    auto names_copy = generated::NamesView::create(generated_names);
    assert(std::ranges::equal(names.bytes(), names_copy.bytes()));
    assert(std::ranges::equal(names.bytes(), read_bytes(directory / "names.bin")));
    auto view = names.view();
    assert(view.Child().value() == 3 && view.second().value() == 7);
    assert(view.Factory() == 13 && view.ElementAccess() == 17);
    assert(view.source().renamed() == 19);
    assert(NamesMetadata::Field<2>::read(names.bytes(), 0) == 13);
    NamesMetadata::Field<2>::set(names.bytes(), 0, 14);
    assert(view.Factory() == 14);
    auto renamed = generated::VView::create(native::V{43});
    assert(renamed.view().renamed() == 43);
    auto keyword = generated::KeywordView::create(generated::Keyword{131});
    assert(keyword.view().kind() == 131);
    keyword.view().set_kind(137);
    assert(generated::KeywordViewReadOnly(keyword.bytes()).kind() == 137);
    view.Child().set_value(47);
    view.second().set_marker(53);
    view.set_Factory(59);
    view.set_ElementAccess(61);
    view.source().set_renamed(67);
    generated::NamesViewReadOnly readonly(names.bytes());
    static_assert(CanSetFactory<generated::NamesView>);
    static_assert(!CanSetFactory<generated::NamesViewReadOnly>);
    assert(readonly.Child().value() == 47 && readonly.second().marker() == 53);
    assert(readonly.Factory() == 59 && readonly.ElementAccess() == 61);
    assert(readonly.source().renamed() == 67);

    assert(generated::ModeView::create(native::Mode::On).view().tag() == 7);
    assert(generated::ModeView::create(native::SignedMode::Off).view().tag() == 9);
    const auto rejects_enum = [](auto value) {
        const auto rejects = [](auto&& operation) {
            bool rejected = false;
            try { operation(); } catch (const std::invalid_argument&) { rejected = true; }
            assert(rejected);
        };
        rejects([&] { generated::ModeView::compute_byte_length(value); });
        rejects([&] { generated::ModeView::create(value); });
        alignas(4) std::array<std::byte, 4> bytes;
        bytes.fill(std::byte{0xa5});
        rejects([&] { generated::ModeView::create_in_place(bytes, value); });
        intrlay::detail::Cursor cursor{4};
        rejects([&] { generated::ModeView::Metadata::write(bytes, 0, value, cursor); });
        assert(std::ranges::all_of(bytes, [](auto byte) { return byte == std::byte{0xa5}; }));
    };
    rejects_enum(native::Mode{});
    rejects_enum(static_cast<native::Mode>(0x100000007ULL));
    rejects_enum(static_cast<native::SignedMode>(-4294967289LL));

    auto ordinary = generated::OrdinaryChoiceView::create(native::OrdinaryChoice{native::Value{23, 29}});
    auto ordinary_copy = generated::OrdinaryChoiceView::create(
        generated::OrdinaryChoice{generated::OrdinaryChoiceValue{23, 29}});
    assert(std::ranges::equal(ordinary.bytes(), ordinary_copy.bytes()));
    assert(std::ranges::equal(ordinary.bytes(), read_bytes(directory / "ordinary.bin")));
    assert(ordinary.view().value_value().value().value() == 23);
    assert(ordinary.view().value_renamed_extra() == 29u);
    ordinary.view().set_value_value(71);
    ordinary.view().set_value_renamed_extra(73);
    assert(ordinary.view().value_value().value().value() == 71);
    assert(ordinary.view().value_renamed_extra() == 73u);
    auto absent = generated::OrdinaryChoiceView::create(native::OrdinaryChoice{native::Value{std::nullopt, 79}});
    assert(absent.view().value_value().has_value() && !absent.view().value_value().value());
    absent.view().set_value_value(83);
    assert(absent.view().value_value().value() == 83u);
    auto empty = generated::OrdinaryChoiceView::create(native::OrdinaryChoice{std::monostate{}});
    assert(!empty.view().value_value() && !empty.view().value_renamed_extra());

    std::variant<native::ThrowingValue, std::monostate> valueless{std::in_place_index<1>};
    try { valueless.emplace<0>(0); } catch (const std::runtime_error&) {}
    assert(valueless.valueless_by_exception());
    const auto rejects_valueless = [](auto&& operation) {
        bool rejected = false;
        try { operation(); } catch (const std::bad_variant_access&) { rejected = true; }
        assert(rejected);
    };
    rejects_valueless([&] { generated::OrdinaryChoiceView::compute_byte_length(valueless); });
    rejects_valueless([&] { generated::OrdinaryChoiceView::create(valueless); });
    alignas(4) std::array<std::byte, 32> untouched;
    untouched.fill(std::byte{0xa5});
    rejects_valueless([&] { generated::OrdinaryChoiceView::create_in_place(untouched, valueless); });
    assert(std::ranges::all_of(untouched, [](auto byte) { return byte == std::byte{0xa5}; }));
    intrlay::detail::Cursor invalid_cursor{generated::OrdinaryChoiceView::byte_size};
    rejects_valueless([&] { OrdinaryMetadata::write(untouched, 0, valueless, invalid_cursor); });

    auto whole = generated::WholeChoiceView::create(native::WholeChoice{native::Child{31, 37}});
    auto whole_copy = generated::WholeChoiceView::create(
        generated::WholeChoice{generated::Child{31, 37}});
    assert(std::ranges::equal(whole.bytes(), whole_copy.bytes()));
    assert(std::ranges::equal(whole.bytes(), read_bytes(directory / "whole.bin")));
    assert(whole.view().as_value()->value() == 31);
    whole.view().as_value()->set_marker(89);
    assert(generated::WholeChoiceViewReadOnly(whole.bytes()).as_value()->marker() == 89);
    auto nested = generated::WholeChoiceView::create(
        native::WholeChoice{std::array<std::optional<std::uint32_t>, 2>{41, std::nullopt}});
    auto nested_copy = generated::WholeChoiceView::create(
        generated::WholeChoice{std::array<std::optional<std::uint32_t>, 2>{41, std::nullopt}});
    assert(std::ranges::equal(nested.bytes(), nested_copy.bytes()));
    assert(std::ranges::equal(nested.bytes(), read_bytes(directory / "nested.bin")));
    auto payload = nested.view().as_nested().value();
    static_assert(decltype(payload)::extent == 2);
    payload.set(0, std::optional<std::uint32_t>{97});
    payload.set(1, std::optional<std::uint32_t>{101});
    assert(payload.get(0) == 97u && payload.get(1) == 101u);
    auto unit = generated::WholeChoiceView::create(native::WholeChoice{std::monostate{}});
    assert(unit.view().as_empty().has_value());
    assert(!unit.view().as_value() && !unit.view().as_nested());

    namespace custom = cpp_metadata::custom;
    constexpr custom::ApiChild custom_input{103, 107};
    static_assert(CustomMetadata::Field<0>::get(custom_input) == 103);
    auto custom_owned = custom::ApiChildView::BuildCreate(custom_input);
    auto custom_native = custom::ApiChildView::BuildCreate(native::Child{103, 107});
    assert(std::ranges::equal(custom_owned.bytes(), custom_native.bytes()));
    assert(custom::ApiChildView::BuildComputeByteLength(custom_input) == 8);
    assert(custom_owned.view().ReadValue() == 103 && custom_owned.view().ReadMarker() == 107);
    custom_owned.view().set_ReadValue(109);
    custom::ApiChildViewReadOnly custom_readonly(custom_owned.bytes());
    assert(custom_readonly.ReadValue() == 109 && custom_readonly.ReadBytes().size() == 8);
    alignas(4) std::array<std::byte, 8> in_place{};
    auto custom_in_place = custom::ApiChildView::BuildCreateInPlace(in_place, native::Child{113, 127});
    assert(custom_in_place.ReadValue() == 113 && custom_in_place.ReadMarker() == 127);

    for (const auto& text : {std::string{}, std::string("a\0b", 3), std::string(37, 'x')}) {
        const auto expected = generated::TextView::create(generated::Text{text});
        const native::Text source{std::basic_string<char, native::CharacterTraits>{text.data(), text.size()}};
        auto encoded_text = generated::TextView::create(source);
        assert(std::ranges::equal(encoded_text.bytes(), expected.bytes()));
        assert(encoded_text.view().value() == text);
        const auto borrowed = generated::TextView::create(native::Text{
            std::basic_string_view<char, native::CharacterTraits>{text.data(), text.size()}});
        assert(std::ranges::equal(borrowed.bytes(), expected.bytes()));

        std::pmr::monotonic_buffer_resource arena;
        const native::Text allocated{std::basic_string<char, native::CharacterTraits,
            std::pmr::polymorphic_allocator<char>>{text.data(), text.size(), {&arena}}};
        const auto custom_allocator = generated::TextView::create(allocated);
        assert(std::ranges::equal(custom_allocator.bytes(), expected.bytes()));

        alignas(4) std::array<std::byte, 128> storage;
        storage.fill(std::byte{0xa5});
        const auto written = generated::TextView::create_in_place(storage, source);
        assert(written.value() == text);
        assert(std::ranges::equal(written.bytes(), expected.bytes()));
        assert(std::ranges::all_of(std::span(storage).subspan(expected.bytes().size()),
            [](auto byte) { return byte == std::byte{0xa5}; }));
    }
    assert(generated::TextView::create(native::Text{"literal"}).view().value() == "literal");
    std::cout << "C++ metadata regressions passed\n";
}
