#ifdef NDEBUG
#undef NDEBUG
#endif
#include "bindings.hpp"
#include "custom.hpp"
#include "enum_getters.hpp"
#include "../../target/naming/enum_properties.hpp"
#include "peer.hpp"
#include <cassert>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <memory_resource>

#ifdef INTRLAY_CHECK_LINKAGE
auto child_value_reader_from_peer()
    -> decltype(&cpp_metadata::ChildView::Metadata::Field<0>::read<const std::byte>);
#endif

namespace native {
struct CharacterTraits : std::char_traits<char> {};
template<class T> struct Text { T value; };
struct AdlSizedWords {
    const std::uint32_t* values;
    const std::uint32_t* begin() const { return values; }
    const std::uint32_t* end() const { return values + 2; }
    const std::uint32_t& operator[](std::size_t index) const { return values[index]; }
    friend std::size_t size(const AdlSizedWords&) { return 2; }
};
enum class Mode : std::uint64_t { On = 7, Off = 9 };
enum class SignedMode : std::int64_t { On = 7, Off = 9 };
struct Child {
    std::uint32_t value;
    std::uint32_t marker;
};
struct V {
    using intrlay_input = void;
    std::uint32_t native_value;
};
struct Names {
    // An unrelated alias must not select generated-field names for native sources.
    using intrlay_input = int;
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

template<class Metadata, class Visitor>
concept CanVisitFields = requires(Visitor& visitor) { Metadata::for_each_field(visitor); };

template<class Codec, class Source>
concept CanMeasureSource = requires(const Source& source, intrlay::detail::Cursor& cursor) {
    Codec::measure(source, cursor);
};

struct FirstFieldVisitor {
    template<std::same_as<NamesMetadata::Field<0>> Field>
    void operator()() const {}
};

static_assert(!CanVisitFields<NamesMetadata, int>);
static_assert(!CanVisitFields<NamesMetadata, FirstFieldVisitor>);
static_assert(!CanMeasureSource<NamesMetadata, int>);
static_assert(!intrlay::SetSourceFor<std::string, NamesMetadata::Field<2>>);
static_assert(!intrlay::SetSourceFor<int, cpp_metadata::ChildView::Metadata>);
static_assert(!intrlay::SetSourceFor<int, intrlay::Optional<intrlay::Scalar<std::uint32_t>, 4, 8>>);
static_assert(intrlay::SetSourceFor<std::optional<std::uint32_t>, intrlay::Optional<intrlay::Scalar<std::uint32_t>, 4, 8>>);

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
static_assert(!intrlay::EncodeSourceFor<native::Text<volatile std::string>, cpp_metadata::TextView::Metadata>);
static_assert(!intrlay::EncodeSourceFor<volatile native::Text<std::string>, cpp_metadata::TextView::Metadata>);
static_assert(!intrlay::EncodeSourceFor<volatile cpp_metadata::OrdinaryChoiceInput<>, OrdinaryMetadata>);
static_assert(!intrlay::EncodeSourceFor<void, OrdinaryMetadata>);
static_assert(!intrlay::SetSourceFor<int, OrdinaryMetadata>);
static_assert(std::ranges::sized_range<const native::AdlSizedWords>);
static_assert(!intrlay::EncodeSourceFor<native::AdlSizedWords,
    intrlay::SequenceCodec<intrlay::Scalar<std::uint32_t>, 4, 4>>);
static_assert(!intrlay::EncodeSourceFor<native::AdlSizedWords,
    intrlay::ArrayCodec<intrlay::Scalar<std::uint32_t>, 4, 2>>);
static_assert(std::is_same_v<cpp_metadata::TextAliasInput<>, cpp_metadata::TextInput<>>);
static_assert(std::is_same_v<cpp_metadata::ChildAliasInput<>, cpp_metadata::ChildInput<>>);
static_assert(std::is_same_v<cpp_metadata::OrdinaryChoiceAliasInput<>, cpp_metadata::OrdinaryChoiceInput<>>);
static_assert(std::is_same_v<cpp_metadata::ModeAlias, cpp_metadata::Mode>);
template<class... Fields>
concept CanDeduceChildInput = requires(Fields... fields) { cpp_metadata::ChildInput{fields...}; };
template<class... Fields>
concept CanDeduceChildAliasInput = requires(Fields... fields) { cpp_metadata::ChildAliasInput{fields...}; };
static_assert(CanDeduceChildInput<> && CanDeduceChildInput<std::uint32_t, std::uint32_t> && !CanDeduceChildInput<int, int>);
static_assert(!CanDeduceChildInput<int> && !CanDeduceChildInput<int, int, int>);
static_assert(CanDeduceChildAliasInput<> && CanDeduceChildAliasInput<std::uint32_t, std::uint32_t> && !CanDeduceChildAliasInput<int, int>);
static_assert(!CanDeduceChildAliasInput<int> && !CanDeduceChildAliasInput<int, int, int>);
static_assert(!CanDeduceChildAliasInput<std::uint16_t, std::uint16_t>);
// MSVC 19.42 crashes if alias CTAD precedes these deduction requires-expressions.
// Keep both checks, instantiating the constrained probes before the direct use.
static_assert(std::is_same_v<decltype(cpp_metadata::ChildAliasInput{}), cpp_metadata::ChildInput<>>);
static_assert(intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, native::Child>);
static_assert(intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, cpp_metadata::ChildInput<>>);
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, int>);
// The unmapped byte introduces native padding without changing a schema field's type.
struct PaddedChild { std::uint32_t value; std::uint8_t unused; std::uint32_t marker; };
struct WidenedChild { std::uint32_t value; std::uint64_t marker; };
struct ReorderedChild { std::uint32_t marker; std::uint32_t value; };
struct VolatileChild { volatile std::uint32_t value; std::uint32_t marker; };
struct StaticChild {
    static constexpr std::uint32_t value = 71;
    std::uint32_t marker;
};
struct AllStaticChild {
    static constexpr std::uint32_t value = 73;
    static constexpr std::uint32_t marker = 79;
};
struct StaticPaddedChild {
    static constexpr std::uint32_t value = 83;
    std::uint32_t marker;
    std::uint32_t unused;
};
static_assert(sizeof(StaticPaddedChild) == cpp_metadata::ChildView::byte_size);
static_assert(intrlay::EncodeSourceFor<StaticChild, cpp_metadata::ChildView::Metadata>);
static_assert(intrlay::EncodeSourceFor<AllStaticChild, cpp_metadata::ChildView::Metadata>);
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, StaticChild>);
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, AllStaticChild>);
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, StaticPaddedChild>);

