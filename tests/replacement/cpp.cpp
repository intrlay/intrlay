#ifdef NDEBUG
#undef NDEBUG
#endif
#include "bindings.hpp"
#include <cassert>
#include <deque>
#include <iostream>

using namespace intrlay_generated::replacement;

template<class View, class Source>
concept CanReplace = requires(View view, const Source& source) { view.replace(source); };

template<class View, class Source>
concept CanSet = requires(View view, const Source& source) { view.set(source); };

template<class View, class Source>
concept CanSetBraced = requires(View view, const Source& source) { view.set({source}); };

template<class View, class Source>
concept CanReplaceBraced = requires(View view, const Source& source) { view.replace({source}); };

template<class View, class Source>
concept CanSetScalarBraced = requires(View view, const Source& source) { view.set_scalar({source}); };

template<class Source>
concept CanSetNumber = requires(LeafView view, const Source& source) { view.set_number(source); };

template<class Source>
concept CanSetValues = requires(ChoiceView view, const Source& source) { view.set_numbers_values(source); };

struct ConvertibleNumber {
    operator std::uint32_t() const { return 7; }
};

struct NativeLeaf {
    std::uint32_t number;
    std::optional<std::uint32_t> scalar;
};

struct ThrowingNumbers {
    std::deque<std::uint32_t> values{31, 37, 41};
    auto begin() const { return values.begin(); }
    auto end() const { return values.end(); }
    auto size() const { return values.size(); }
    const std::uint32_t& operator[](std::size_t index) const {
        if (index == 1) throw std::runtime_error("source read failed");
        return values[index];
    }
};

struct ThrowingOptional {
    bool has_value() const noexcept { return true; }
    std::uint32_t operator*() const { throw std::runtime_error("source read failed"); }
};

template<class Number, class Scalar>
struct LeafSource { Number number; Scalar scalar; };
template<class... Fields>
concept CanFormLeaf = requires { typename LeafInput<Fields...>; };
static_assert(!CanFormLeaf<std::uint64_t, std::optional<std::uint32_t>>);

// These constraints must fail during overload resolution, not inside a writer.
static_assert(CanSetNumber<std::uint32_t>);
static_assert(!CanSetNumber<std::uint64_t> && !CanSetNumber<ConvertibleNumber>);
static_assert(CanSet<MaybeScalarView, std::uint32_t>);
static_assert(CanSet<MaybeScalarView, std::optional<std::uint32_t>>);
static_assert(CanSet<MaybeScalarView, std::nullopt_t>);
static_assert(!CanSet<MaybeScalarView, std::uint64_t>);
static_assert(!CanSet<MaybeScalarView, std::optional<std::uint16_t>>);
static_assert(!CanSet<MaybeScalarView, ConvertibleNumber>);
// Untyped braces must not select a native input that narrows before checking.
static_assert(!CanSetBraced<MaybeScalarView, std::uint64_t>);
static_assert(!CanReplaceBraced<MaybeScalarView, std::uint64_t>);
static_assert(!CanSetScalarBraced<LeafView, std::uint64_t>);
static_assert(!CanSet<IntrlayOptionalU32ViewReadOnly, std::uint32_t>);
static_assert(CanReplace<LeafView, NativeLeaf>);
static_assert(!CanReplace<LeafView, LeafSource<std::uint64_t, std::optional<std::uint32_t>>>);
static_assert(!CanReplace<LeafView, LeafSource<std::uint32_t, std::optional<std::uint16_t>>>);
static_assert(CanSetValues<std::span<const std::uint32_t>>);
static_assert(!CanSetValues<std::span<const std::uint64_t>>);
static_assert(!CanSetValues<std::array<std::uint16_t, 3>>);

#ifdef INTRLAY_TEST_INVALID_MUTATIONS
void invalid_sources(LeafView leaf, MaybeScalarView optional, ChoiceView choice,
                     std::uint64_t scalar, std::span<const std::uint64_t> values) {
    leaf.set_number(scalar);
    optional.set(std::optional{scalar});
    choice.set_numbers_values(values);
    leaf.replace(LeafSource{scalar, std::optional<std::uint32_t>{}});
}
#endif

