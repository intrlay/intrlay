#ifdef NDEBUG
#undef NDEBUG
#endif
#include "cpp-adapters.hpp"
#include "../../crates/intrlay-frontend-cpp/tests/fixtures/adapters.hpp"
#include <cassert>

namespace adapted = intrlay_generated::adapters;

// Invalid mappings fail the source concept before any encoder body instantiates.
struct MissingSource {};
struct VoidSource {};
struct CycleA {};
struct CycleB {};
struct Imprecise {};
template<> struct intrlay::Adapter<MissingSource> { using schema_type = float; };
template<> struct intrlay::Adapter<VoidSource> {
    using schema_type = float;
    static void source(const VoidSource&) {}
};
template<> struct intrlay::Adapter<CycleA> {
    using schema_type = float;
    static CycleB source(const CycleA&) { return {}; }
};
template<> struct intrlay::Adapter<CycleB> {
    using schema_type = float;
    static CycleA source(const CycleB&) { return {}; }
};
template<> struct intrlay::Adapter<Imprecise> {
    using schema_type = float;
    static double source(const Imprecise&) { return 1.0; }
};
static_assert(!intrlay::EncodeSourceFor<MissingSource, intrlay::Scalar<float>>);
static_assert(!intrlay::EncodeSourceFor<VoidSource, intrlay::Scalar<float>>);
static_assert(!intrlay::EncodeSourceFor<CycleA, intrlay::Scalar<float>>);
static_assert(!intrlay::EncodeSourceFor<Imprecise, intrlay::Scalar<float>>);
static_assert(!intrlay::EncodeSourceFor<std::vector<Imprecise>,
    intrlay::SequenceCodec<intrlay::Scalar<float>, 4, 4>>);
static_assert(intrlay::EncodeSourceFor<external::Wrapped, intrlay::Scalar<float>>);
static_assert(intrlay::detail::native_layout<adapted::PositionView::Metadata, external::Vector3>);
static_assert(!intrlay::detail::native_layout<adapted::ShiftedView::Metadata, external::Shifted>);
static_assert(intrlay::detail::native_layout<intrlay::Scalar<float>, float>);

