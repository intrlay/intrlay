#include "bindings.hpp"
#include "value-enums.hpp"
#include <cassert>
#include <iostream>
#include <fstream>

template<class View>
concept CanReadWriteOnlyEnum = requires(const View& view) { view.write_only(); };

template<class View>
concept CanSetReadOnlyEnum = requires(const View& view, value_enums::PaddedState value) {
    view.set_read_only(value);
};

void value_enum_regressions() {
    using namespace value_enums;
    using Kind = PaddedState;
    static_assert(std::same_as<SparseState, StateAlias>);
    static_assert(NestedEnumValuesView::alignment == 16);
    static_assert(std::same_as<decltype(std::declval<NestedEnumValuesView>().state()), Kind>);
    static_assert(std::same_as<decltype(std::declval<NestedEnumValuesViewReadOnly>().state()), Kind>);
    static_assert(std::same_as<decltype(std::declval<NestedEnumValuesView>().optional()), std::optional<Kind>>);
    static_assert(std::same_as<decltype(std::declval<NestedEnumValuesView>().boxed()), Kind>);
    static_assert(CanReadWriteOnlyEnum<NestedEnumValuesView>);
    static_assert(CanSetReadOnlyEnum<NestedEnumValuesView>);
    static_assert(!CanSetReadOnlyEnum<NestedEnumValuesViewReadOnly>);

    NestedEnumValuesInput<> input{
        Kind::Last, Kind::Middle, std::make_unique<Kind>(Kind::First),
        {Kind::First, Kind::Last}, {Kind::Middle, Kind::First},
        {{Kind::Middle, Kind::First}}, {Kind::First, Kind::Last}, Kind::First, Kind::Middle,
    };
    auto encoded = NestedEnumValuesView::create(input);
    std::ifstream file("target/compositions/nested-enums.bin", std::ios::binary);
    std::vector<char> expected{std::istreambuf_iterator<char>(file), {}};
    assert(expected.size() == encoded.bytes().size());
    for (std::size_t index = 0; index < expected.size(); ++index)
        assert(encoded.bytes()[index] == static_cast<std::byte>(expected[index]));

    auto view = encoded.view_unchecked();
    assert(view.state() == Kind::Last && view.optional() == Kind::Middle && view.boxed() == Kind::First);
    assert(view.fixed().get(0) == Kind::First && view.fixed().get(1) == Kind::Last);
    assert(view.states().get(0) == Kind::Middle && view.states().get(1) == Kind::First);
    assert(view.indexed().get(0).state() == Kind::Middle && view.indexed().get(0).optional() == Kind::First);
    assert(view.projected().get(0) == Kind::First && view.projected().get(1) == Kind::Last);
    assert(view.read_only() == Kind::Middle);
    view.set_state(Kind::First);
    view.set_optional(Kind::Last);
    view.set_boxed(Kind::Last);
    view.fixed().set(1, Kind::Middle);
    view.states().set(0, Kind::First);
    view.indexed().get(0).set_state(Kind::Last);
    view.indexed().get(0).set_optional(std::nullopt);
    assert(!view.indexed().get(0).optional());
    view.indexed().get(0).set_optional(Kind::Last);
    view.projected().set(1, Kind::Middle);
    view.set_write_only(Kind::Last);
    view.set_read_only(Kind::First);
    assert(view.state() == Kind::First && view.optional() == Kind::Last && view.boxed() == Kind::Last);
    assert(view.fixed().get(1) == Kind::Middle && view.states().get(0) == Kind::First);
    assert(view.indexed().get(0).state() == Kind::Last && view.indexed().get(0).optional() == Kind::Last);
    assert(view.projected().get(1) == Kind::Middle);
    assert(view.write_only() == Kind::Last && view.read_only() == Kind::First);

    bool rejected = false;
    try { view.set_state(static_cast<Kind>(5)); }
    catch (const std::invalid_argument&) { rejected = true; }
    assert(rejected && view.state() == Kind::First);
    auto root = StateAliasView::create(StateAlias::Last);
    assert(root.view_unchecked().tag() == SparseState::Last);
    root.view_unchecked().replace(SparseState::First);
    assert(root.view_unchecked().tag() == StateAlias::First);
}

