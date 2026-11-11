#include "bindings.hpp"
#include "scalar.hpp"
#include <cassert>
#include <fstream>
#include <iostream>
#include <iterator>

template<class View, class Source>
concept CanReplace = requires(View view, const Source& source) { view.replace(source); };

template<class View, class Source>
concept CanSet = requires(View view, const Source& source) { view.set(source); };

static std::vector<std::byte> read(const std::string& name) {
    std::ifstream file("target/edge-cases/" + name + ".bin", std::ios::binary);
    assert(file);
    std::vector<char> input{std::istreambuf_iterator<char>(file), std::istreambuf_iterator<char>()};
    std::vector<std::byte> result(input.size());
    std::memcpy(result.data(), input.data(), input.size());
    return result;
}

int main() {
    namespace types = edge_cases;
    const std::string bom = "\xef\xbb\xbf";
    const types::TextInput<> input{{"", "hello", bom, bom + "hello", bom + bom + "hello", "a" + bom + "b", "\xed\x95\x9c\xea\xb8\x80\xf0\x9f\x98\x80"}};
    auto text = types::TextView::create(input);
    const auto expected = read("text");
    assert(std::ranges::equal(text.bytes(), expected));
    auto bytes = read("golden");
    auto root = types::EdgeCasesView::from_bytes(bytes).value();
    for (std::size_t i = 0; i < input.values.size(); ++i) {
        assert(text.view_unchecked().values().get(i) == input.values[i]);
        assert(root.text().values().get(i) == input.values[i]);
    }
    assert(root.nested().child().value().value().value().value() == 9);
    assert(root.recursive().child().value().value().value() == 8);
    assert(root.zeroes().after() == 42 && root.zeroes().units().size() == 0 && root.zeroes().elements().size() == 2);
    assert(root.shadow().custom().value() == 13 && root.shadow().optional() == 14 && root.shadow().items().get(0) == 15);
    assert(root.choice().value_child().value().value().value() == 16);
    root.projection().many().get(0).set_x(20.0f);
    assert(types::EdgeCasesViewReadOnly::from_bytes(bytes).value().projection().many().get(0).x() == 20);
    const auto absent0 = read("absent-0"), absent1 = read("absent-1"), absent2 = read("absent-2");
    assert(!types::NestedViewReadOnly::from_bytes(absent0).value().child());
    assert(!types::NestedViewReadOnly::from_bytes(absent1).value().child().value());
    assert(!types::NestedViewReadOnly::from_bytes(absent2).value().child().value().value());
    types::NestedInput<> defaulted;
    types::RecursiveInput<> recursive;
    assert(!defaulted.child && !recursive.child && recursive.value == 0);
    auto recursive_wire = types::RecursiveView::create(recursive);
    assert(!recursive_wire.view_unchecked().child());
    assert(edge_scalar::IntrlayU32View::create(std::uint32_t{123}).view_unchecked().value() == 123);
    using Scalar = edge_scalar::IntrlayU32View;
    static_assert(CanReplace<Scalar, std::uint32_t> && CanSet<Scalar, std::uint32_t>);
    static_assert(!CanReplace<Scalar, std::uint64_t> && !CanSet<Scalar, std::uint64_t>);
    static_assert(!CanReplace<Scalar, std::uint16_t> && !CanSet<Scalar, std::uint16_t>);
    static_assert(!CanReplace<Scalar, std::int32_t> && !CanSet<Scalar, std::int32_t>);
    static_assert(!CanReplace<edge_scalar::IntrlayU32ViewReadOnly, std::uint32_t>);
    auto scalar = Scalar::create(std::uint32_t{0});
    scalar.view_unchecked().replace(std::numeric_limits<std::uint32_t>::max());
    assert(scalar.view_unchecked().value() == std::numeric_limits<std::uint32_t>::max());
    std::cout << "C++ boundary values, defaults, and recursive declarations passed\n";
}
