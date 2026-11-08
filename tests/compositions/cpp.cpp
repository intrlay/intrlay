#include "bindings.hpp"
#include <cassert>
#include <iostream>
#include <fstream>

int main() {
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
    assert(names.view().items().get(0).first() == 11);
    names.view().items().get(0).set_second(23u);
    assert(names.view().items().get(0).second() == 23);

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
    auto view = owned.view();
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
    root.view().set(9u);
    assert(root.view().value().value().value() == 9);
    std::cout << "C++ composition regressions passed\n";
}
