#include "bindings.hpp"
#include <cassert>
#include <iostream>

using namespace intrlay_generated::replacement;

int main() {
    Message initial{123, Fixed{std::nullopt, std::optional<Leaf>{}, Choice{ChoiceEmpty{}}}, {std::nullopt}, "preserved", 456};
    auto owned = MessageView::create(initial);
    MessageView view(owned.bytes());
    MessageView alias(owned.bytes());
    auto fixed = alias.fixed();
    assert(!fixed.maybe() && !*fixed.nested());
    assert(!fixed.choice().child_leaf());
    const Leaf leaf{7, 8};
    view.fixed().set_maybe(std::optional{leaf});
    assert(fixed.maybe()->number() == 7);
    view.fixed().set_nested(std::optional{std::optional{leaf}});
    assert(fixed.nested()->value().number() == 7);
    view.fixed().choice().replace(Choice{ChoiceChild{leaf}});
    assert(fixed.choice().child_leaf()->number() == 7);
    view.fixed().choice().replace(Choice{ChoiceNumbers{{1,2,3}}});
    assert(!fixed.choice().child_leaf() && fixed.choice().numbers_values()->get(2) == 3);
    view.fixed().set_maybe(std::nullopt);
    view.fixed().set_nested(std::optional{std::optional<Leaf>{}});
    assert(!fixed.maybe() && !*fixed.nested());
    view.fixed().set_nested(std::nullopt);
    assert(!fixed.nested());
    view.items().set(0, std::optional{leaf});
    assert(alias.items().get(0)->number() == 7);
    view.items().set(0, std::optional<Leaf>{});
    assert(!alias.items().get(0));
    auto scalarOwned = MaybeScalarView::create(std::optional<std::uint32_t>{});
    MaybeScalarView scalar(scalarOwned.bytes());
    scalar.set(42);
    assert(scalar.value() == 42);
    scalar.set(std::nullopt);
    assert(!scalar.value());
    auto emptyOwned = EmptyPayloadsView::create(EmptyPayloads{{}, {}, 1});
    EmptyPayloadsView emptyView(emptyOwned.bytes());
    EmptyPayloadsView emptyAlias(emptyOwned.bytes());
    const EmptyPayloads replacement{{}, {}, 2};
    emptyView.replace(replacement);
    assert(emptyAlias.number() == 2 && emptyOwned.bytes().size() == 4);
    assert(emptyOwned.bytes()[0] == std::byte{2});
    for (std::size_t i = 1; i < 4; ++i) assert(emptyOwned.bytes()[i] == std::byte{0});
    assert(emptyAlias.empty().size() == 0 && emptyAlias.nested().size() == 2);
    assert(emptyAlias.nested().get(1).size() == 0);
    view.fixed().choice().replace(Choice{ChoiceEmptyArrays{replacement}});
    assert(fixed.choice().empty_arrays_data()->number() == 2);
    assert(alias.before() == 123 && alias.after() == 456 && alias.text() == "preserved");
    std::cout << "C++20: in-place replacement and shared views passed\n";
}
