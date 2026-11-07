#ifdef NDEBUG
#undef NDEBUG
#endif
#include "bindings.hpp"
#include "custom.hpp"
#include <cassert>
#include <filesystem>
#include <fstream>
#include <iostream>

namespace generated = cpp_collections;

template<class View, class Value>
concept CanSet = requires(View view, Value value) { view.set(0, value); };

template<class View, class Value>
concept CanCreate = requires(Value value) { View::create(value); };

template<class View, class Value>
concept CanAccessSet = requires(View view, Value value) { view.AccessSet(0, value); };

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

int main(int argc, char** argv) {
    const std::filesystem::path directory = argc > 1 ? argv[1] : "target/cpp-collections";
    generated::Collections input;
    input.fixed = {1, 2, 3};
    input.nested = {{4, 5, 6}, {7, 8, 9}};
    input.optional[0] = std::make_unique<std::uint32_t>(11);
    input.optional[2] = std::make_unique<std::uint32_t>(13);
    input.tree = {{17, {{19, {}}}}};
    input.padded = {{{23}, {29}}};
    input.units.resize(3);
    input.flags = {true, false};
    input.borrowed.values = {31, 37};

    auto owned = generated::CollectionsView::create(input);
    const auto reference = read_bytes(directory / "collections.bin");
    assert(std::ranges::equal(owned.bytes(), reference));
    auto view = owned.view();
    using Fixed = decltype(view.fixed());
    static_assert(std::is_same_v<Fixed, intrlay::ArrayView<typename Fixed::Metadata>>);
    static_assert(Fixed::extent == 3 && Fixed::byte_size == 12);
    static_assert(CanSet<Fixed, float>);
    static_assert(!CanSet<decltype(generated::CollectionsViewReadOnly(reference).fixed()), float>);
    static_assert(!CanSet<decltype(view.tree()), generated::Branch>);
    static_assert(CanSet<decltype(view.nested()), std::array<float, 3>>);
    static_assert(!CanCreate<generated::BorrowOnlyView, generated::BorrowOnly>);
    static_assert(decltype(view.nested())::extent == std::dynamic_extent);
    assert(view.nested().get(1).get(2) == 9);
    assert(view.tree().get(0).children().get(0).value() == 19);
    assert(view.tree().get(0).children().get(0).children().size() == 0);
    assert(view.padded().get(0).value() == 23 && view.padded().get(1).value() == 29);
    assert(view.empty().size() == 0 && decltype(view.empty())::extent == 0);
    assert(view.units().size() == 3 && view.units().get(2) == std::monostate{});
    assert(view.flags().get(0) && !view.flags().get(1));

    view.fixed().set(0, 41);
    view.nested().set(1, std::array<float, 3>{7, 8, 43});
    view.optional().set(0, 47);
    view.optional().set(1, 53); // An absent element stays absent.
    view.tree().get(0).children().get(0).set_value(59);
    view.flags().set(1, true);
    generated::CollectionsViewReadOnly readonly(owned.bytes());
    assert(readonly.fixed().get(0) == 41 && readonly.nested().get(1).get(2) == 43);
    assert(readonly.optional().get(0) == 47 && !readonly.optional().get(1));
    assert(readonly.tree().get(0).children().get(0).value() == 59);
    assert(readonly.flags().get(1));

    // External const storage needs no lifetime preparation or writable cast.
    static constexpr auto bytes = std::bit_cast<std::array<std::byte, 12>>(std::array<float, 3>{61, 67, 71});
    generated::IntrlayArray3OfF32ViewReadOnly borrowed(bytes);
    assert(borrowed.get(0) == 61 && borrowed.get(2) == 71);
    auto triplet = generated::TripletView::create(std::array<float, 3>{61, 67, 71});
    assert(std::ranges::equal(triplet.bytes(), bytes));
    assert(generated::TripletView::compute_byte_length(std::array<float, 3>{}) == 12);
    alignas(4) std::array<std::byte, 24> storage{};
    auto in_place = Fixed::create_in_place(std::span(storage).subspan(8), std::array<float, 3>{73, 79, 83});
    assert(in_place.bytes().size() == 12 && in_place.get(1) == 79);
    assert(Fixed(storage, 8).get(2) == 83);
    const auto const_copy = generated::IntrlayArray3OfF32ViewReadOnly::create(std::array<float, 3>{89, 97, 101});
    assert(generated::IntrlayArray3OfF32ViewReadOnly(const_copy.bytes()).get(2) == 101);

    std::vector<std::optional<std::unique_ptr<std::uint32_t>>> scalar_input;
    scalar_input.emplace_back(std::make_unique<std::uint32_t>(103));
    scalar_input.emplace_back();
    auto sequence = generated::OptionalSequenceView::create(scalar_input);
    sequence.view().set(0, 107);
    sequence.view().set(1, 109);
    assert(sequence.view().get(0) == 107 && !sequence.view().get(1));
    using Empty = decltype(view.empty());
    auto empty = Empty::create(std::array<double, 0>{});
    assert(empty.bytes().empty() && empty.view().size() == 0);

    namespace custom = cpp_collections::custom;
    auto named = custom::ApiCollectionsView::BuildCreate(input);
    assert(std::ranges::equal(named.bytes(), reference));
    auto named_view = named.view();
    auto named_fixed = named_view.ReadFixed();
    using NamedFixed = decltype(named_fixed);
    static_assert(NamedFixed::LayoutByteSize == 12 && NamedFixed::LayoutAlignment == 4);
    static_assert(NamedFixed::extent == 3);
    static_assert(CanAccessSet<NamedFixed, float>);
    named_fixed.AccessSet(1, 113);
    named_view.ReadOptional().AccessSet(2, 127);
    named_view.ReadNested().AccessGet(1).AccessSet(2, 131);
    assert(named_fixed.AccessGet(1) == 113 && named_fixed.ReadSize() == 3);
    custom::ApiCollectionsViewReadOnly named_readonly(named.bytes());
    static_assert(!CanAccessSet<decltype(named_readonly.ReadFixed()), float>);
    assert(named_readonly.ReadOptional().AccessGet(2) == 127);
    assert(named_readonly.ReadNested().AccessGet(1).AccessGet(2) == 131);
    assert(named_readonly.ReadTree().AccessGet(0).ReadChildren().AccessGet(0).ReadValue() == 19);
    auto named_array = NamedFixed::BuildCreate(std::array<float, 3>{137, 139, 149});
    static_assert(std::is_same_v<decltype(named_array.view()), NamedFixed>);
    assert(named_array.view().AccessGet(2) == 149);
    assert(NamedFixed::BuildComputeByteLength(std::array<float, 3>{}) == 12);
    auto named_in_place = NamedFixed::BuildCreateInPlace(storage, std::array<float, 3>{151, 157, 163});
    assert(named_in_place.AccessGet(0) == 151 && named_in_place.ReadBytes().size() == 12);
    named_in_place.AccessReplace(std::array<float, 3>{167, 173, 179});
    assert(named_in_place.AccessGet(1) == 173);
    std::cout << "Shared C++ collection views passed\n";
}
