#include "bindings.hpp"
#include <cassert>
#include <iostream>

int main() {
    using namespace composition;
    auto names = RenamedListView::create(RenamedList{{Renamed{11, 22}}});
    assert(names.view().items().get(0).first() == 11);
    names.view().items().get(0).set_second(23);
    assert(names.view().items().get(0).second() == 23);

    WrappedScalars input;
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
    view.set_read_only(100);
    assert(view.write_only() == 7 && view.read_only() == 100);
    view.set_present(11); view.set_absent(12); view.set_boxed(13);
    view.set_nested(14); view.set_missing_inner(15); view.set_write_only(20);
    view.items().set(0, 16); view.items().set(1, 17);
    view.fixed().set(0, std::optional<std::uint32_t>{18});
    view.fixed().set(1, std::optional<std::uint32_t>{19});
    assert(view.present() == 11 && view.absent() == 12 && view.boxed() == 13);
    assert(view.nested().value().value() == 14 && view.missing_inner().value() == 15);
    assert(view.items().get(0) == 16 && !view.items().get(1));
    assert(view.fixed().get(0) == 18 && view.fixed().get(1) == 19);
    std::optional<std::unique_ptr<std::optional<std::uint32_t>>> root_value{std::make_unique<std::optional<std::uint32_t>>(2)};
    auto root = OptionalRootView::create(root_value);
    root.view().set(9);
    assert(root.view().value().value().value() == 9);
    std::cout << "C++ composition regressions passed\n";
}