struct StaticNames;
struct AddressChild {
    std::uint32_t value;
    std::uint32_t marker;
    AddressChild StaticNames::* operator&() const;
};
struct StaticNames {
    AddressChild first;
    inline static AddressChild second{97, 101};
    std::uint32_t factory;
    std::uint32_t element_access;
    native::V source;
};
AddressChild StaticNames::* AddressChild::operator&() const { return &StaticNames::first; }
static_assert(std::is_member_object_pointer_v<decltype(&StaticNames::second)>);
static_assert(intrlay::EncodeSourceFor<StaticNames, NamesMetadata>);
static_assert(!intrlay::detail::native_layout<NamesMetadata, StaticNames>);

struct BitfieldChild { std::uint32_t value : 16; std::uint32_t marker; };
struct FullWidthBitfieldChild { std::uint32_t value : 32; std::uint32_t marker; };
struct PolymorphicChild {
    virtual ~PolymorphicChild() = default;
    std::uint32_t value = 61;
    std::uint32_t marker = 67;
};
struct PolymorphicBitfieldChild {
    virtual ~PolymorphicBitfieldChild() = default;
    std::uint32_t value : 16 = 61;
    std::uint32_t marker = 67;
};

template<class Source>
concept CanCreateChild = requires(const Source& source) { cpp_metadata::ChildView::create(source); };

// A bitfield cannot back a borrowed member reference, even outside native layout.
static_assert(!intrlay::EncodeSourceFor<BitfieldChild, cpp_metadata::ChildView::Metadata>);
static_assert(!intrlay::EncodeSourceFor<FullWidthBitfieldChild, cpp_metadata::ChildView::Metadata>);
static_assert(!intrlay::EncodeSourceFor<PolymorphicBitfieldChild, cpp_metadata::ChildView::Metadata>);
static_assert(!intrlay::SetSourceFor<BitfieldChild, cpp_metadata::ChildView::Metadata>);
static_assert(!intrlay::SetSourceFor<PolymorphicBitfieldChild, cpp_metadata::ChildView::Metadata>);
static_assert(!CanCreateChild<BitfieldChild> && !CanCreateChild<PolymorphicBitfieldChild>);
static_assert(CanCreateChild<PolymorphicChild>);
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, BitfieldChild>);
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, PolymorphicBitfieldChild>);

#ifdef INTRLAY_TEST_INVALID_SOURCES
void invalid_bitfield_source(const PolymorphicBitfieldChild& source) {
    cpp_metadata::ChildView::create(source);
}
#endif

