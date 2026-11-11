#include "bindings.hpp"
#include <cassert>
#include <fstream>
#include <iterator>

int main() {
    namespace wire = intrlay_generated::library_adapters;
    std::ifstream file("target/library-adapters/sample.bin", std::ios::binary);
    assert(file);
    std::vector<char> input{std::istreambuf_iterator<char>(file), std::istreambuf_iterator<char>()};
    std::vector<std::byte> bytes(input.size());
    std::memcpy(bytes.data(), input.data(), input.size());
    auto checked = wire::SampleView::from_bytes(bytes);
    assert(checked);
    assert(checked->bounds().min().y() == -2 && checked->bounds().max().z() == 6);
    assert(checked->points().get(1).x() == 10 && checked->matrix().y_axis().z() == 6);
    assert(checked->mask().x() && !checked->mask().y() && checked->mask().z());
    const std::string second = "two \xf0\x9f\xa6\x80";
    assert(checked->labels().get(1) == second);
    wire::SampleInput<> rebuilt{
        {{-1, -2, -3}, {4, 5, 6}}, {{7, 8, 9}, {10, 11, 12}},
        {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}}, {true, false, true}, {"one", second},
    };
    assert(std::ranges::equal(wire::SampleView::create(rebuilt).bytes(), bytes));
}