int main() {
    value_enum_regressions();
    using namespace composition;
    auto zero_input = ZeroedArraysInput{73u, intrlay::zeroed(3), intrlay::zeroed(4), intrlay::zeroed(),
        std::optional<intrlay::Zeroed>{intrlay::zeroed()},
        std::array{intrlay::zeroed(2), intrlay::zeroed(0)}, intrlay::zeroed()};
    auto zero_buffer = ZeroedArraysView::create(zero_input);
    std::ifstream zero_file("target/compositions/zeroed.bin", std::ios::binary);
    std::vector<char> expected{std::istreambuf_iterator<char>(zero_file), {}};
    assert(expected.size() == zero_buffer.bytes().size());
    for (std::size_t i = 0; i < expected.size(); ++i)
        assert(zero_buffer.bytes()[i] == static_cast<std::byte>(expected[i]));
    std::fill(zero_buffer.bytes().begin(), zero_buffer.bytes().end(), std::byte{0xa5});
    ZeroedArraysView::create_in_place(zero_buffer.bytes(), zero_input);
    for (std::size_t i = 0; i < expected.size(); ++i)
        assert(zero_buffer.bytes()[i] == static_cast<std::byte>(expected[i]));
    static_assert(!intrlay::EncodeSourceFor<intrlay::ZeroedSequence, intrlay::SequenceCodec<intrlay::StringCodec, 8, 4>>);
    static_assert(!intrlay::EncodeSourceFor<intrlay::ZeroedSequence, intrlay::ArrayCodec<intrlay::ScalarCodec<float>, 4, 3>>);
    auto names = RenamedListView::create(RenamedListInput<>{{RenamedInput<>{11, 22}}});
    assert(names.view_unchecked().items().get(0).first() == 11);
    names.view_unchecked().items().get(0).set_second(23u);
    assert(names.view_unchecked().items().get(0).second() == 23);

    WrappedScalarsInput<> input;
    input.present = 1;
    input.boxed = std::make_unique<std::uint32_t>(2);
    input.nested = std::make_unique<std::optional<std::uint32_t>>(3);
    input.missing_inner = std::make_unique<std::optional<std::uint32_t>>();
    input.items.push_back(std::make_unique<std::uint32_t>(4));
    input.items.push_back(std::nullopt);
    input.fixed = {5, std::nullopt};
    input.read_only = 6; input.write_only = 7;
    auto owned = WrappedScalarsView::create(input);
    auto view = owned.view_unchecked();
    view.set_read_only(100u);
    assert(view.write_only() == 7 && view.read_only() == 100);
    view.set_present(11u); view.set_absent(12u); view.set_boxed(13u);
    view.set_nested(14u); view.set_missing_inner(15u); view.set_write_only(20u);
    view.items().set(0, 16u); view.items().set(1, 17u);
    view.fixed().set(0, std::optional<std::uint32_t>{18});
    view.fixed().set(1, std::optional<std::uint32_t>{19});
    assert(view.present() == 11 && view.absent() == 12 && view.boxed() == 13);
    assert(view.nested().value().value() == 14 && view.missing_inner().value() == 15);
    assert(view.items().get(0) == 16 && !view.items().get(1));
    assert(view.fixed().get(0) == 18 && view.fixed().get(1) == 19);
    std::optional<std::unique_ptr<std::optional<std::uint32_t>>> root_value{std::make_unique<std::optional<std::uint32_t>>(2)};
    auto root = OptionalRootView::create(root_value);
    root.view_unchecked().set(9u);
    assert(root.view_unchecked().value().value().value() == 9);
    std::cout << "C++ composition regressions passed\n";
}