int main() {
    MessageInput<> initial{123, FixedInput<>{std::nullopt, std::optional<LeafInput<>>{}, ChoiceInput<>{ChoiceEmptyInput<>{}}}, {std::nullopt}, "preserved", 456};
    auto owned = MessageView::create(initial);
    std::vector<std::byte> external(owned.bytes().size() + 16, std::byte{0xa5});
    auto view = MessageView::create_in_place(std::span(external).subspan(8), initial);
    auto alias = MessageView::from_bytes(view.bytes()).value();
    auto fixed = alias.fixed();
    assert(!fixed.maybe() && !*fixed.nested());
    assert(!fixed.choice().child_leaf());
    const LeafInput<> leaf{7, 8};
    view.fixed().set_maybe(std::optional{leaf});
    assert(fixed.maybe()->number() == 7);
    view.fixed().set_nested(std::optional{std::optional{leaf}});
    assert(fixed.nested()->value().number() == 7);
    view.fixed().choice().replace(ChoiceInput<>{ChoiceChildInput<>{leaf}});
    assert(fixed.choice().child_leaf()->number() == 7);
    view.fixed().choice().replace(ChoiceInput<>{ChoiceNumbersInput<>{{1,2,3}}});
    assert(!fixed.choice().child_leaf() && fixed.choice().numbers_values()->get(2) == 3);
    const std::array<std::uint32_t, 3> numbers{11, 13, 17};
    view.fixed().choice().set_numbers_values(std::span(numbers));
    assert(fixed.choice().numbers_values()->get(2) == 17);
    const std::vector<std::byte> before(view.bytes().begin(), view.bytes().end());
    // An inactive variant cannot overwrite the active variant's payload.
    view.fixed().choice().set_child_leaf(NativeLeaf{19, 23});
    assert(std::ranges::equal(view.bytes(), before));
    bool rejected = false;
    try {
        view.fixed().choice().set_numbers_values(std::span(numbers).first(2));
    } catch (const std::length_error&) {
        rejected = true;
    }
    assert(rejected && std::ranges::equal(view.bytes(), before));
    // A source can throw after writing the first element. Existing aliases must
    // still see the complete previous value, not a partially replaced array.
    try {
        view.fixed().choice().set_numbers_values(ThrowingNumbers{});
        assert(false);
    } catch (const std::runtime_error&) {}
    assert(std::ranges::equal(view.bytes(), before));

    view.fixed().choice().replace(ChoiceInput<>{ChoiceChildInput<>{leaf}});
    const std::vector<std::byte> before_variant(view.bytes().begin(), view.bytes().end());
    using BorrowedChoice = std::variant<ChoiceEmptyInput<>, ChoiceChildInput<>,
        ChoiceNumbersInput<std::span<const std::uint32_t>>, ChoiceEmptyArraysInput<>>;
    try {
        view.fixed().choice().replace(BorrowedChoice{ChoiceNumbersInput{std::span(numbers).first(2)}});
        assert(false);
    } catch (const std::length_error&) {}
    assert(std::ranges::equal(view.bytes(), before_variant));
    assert(MessageView::from_bytes(view.bytes()) && fixed.choice().child_leaf()->number() == 7);

    // Record replacement must not commit earlier fields before a later source fails.
    try {
        fixed.choice().child_leaf()->replace(LeafSource{79u, ThrowingOptional{}});
        assert(false);
    } catch (const std::runtime_error&) {}
    assert(std::ranges::equal(view.bytes(), before_variant));
    view.fixed().set_maybe(std::nullopt);
    view.fixed().set_nested(std::optional{std::optional<LeafInput<>>{}});
    assert(!fixed.maybe() && !*fixed.nested());
    view.fixed().set_nested(std::nullopt);
    assert(!fixed.nested());
    view.items().set(0, std::optional{leaf});
    assert(alias.items().get(0)->number() == 7);
    view.items().set(0, std::optional<LeafInput<>>{});
    assert(!alias.items().get(0));
    auto scalarOwned = MaybeScalarView::create(std::optional<std::uint32_t>{});
    auto scalar = MaybeScalarView::from_bytes(scalarOwned.bytes()).value();
    scalar.set(42u);
    assert(scalar.value() == 42u);
    scalar.set(std::nullopt);
    assert(!scalar.value());
    scalar.set(std::optional{43u});
    assert(scalar.value() == 43u);
    scalar.replace(std::nullopt);
    assert(!scalar.value());
    assert(!MaybeScalarView::create(std::nullopt).view_unchecked().value());

    auto native = LeafView::create(leaf);
    // Creation initializes padding; replacing an existing value preserves it.
    const auto padding = LeafView::Metadata::Field<1>::offset + 1;
    assert(native.bytes()[padding] == std::byte{});
    native.bytes()[padding] = std::byte{0xa5};
    native.view_unchecked().replace(NativeLeaf{47, 53});
    assert(native.view_unchecked().number() == 47 && native.view_unchecked().scalar() == 53u);
    assert(native.bytes()[padding] == std::byte{0xa5});
    native.view_unchecked().replace(LeafInput{59u, std::nullopt});
    assert(native.view_unchecked().number() == 59 && !native.view_unchecked().scalar());

    auto boxed = BoxedLeafView::create(std::make_unique<std::optional<LeafInput<>>>(leaf));
    const auto boxed_size = boxed.bytes().size();
    boxed.view_unchecked().set(std::nullopt);
    assert(!boxed.view_unchecked().value());
    boxed.view_unchecked().set(NativeLeaf{61, 67});
    assert(boxed.view_unchecked().value()->number() == 61 && boxed.view_unchecked().value()->scalar() == 67u);
    assert(boxed.bytes().size() == boxed_size);
    auto emptyOwned = EmptyPayloadsView::create(EmptyPayloadsInput<>{{}, {}, 1});
    auto emptyView = EmptyPayloadsView::from_bytes(emptyOwned.bytes()).value();
    auto emptyAlias = EmptyPayloadsView::from_bytes(emptyOwned.bytes()).value();
    const EmptyPayloadsInput<> replacement{{}, {}, 2};
    emptyView.replace(replacement);
    assert(emptyAlias.number() == 2 && emptyOwned.bytes().size() == 4);
    assert(emptyOwned.bytes()[0] == std::byte{2});
    for (std::size_t i = 1; i < 4; ++i) assert(emptyOwned.bytes()[i] == std::byte{0});
    assert(emptyAlias.empty().size() == 0 && emptyAlias.nested().size() == 2);
    assert(emptyAlias.nested().get(1).size() == 0);
    view.fixed().choice().replace(ChoiceInput<>{ChoiceEmptyArraysInput<>{replacement}});
    assert(fixed.choice().empty_arrays_data()->number() == 2);
    assert(alias.before() == 123 && alias.after() == 456 && alias.text() == "preserved");
    assert(std::ranges::all_of(std::span(external).first(8), [](auto byte) { return byte == std::byte{0xa5}; }));
    assert(std::ranges::all_of(std::span(external).last(8), [](auto byte) { return byte == std::byte{0xa5}; }));
    std::cout << "C++20: in-place replacement and shared views passed\n";
}
