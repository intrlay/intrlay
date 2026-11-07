#include "bindings.hpp"
#include <fstream>
#include <iostream>

using namespace intrlay_generated::conformance;

void check(bool condition, const char* label) {
    if (!condition) throw std::runtime_error(label);
}

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    const std::string directory = argv[1];
    std::ifstream nullable_file(directory + "/nullable.bin", std::ios::binary | std::ios::ate);
    const auto nullable_length = static_cast<std::size_t>(nullable_file.tellg());
    nullable_file.seekg(0);
    std::vector<std::byte> nullable_bytes(nullable_length);
    nullable_file.read(reinterpret_cast<char*>(nullable_bytes.data()), static_cast<std::streamsize>(nullable_length));
    NullableCasesView nullable(nullable_bytes);
    check(!nullable.absent() && nullable.zero() == 0u, "nullable scalar");
    check(nullable.text() == "" && !nullable.missing_text(), "nullable string");
    check(!nullable.absent_unit() && nullable.present_unit().has_value(), "nullable unit");
    check(!nullable.nested_absent() && nullable.nested_none() && !nullable.nested_none()->has_value(), "nested None");
    check(nullable.nested_value() && *nullable.nested_value() == 7u, "nested value");
    check(nullable.boxed_none() && !nullable.boxed_none()->has_value(), "boxed None");
    check(nullable.triple() && *nullable.triple() && !(**nullable.triple()).has_value(), "three option levels");
    check(!nullable.variants().get(0).value_value(), "inactive nullable enum field");
    check(nullable.variants().get(1).value_value() && !nullable.variants().get(1).value_value()->has_value(), "active nullable enum field");
    NullableCases nullable_value{
        std::nullopt, 0, std::string{}, std::nullopt, std::nullopt, std::monostate{},
        std::nullopt, std::optional<std::uint32_t>{}, std::optional<std::uint32_t>{7},
        std::make_unique<std::optional<std::uint32_t>>(),
        std::optional<std::optional<std::uint32_t>>{std::in_place, std::nullopt},
        {{NullableVariantEmpty{}}, {NullableVariantValue{std::nullopt}}, {NullableVariantValue{9}}}
    };
    check(std::ranges::equal(NullableCasesView::create(nullable_value).bytes(), nullable_bytes), "nullable exact Rust bytes");
    std::ifstream file(directory + "/rust.bin", std::ios::binary | std::ios::ate);
    const auto length = static_cast<std::size_t>(file.tellg());
    file.seekg(0);
    std::vector<std::byte> bytes(length);
    file.read(reinterpret_cast<char*>(bytes.data()), static_cast<std::streamsize>(length));
    MessageView view(bytes);
    check(view.sequence() == UINT64_C(0x123456789abcdef0), "u64");
    check(view.title() == "fixture 🦀 한글", "UTF-8");
    check(view.points().get(1).z() == 6, "point");
    check(*view.commands().get(1).draw_point_size() == 0.75, "variant");
    check(view.tree().next()->id() == 8 && !view.tree().next()->next().has_value(), "recursive box");
    check(view.optional().has_value() && *view.optional() == 0, "Some(0)");

    Message value{
        UINT64_C(0x123456789abcdef0), "fixture 🦀 한글",
        {{1, 2, 3}, {4, 5, 6}},
        {{DrawingCommandClear{}}, {DrawingCommandDrawPoint{{7, 8, 9}, 0.75, "example"}}},
        {7, std::make_unique<Link>(Link{8, std::nullopt})},
        0, {10, 20, 30}
    };
    auto created = MessageView::create(value);
    check(std::ranges::equal(created.bytes(), bytes), "exact Rust bytes");
    intrlay::OwnedView<MessageView> storage(length + 32, MessageView::alignment);
    std::ranges::fill(storage.bytes(), std::byte{0xcc});
    auto borrowed = MessageView::create_in_place(storage.bytes().subspan(16), value);
    borrowed.set_sequence(42);
    borrowed.points().get(1).set_z(99);
    check(borrowed.sequence() == 42 && borrowed.points().get(1).z() == 99, "mutation");
    check(storage.bytes()[15] == std::byte{0xcc} && storage.bytes().back() == std::byte{0xcc}, "external buffer bounds");
    std::ofstream output(directory + "/cpp.bin", std::ios::binary);
    output.write(reinterpret_cast<const char*>(created.bytes().data()), static_cast<std::streamsize>(created.bytes().size()));
    std::cout << "C++20: exact bytes, variants, recursive boxes, external subview mutation passed\n";
}