struct VolatileNames {
    volatile native::Child first;
    native::Child second;
    std::uint32_t factory;
    std::uint32_t element_access;
    native::V source;
};
static_assert(!intrlay::detail::native_layout<cpp_metadata::ChildView::Metadata, PaddedChild>);
static_assert(!intrlay::EncodeSourceFor<WidenedChild, cpp_metadata::ChildView::Metadata>);
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
concept CanSetFactory = requires(T view) { view.set_Factory(1u); };

int main(int argc, char** argv) {
    namespace payload_names = intrlay_generated::payload_names;
    auto named_payload = payload_names::PayloadEventView::create(payload_names::PayloadEventInput<>{
        payload_names::PayloadEventVariantTaskDoneInput<>{7, 11}});
    auto payload_view = named_payload.view_unchecked();
    assert(payload_view.read_task_done_request_id() == 7u);
    assert(payload_view.read_task_done_saved_id() == 11u);
    payload_view.set_read_task_done_request_id(std::uint32_t{19});
    assert(payload_view.read_task_done_request_id() == 19u);
    payload_view.replace(payload_names::PayloadEventInput<>{payload_names::PayloadEventDeliveredInput<>{23}});
    payload_view.set_read_delivered_ticket(std::uint32_t{29});
    assert(payload_view.read_delivered_ticket() == 29u);
    assert(!payload_view.read_task_done_request_id());

    namespace generated = cpp_metadata;
    const std::filesystem::path directory = argc > 1 ? argv[1] : "target/cpp-metadata";

    namespace getter_names = cpp_metadata::getter_names;
    auto named_whole = getter_names::WholeChoiceView::create(native::WholeChoice{native::Child{31, 37}});
    assert(named_whole.view_unchecked().read_as_value()->read_value() == 31);
    auto named_ordinary = getter_names::OrdinaryChoiceView::create(native::OrdinaryChoice{native::Value{23, 29}});
    assert(named_ordinary.view_unchecked().read_value_value().value().value() == 23);

    auto peer = generated::PeerView::create(generated::PeerInput<>{173, 179});
    assert(peer.view_unchecked().value() == 173 && peer.view_unchecked().marker() == 179);
    const auto alias_child = generated::ChildAliasInput{std::uint32_t{3}, std::uint32_t{5}};
    static_assert(std::is_same_v<decltype(alias_child), const generated::ChildInput<std::uint32_t, std::uint32_t>>);
    auto aliased_child = generated::ChildAliasView::create(alias_child);
    assert(aliased_child.view_unchecked().value() == 3 && aliased_child.view_unchecked().marker() == 5);

#ifdef INTRLAY_CHECK_LINKAGE
    // The CMake consumer includes the same generated metadata in another TU.
    // Its field specialization must denote the same function in both files.
    assert(child_value_reader_from_peer() ==
        &generated::ChildView::Metadata::Field<0>::read<const std::byte>);
#endif

    volatile native::Child volatile_root{41, 43};
    auto root = generated::ChildView::create(volatile_root);
    assert(root.view_unchecked().value() == 41 && root.view_unchecked().marker() == 43);
    auto member = generated::ChildView::create(VolatileChild{47, 53});
    assert(member.view_unchecked().value() == 47);
    auto polymorphic = generated::ChildView::create(PolymorphicChild{});
    assert(polymorphic.view_unchecked().value() == 61 && polymorphic.view_unchecked().marker() == 67);
    auto static_child = generated::ChildView::create(StaticChild{89});
    assert(static_child.view_unchecked().value() == 71 && static_child.view_unchecked().marker() == 89);
    static_child.view_unchecked().replace(AllStaticChild{});
    assert(static_child.view_unchecked().value() == 73 && static_child.view_unchecked().marker() == 79);
    static_child.view_unchecked().replace(StaticPaddedChild{107, 0xffffffffu});
    assert(static_child.view_unchecked().value() == 83 && static_child.view_unchecked().marker() == 107);
    auto static_names = generated::NamesView::create(StaticNames{{109, 113}, 127, 131, {137}});
    assert(static_names.view_unchecked().Child().value() == 109 && static_names.view_unchecked().Child().marker() == 113);
    assert(static_names.view_unchecked().second().value() == 97 && static_names.view_unchecked().second().marker() == 101);
    const std::array<StaticChild, 2> static_children{{{139}, {149}}};
    using StaticChildren = intrlay::SequenceCodec<generated::ChildView::Metadata, 8, 4>;
    std::array<std::byte, 24> static_bytes{};
    intrlay::detail::Cursor static_cursor{8};
    StaticChildren::write(static_bytes, 0, static_children, static_cursor);
    const auto static_first = generated::ChildView::from_parts(static_bytes, 8).value();
    const auto static_second = generated::ChildView::from_parts(static_bytes, 16).value();
    assert(static_first.value() == 71 && static_first.marker() == 139);
    assert(static_second.value() == 71 && static_second.marker() == 149);
    auto volatile_nested = generated::NamesView::create(VolatileNames{{3, 5}, {7, 11}, 13, 17, {19}});
    assert(volatile_nested.view_unchecked().Child().value() == 3);
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
    const generated::NamesInput<> generated_names{{3, 5}, {7, 11}, 13, 17, {19}};
    auto names = generated::NamesView::create(native_names);
    auto reordered = generated::ChildView::create(ReorderedChild{17, 13});
    assert(reordered.view_unchecked().value() == 13 && reordered.view_unchecked().marker() == 17);
    auto padded_child = generated::ChildView::create(PaddedChild{19, 0xff, 23});
    assert(padded_child.view_unchecked().value() == 19 && padded_child.view_unchecked().marker() == 23);
    auto names_copy = generated::NamesView::create(generated_names);
    assert(std::ranges::equal(names.bytes(), names_copy.bytes()));
    assert(std::ranges::equal(names.bytes(), read_bytes(directory / "names.bin")));
    auto view = names.view_unchecked();
    assert(view.Child().value() == 3 && view.second().value() == 7);
    assert(view.Factory() == 13 && view.ElementAccess() == 17);
    assert(view.source().renamed() == 19);
    assert(NamesMetadata::Field<2>::read(names.bytes(), 0) == 13);
    NamesMetadata::Field<2>::set(names.bytes(), 0, 14u);
    assert(view.Factory() == 14);
    auto renamed = generated::VView::create(native::V{43});
    assert(renamed.view_unchecked().renamed() == 43);
    auto keyword = generated::KeywordView::create(generated::KeywordInput<>{131, 139});
    assert(keyword.view_unchecked().restricted() == 139);
    keyword.view_unchecked().set_restricted(149u);
    assert(generated::KeywordViewReadOnly::from_bytes(keyword.bytes()).value().restricted() == 149);
    assert(keyword.view_unchecked().kind() == 131);
    keyword.view_unchecked().set_kind(137u);
    assert(generated::KeywordViewReadOnly::from_bytes(keyword.bytes()).value().kind() == 137);
    view.Child().set_value(47u);
    view.second().set_marker(53u);
    view.set_Factory(59u);
    view.set_ElementAccess(61u);
    view.source().set_renamed(67u);
    auto readonly = generated::NamesViewReadOnly::from_bytes(names.bytes()).value();
    static_assert(CanSetFactory<generated::NamesView>);
    static_assert(!CanSetFactory<generated::NamesViewReadOnly>);
    assert(readonly.Child().value() == 47 && readonly.second().marker() == 53);
    assert(readonly.Factory() == 59 && readonly.ElementAccess() == 61);
    assert(readonly.source().renamed() == 67);

    assert(generated::ModeView::create(native::Mode::On).view_unchecked().tag() == generated::Mode::On);
    assert(generated::ModeView::create(native::SignedMode::Off).view_unchecked().tag() == generated::Mode::Off);
    assert(generated::ModeAliasView::create(generated::ModeAlias::On).view_unchecked().tag() == generated::Mode::On);
    generated::ModeFieldsInput<> default_modes;
    assert(static_cast<std::uint32_t>(default_modes.mode) == 0);
    assert(std::ranges::all_of(default_modes.modes,
        [](auto mode) { return static_cast<std::uint32_t>(mode) == 0; }));
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
        generated::OrdinaryChoiceInput<>{generated::OrdinaryChoiceValueInput<>{23, 29}});
    const auto alias_choice = generated::OrdinaryChoiceAliasInput{
        std::variant<generated::OrdinaryChoiceValueInput<>, generated::OrdinaryChoiceEmptyInput<>>{
            generated::OrdinaryChoiceValueInput<>{23, 29}}};
    auto alias_copy = generated::OrdinaryChoiceAliasView::create(alias_choice);
    assert(std::ranges::equal(ordinary.bytes(), alias_copy.bytes()));
    assert(std::ranges::equal(ordinary.bytes(), ordinary_copy.bytes()));
    assert(std::ranges::equal(ordinary.bytes(), read_bytes(directory / "ordinary.bin")));
    assert(ordinary.view_unchecked().value_value().value().value() == 23);
    assert(ordinary.view_unchecked().value_renamed_extra() == 29u);
    ordinary.view_unchecked().set_value_value(71u);
    ordinary.view_unchecked().set_value_renamed_extra(73u);
    assert(ordinary.view_unchecked().value_value().value().value() == 71);
    assert(ordinary.view_unchecked().value_renamed_extra() == 73u);
    auto absent = generated::OrdinaryChoiceView::create(native::OrdinaryChoice{native::Value{std::nullopt, 79}});
    assert(absent.view_unchecked().value_value().has_value() && !absent.view_unchecked().value_value().value());
    absent.view_unchecked().set_value_value(83u);
    assert(absent.view_unchecked().value_value().value() == 83u);
    auto empty = generated::OrdinaryChoiceView::create(native::OrdinaryChoice{std::monostate{}});
    assert(!empty.view_unchecked().value_value() && !empty.view_unchecked().value_renamed_extra());

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
        generated::WholeChoiceInput<>{generated::ChildInput<>{31, 37}});
    assert(std::ranges::equal(whole.bytes(), whole_copy.bytes()));
    assert(std::ranges::equal(whole.bytes(), read_bytes(directory / "whole.bin")));
    assert(whole.view_unchecked().as_value()->value() == 31);
    whole.view_unchecked().as_value()->set_marker(89u);
    assert(generated::WholeChoiceViewReadOnly::from_bytes(whole.bytes()).value().as_value()->marker() == 89);
    auto nested = generated::WholeChoiceView::create(
        native::WholeChoice{std::array<std::optional<std::uint32_t>, 2>{41, std::nullopt}});
    auto nested_copy = generated::WholeChoiceView::create(
        generated::WholeChoiceInput<>{std::array<std::optional<std::uint32_t>, 2>{41, std::nullopt}});
    assert(std::ranges::equal(nested.bytes(), nested_copy.bytes()));
    assert(std::ranges::equal(nested.bytes(), read_bytes(directory / "nested.bin")));
    auto payload = nested.view_unchecked().as_nested().value();
    static_assert(decltype(payload)::extent == 2);
    payload.set(0, std::optional<std::uint32_t>{97});
    payload.set(1, std::optional<std::uint32_t>{101});
    assert(payload.get(0) == 97u && payload.get(1) == 101u);
    auto unit = generated::WholeChoiceView::create(native::WholeChoice{std::monostate{}});
    assert(unit.view_unchecked().as_empty().has_value());
    assert(!unit.view_unchecked().as_value() && !unit.view_unchecked().as_nested());

    namespace custom = cpp_metadata::custom;
    constexpr custom::ApiChildInput<> custom_input{103, 107};
    static_assert(CustomMetadata::Field<0>::get(custom_input) == 103);
    auto custom_owned = custom::ApiChildView::BuildCreate(custom_input);
    auto custom_native = custom::ApiChildView::BuildCreate(native::Child{103, 107});
    assert(std::ranges::equal(custom_owned.bytes(), custom_native.bytes()));
    assert(custom::ApiChildView::BuildComputeByteLength(custom_input) == 8);
    assert(custom_owned.view_unchecked().ReadValue() == 103 && custom_owned.view_unchecked().ReadMarker() == 107);
    custom_owned.view_unchecked().set_ReadValue(109u);
    auto custom_readonly = custom::ApiChildViewReadOnly::BuildFromBytes(custom_owned.bytes()).value();
    assert(custom_readonly.ReadValue() == 109 && custom_readonly.ReadBytes().size() == 8);
    alignas(4) std::array<std::byte, 8> in_place{};
    auto custom_in_place = custom::ApiChildView::BuildCreateInPlace(in_place, native::Child{113, 127});
    assert(custom_in_place.ReadValue() == 113 && custom_in_place.ReadMarker() == 127);

    for (const auto& text : {std::string{}, std::string("a\0b", 3), std::string(37, 'x')}) {
        const auto expected = generated::TextView::create(generated::TextInput<>{text});
        const auto alias_input = generated::TextAliasInput{std::string_view(text)};
        static_assert(std::is_same_v<decltype(alias_input.value), std::string_view>);
        assert(alias_input.value.data() == text.data());
        auto alias_text = generated::TextAliasView::create(alias_input);
        assert(std::ranges::equal(alias_text.bytes(), expected.bytes()));
        const native::Text source{std::basic_string<char, native::CharacterTraits>{text.data(), text.size()}};
        auto encoded_text = generated::TextView::create(source);
        assert(std::ranges::equal(encoded_text.bytes(), expected.bytes()));
        assert(encoded_text.view_unchecked().value() == text);
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
    assert(generated::TextView::create(native::Text{"literal"}).view_unchecked().value() == "literal");
    std::cout << "C++ metadata regressions passed\n";
}