int main() {
    auto nested = adapted::NestedPayloadView::create(alias_schema::NestedPayload{11});
    assert(nested.view_unchecked().value() == 11);
    auto kind = adapted::NestedKindView::create(alias_schema::NestedKind::Last);
    assert(static_cast<std::uint32_t>(kind.view_unchecked().tag()) == 3);

    // Block-local namesakes keep their own value types even when USRs collide.
    auto integer = adapted::LocalIntegerView::create(external::LocalInteger{42});
    auto floating = adapted::LocalFloatView::create(external::LocalFloat{1.5f});
    assert(integer.view_unchecked().value() == 42);
    assert(floating.view_unchecked().value() == 1.5f);
    auto local_float = local_schema::floating();
    local_float.value = 2.5f;
    adapter_fixture::LocalFloatChoices local_choice{std::in_place_index<1>, local_float};
    auto choice = adapted::LocalFloatChoicesView::create(local_choice);
    assert(!choice.view_unchecked().as_first_float());
    assert(choice.view_unchecked().as_second_float()->value() == 2.5f);

    auto one = adapted::SamplesOneView::create(structural_schema::SamplesOne{{1.5f}});
    assert(one.view_unchecked().values().get(0) == 1.5f);
    const structural_schema::ConstSamplesOne constant{{2.5f}};
    assert(adapted::ConstSamplesOneView::create(constant).view_unchecked().values().get(0) == 2.5f);
    auto three = adapted::SamplesThreeView::create(structural_schema::Samples<structural_schema::Extent{3}>{{1, 2, 3}});
    assert(three.view_unchecked().values().get(2) == 3);
    structural_schema::SampleChoices samples{structural_schema::SamplesTwo{{4, 5}}};
    auto pair = adapted::SampleChoicesView::create(samples);
    assert(!pair.view_unchecked().as_single());
    assert(pair.view_unchecked().as_pair()->values().get(1) == 5);

    adapter_fixture::Message input{
        external::Pose{{1, 2, 3}, external::Label{std::string("a\0b", 3)}},
        {{4, 5, 6}, {7, 8, 9}},
        {external::Vector3{10, 11, 12}, std::nullopt},
        std::make_unique<external::Vector3>(external::Vector3{13, 14, 15}),
        external::Shifted{16},
        {{17}, {18}},
        {19},
        {20, {{21, {{22, {}}}}}},
        {},
    };
    auto encoded = adapted::MessageView::create(input);
    auto view = encoded.view_unchecked();
    assert(view.pose().position().z() == 3);
    assert(view.pose().label() == std::string_view("a\0b", 3));
    assert(view.positions().size() == 2 && view.positions().get(1).x() == 7);
    assert(view.optional_positions().get(0)->z() == 12 && !view.optional_positions().get(1));
    assert(view.boxed_position().z() == 15);
    assert(view.choice().as_shifted()->value() == 17);
    assert(view.shifted().get(0).value() == 18 && view.shifted().get(1).value() == 19);
    assert(view.wrapped() == 19);
    assert(view.tree().children().get(0).children().get(0).value() == 22);

    // Adapted objects encode identically to ordinary generated inputs. The
    // original records, borrowed proxies, and recursive containers need no copies.
    adapted::MessageInput<> expected{
        {{1, 2, 3}, std::string("a\0b", 3)},
        {{4, 5, 6}, {7, 8, 9}},
        {adapted::PositionInput<>{10, 11, 12}, std::nullopt},
        std::make_unique<adapted::PositionInput<>>(adapted::PositionInput<>{13, 14, 15}),
        {adapted::ShiftedInput<>{17}},
        {{18}, {19}},
        19,
        {20, {{21, {{22, {}}}}}},
        {},
    };
    auto ordinary = adapted::MessageView::create(expected);
    assert(std::ranges::equal(encoded.bytes(), ordinary.bytes()));
    adapted::MessageView::create_in_place(ordinary.bytes(), input);
    assert(std::ranges::equal(encoded.bytes(), ordinary.bytes()));

    // Every variant shape uses its adapter exactly once, including strings and
    // an adapter that returns a transformed value of its own native type.
    input.choice = external::Label{"choice"};
    assert(adapted::MessageView::create(input).view_unchecked().choice().as_string() == "choice");
    input.choice = external::Vector3{23, 24, 25};
    assert(adapted::MessageView::create(input).view_unchecked().choice().as_position()->z() == 25);
    input.choice = external::Shifted{26};
    assert(adapted::MessageView::create(input).view_unchecked().choice().as_shifted()->value() == 27);

    // Scalar setters and fixed replacements accept the same adapted sources.
    view.set_wrapped(external::Wrapped{30});
    view.positions().set(0, external::Vector3{31, 32, 33});
    view.optional_positions().set(1, std::optional{external::Vector3{34, 35, 36}});
    view.shifted().set(1, external::Shifted{40});
    assert(view.wrapped() == 30 && view.positions().get(0).y() == 32);
    assert(view.optional_positions().get(1)->z() == 36);
    assert(view.shifted().get(1).value() == 41);
    assert(adapted::ScalarView::create(external::Wrapped{42}).view_unchecked().value() == 42);

    // Root annotations address the adapter's resolved variant representation.
    external::Endpoints endpoints{std::variant<external::Vector3, external::Vector3>{
        std::in_place_index<1>, external::Vector3{43, 44, 45}}};
    auto encoded_endpoint = adapted::EndpointsView::create(endpoints);
    assert(!encoded_endpoint.view_unchecked().as_start());
    assert(encoded_endpoint.view_unchecked().as_end()->z() == 45);
}
