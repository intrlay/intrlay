#include "alpha.hpp"
#include "beta.hpp"
#include "bindings.hpp"
#include "arrays.hpp"
#include "contracts.hpp"
#include "cpp-annotations.hpp"
#include "cpp-codecs.hpp"
#include "shared-alpha.hpp"
#include "shared-beta.hpp"
#include "custom-alpha.hpp"
#include "custom-beta.hpp"
#include "../../crates/intrlay-frontend-cpp/tests/fixtures/annotations.hpp"
#include "../../crates/intrlay-frontend-cpp/tests/fixtures/codec_cases.hpp"
#include <cassert>

template<class V> concept CanSetValue = requires (V value) { value.set_value(1); };
template<class V> concept CanReadValue = requires (V value) { value.value(); };

int main() {
    using namespace intrlay_generated::alpha;
    using namespace intrlay_generated::beta;
    using namespace intrlay_generated::regression;
    using namespace intrlay_generated::arrays;
    using namespace intrlay_generated::contracts;
    using namespace intrlay_generated::annotations;
    namespace left = intrlay_generated::shared_alpha;
    namespace right = intrlay_generated::shared_beta;
    static_assert(!std::is_same_v<left::Shared, right::Shared>);
    auto shared_alpha = left::SharedAlphaView::create(left::SharedAlpha{{11}});
    auto shared_beta = right::SharedBetaView::create(right::SharedBeta{{22}});
    assert(shared_alpha.view().shared().value() == 11);
    assert(shared_beta.view().shared().value() == 22);
    auto custom_alpha = fixture::left::SharedView::create(fixture::left::Shared{33});
    auto custom_beta = fixture::right::SharedView::create(fixture::right::Shared{44});
    assert(custom_alpha.view().value() == 33);
    assert(custom_beta.view().value() == 44);
    auto renamed = RenamedFieldView::create(annotation_fixture::RenamedField{7, {9}});
    assert(renamed.view().exported_value() == 7);
    assert(renamed.view().identifier().value() == 9);
    auto alpha = AlphaView::create(Alpha{{1, 2}});
    auto beta = BetaView::create(Beta{{3, 4}});
    assert(alpha.view().values().get(1) == 2);
    assert(beta.view().values().get(0) == 3);
    auto scalar = ScalarView::create(1.5f);
    assert(scalar.view().value() == 1.5f);
    static_assert(AlignedView::alignment == 16);
    const std::array<std::byte, 4> bytes{std::byte{7}};
    assert(ReadOnlyViewReadOnly(bytes).value() == 7);
    AlphaViewReadOnly read_only(alpha.bytes());
    assert(read_only.values().get(1) == 2);
    auto vectors = ProjectionsView::create(Projections{{Vector{1, 2, 3}}});
    assert(vectors.view().vectors().size() == 1);
    assert(vectors.view().vectors().get(0).z() == 3);
    vectors.view().vectors().get(0).set_y(9);
    assert(vectors.view().vectors().get(0).y() == 9);
    Contracts contract_input{
        ContractChild{1}, ContractChild{2}, ContractChild{3}, {4,5,6},
        std::make_unique<std::vector<ContractChild>>(std::initializer_list<ContractChild>{{7}}),
        ConstantChild{8}, Identifier{9}, {Identifier{10}, Identifier{11}}, MaybeValue{std::nullopt}, Label{"hello"}
    };
    auto contract = ContractsView::create(contract_input);
    auto cv = contract.view();
    static_assert(IdentifierView::byte_size == 16);
    assert(cv.identifier().field_0() == 9 && cv.identifiers().get(1).field_0() == 11);
    cv.identifier().set_field_0(19);
    cv.editable().set_value(30);
    cv.write_only().set_value(31);
    cv.read_only().set_value(32);
    cv.fixed().set_value(33);
    cv.read_only_nested().value().get(0).set_value(34);
    assert(cv.write_only().value() == 31 && cv.read_only().value() == 32 && cv.fixed().value() == 33);
    static_assert(!CanSetValue<decltype(ContractsViewReadOnly(contract.bytes()).editable())>);
    static_assert(CanSetValue<decltype(cv.read_only())>);
    static_assert(CanSetValue<decltype(cv.fixed())>);
    static_assert(CanSetValue<decltype(cv.read_only_nested().value().get(0))>);
    static_assert(CanReadValue<decltype(cv.write_only())>);
    assert(cv.identifier().field_0() == 19 && cv.editable().value() == 30);
    FixedArrays input{{1, 2, 3}, {4, 5, 6, 7}, {{{8, 9, 10}, {11, 12, 13}}},
        std::array<float, 3>{14, 15, 16}, std::nullopt, {17, 18, 19}};
    auto arrays = FixedArraysView::create(input);
    auto three = arrays.view().three();
    static_assert(decltype(three)::extent == 3);
    static_assert(decltype(arrays.view().four())::extent == 4);
    static_assert(decltype(arrays.view().matrix())::extent == 2);
    static_assert(decltype(arrays.view().matrix().get(0))::extent == 3);
    assert(three.get(2) == 3 && arrays.view().matrix().get(1).get(2) == 13);
    assert(arrays.view().present().value().get(2) == 16 && !arrays.view().absent());
    three.set(1, 20);
    assert(arrays.view().three().get(1) == 20);

    // Native C arrays and a variant payload with its own member named value.
    namespace codecs = intrlay_generated::codecs;
    auto native = codecs::CodecCasesView::create(
        codec_fixture::CodecCases{{1, 2, 3}, codec_fixture::ValueRecord{7, "payload"}});
    auto generated = codecs::CodecCasesView::create(
        codecs::CodecCases{{1, 2, 3}, {codecs::ValueRecord{7, "payload"}}});
    assert(std::ranges::equal(native.bytes(), generated.bytes()));
    assert(native.view().raw().get(2) == 3);
    auto payload = native.view().choice().as_value_record().value();
    assert(payload.value() == 7 && payload.label() == "payload");

    // Duplicate payload types are selected by index, never by type.
    auto end = codecs::EndpointsView::create(codec_fixture::Endpoints{
        std::in_place_index<1>, codec_fixture::ValueRecord{9, "end"}});
    codecs::Endpoints generated_end;
    generated_end.value.emplace<1>(codecs::ValueRecord{9, "end"});
    auto end_copy = codecs::EndpointsView::create(generated_end);
    assert(std::ranges::equal(end.bytes(), end_copy.bytes()));
    assert(end.view().tag() == 1 && !end.view().as_start());
    assert(end.view().as_end()->value() == 9 && end.view().as_end()->label() == "end");

    // A custom traits string retains embedded NUL characters and matches std::string.
    const codec_fixture::Text text{{"a\0b", 3}};
    auto encoded_text = codecs::TextView::create(text);
    auto ordinary_text = codecs::TextView::create(codecs::Text{std::string("a\0b", 3)});
    assert(std::ranges::equal(encoded_text.bytes(), ordinary_text.bytes()));
    assert(encoded_text.view().value() == std::string_view("a\0b", 3));
    assert(codecs::TextView::create(codec_fixture::Text{}).view().value().empty());
}
