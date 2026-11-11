#ifdef NDEBUG
#undef NDEBUG
#endif
#include "bindings.hpp"
#include "custom.hpp"
#include "boolean-inputs.hpp"
#include <cassert>
#include <filesystem>
#include <fstream>
#include <iostream>

namespace generated = cpp_collections;

template<class View, class Value>
concept CanSet = requires(View view, Value value) { view.set(0, value); };

template<class View, class T>
concept CanCopy = requires(View view, std::span<T> destination) { view.copy_to(destination); };

template<class... Fields>
concept CanFormBooleans = requires { typename generated::BooleanInputsInput<Fields...>; };

template<class View, class Value>
concept CanCreate = requires(Value value) { View::create(value); };

template<class View, class Value>
concept CanReplace = requires(View view, const Value& value) { view.replace(value); };

template<class Byte>
concept CanBorrow = requires { typename generated::BasicCollectionsView<Byte>; };

template<class View, class Value>
concept CanAccessSet = requires(View view, Value value) { view.AccessSet(0, value); };

template<class View, class Value>
concept CanAccessReplace = requires(View view, const Value& value) { view.AccessReplace(value); };

// Range-for would call the non-callable members. The C++20 range customization
// points correctly choose the friend functions for this valid contiguous range.
template<class T>
struct AdlSourceRange {
    enum { begin = 0, end = 0 };
    std::span<const T> values;
    std::size_t size() const { return values.size(); }
    const T& operator[](std::size_t index) const { return values[index]; }
    friend auto begin(const AdlSourceRange& range) { return range.values.begin(); }
    friend auto end(const AdlSourceRange& range) { return range.values.end(); }
};

struct ExactIndexRange : std::span<const float> {
    using std::span<const float>::span;
    using std::span<const float>::operator[];
    const float& operator[](int) const = delete;
};

struct WrongIndexRange : std::span<const float> {
    using std::span<const float>::span;
    std::string_view operator[](std::size_t) const { return {}; }
    const float& operator[](int index) const { return std::span<const float>::operator[](static_cast<std::size_t>(index)); }
};

template<class T>
struct LvalueIndexRange : std::span<const T> {
    using std::span<const T>::span;
    const T& operator[](std::size_t& index) const { return std::span<const T>::operator[](index); }
    const T& operator[](std::size_t&&) const = delete;
};

template<class T>
struct RvalueIndexRange : std::span<const T> {
    using std::span<const T>::span;
    const T& operator[](std::size_t&) const = delete;
    const T& operator[](std::size_t&& index) const { return std::span<const T>::operator[](index); }
};

struct ElementCount {
    std::size_t value;
    operator std::size_t() const { return value; }
    operator std::ptrdiff_t() const { return static_cast<std::ptrdiff_t>(value); }
};

template<class T>
struct CountedSourceRange : std::span<const T> {
    using std::span<const T>::span;
    ElementCount size() const { return {std::span<const T>::size()}; }
};

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

struct BooleanSource {
    bool value;
    explicit operator bool() const { return value; }
};

struct StorageByteSource {
    explicit operator std::uint8_t() const { return 2; }
};

void check_boolean_inputs(const std::filesystem::path& directory) {
    const auto reference = read_bytes(directory / "boolean-inputs.bin");
    const std::array<std::uint32_t, 3> fixed{2, 256, 0};
    const std::array<double, 3> dynamic{0.5, -0.0, 2.5};
    const int boxed = -256;
    static_assert(!CanFormBooleans<int, decltype(fixed), decltype(std::span(dynamic)), std::optional<int>, const int*>);

    // Conversion is explicit at the source; range elements are never collected.
    const bool converted_box = static_cast<bool>(boxed);
    const auto input = generated::BooleanInputsInput{
        static_cast<bool>(256),
        fixed | std::views::transform([](std::uint32_t value) { return static_cast<bool>(value); }),
        dynamic | std::views::transform([](double value) { return static_cast<bool>(value); }),
        std::optional<bool>{static_cast<bool>(256)},
        &converted_box
    };
    static_assert(intrlay::EncodeSourceFor<decltype(input), generated::BooleanInputsView::Metadata>);
    auto encoded = generated::BooleanInputsView::create(input);
    assert(std::ranges::equal(encoded.bytes(), reference));
    auto view = encoded.view_unchecked();
    assert(view.scalar() && view.fixed().get(0) && view.fixed().get(1) && !view.fixed().get(2));
    assert(view.dynamic().get(0) && !view.dynamic().get(1) && view.dynamic().get(2));
    assert(view.maybe() == true && view.boxed());

    // Whole-array replacement keeps the same explicit element conversion.
    view.set_scalar(static_cast<bool>(256));
    static_assert(!CanReplace<decltype(view.fixed()), decltype(fixed)>);
    view.fixed().replace(input.fixed);
    view.dynamic().set(0, static_cast<bool>(256));
    view.set_maybe(std::optional<bool>{true});
    assert(std::ranges::equal(encoded.bytes(), reference));

    generated::BooleanInputsInput<> native;
    native.scalar = true;
    native.fixed = {true, true, false};
    native.dynamic = {true, false, true};
    native.maybe = true;
    native.boxed = std::make_unique<bool>(true);
    assert(std::ranges::equal(generated::BooleanInputsView::create(native).bytes(), reference));

    // Neither numeric nor user-defined conversions are implicit encoding sources.
    using Boolean = intrlay::Scalar<bool, std::uint8_t>;
    static_assert(intrlay::EncodeSourceFor<bool, Boolean>);
    static_assert(intrlay::EncodeSourceFor<const bool&, Boolean>);
    static_assert(intrlay::EncodeSourceFor<volatile bool&, Boolean>);
    static_assert(!intrlay::EncodeSourceFor<int, Boolean>);
    static_assert(!intrlay::EncodeSourceFor<double, Boolean>);
    static_assert(!intrlay::EncodeSourceFor<BooleanSource, Boolean>);
    static_assert(!intrlay::EncodeSourceFor<StorageByteSource, Boolean>);
    static_assert(!intrlay::EncodeSourceFor<StorageByteSource, intrlay::Scalar<std::uint8_t>>);
    static_assert(intrlay::SetSourceFor<bool, Boolean>);
    static_assert(!intrlay::SetSourceFor<int, Boolean>);
    static_assert(!intrlay::SetSourceFor<double, Boolean>);
    static_assert(!intrlay::SetSourceFor<BooleanSource, Boolean>);
    static_assert(!CanSet<decltype(view.fixed()), int>);
    static_assert(!CanSet<decltype(view.dynamic()), double>);
    static_assert(!intrlay::SetSourceFor<int, generated::BooleanInputsView::Metadata::Field<0>>);
    static_assert(!intrlay::SetSourceFor<std::optional<int>, generated::BooleanInputsView::Metadata::Field<3>>);
    static_assert(!intrlay::SetSourceFor<int, generated::BooleanInputsView::Metadata::Field<4>>);
    static_assert(!CanFormBooleans<
        int, std::array<bool, 3>, std::vector<bool>, std::optional<bool>, const bool*>);
    static_assert(!CanFormBooleans<
        bool, std::array<int, 3>, std::vector<bool>, std::optional<bool>, const bool*>);
    static_assert(!CanFormBooleans<
        bool, std::array<bool, 3>, std::vector<double>, std::optional<bool>, const bool*>);
    static_assert(!CanFormBooleans<
        bool, std::array<bool, 3>, std::vector<bool>, std::optional<int>, const bool*>);
    static_assert(!CanFormBooleans<
        bool, std::array<bool, 3>, std::vector<bool>, std::optional<bool>, const int*>);

    std::vector<std::byte> storage(reference.size() + 16, std::byte{0xa5});
    auto reused = generated::BooleanInputsView::create_in_place(std::span(storage).subspan(8), input);
    assert(std::ranges::equal(reused.bytes(), reference));
    assert(std::ranges::all_of(std::span(storage).first(8), [](std::byte value) { return value == std::byte{0xa5}; }));
    assert(std::ranges::all_of(std::span(storage).last(8), [](std::byte value) { return value == std::byte{0xa5}; }));
}

void check_numeric_inputs() {
    using Integer = intrlay::Scalar<std::uint32_t>;
    using Float = intrlay::Scalar<float>;
    static_assert(intrlay::EncodeSourceFor<const std::uint32_t&, Integer>);
    static_assert(!intrlay::EncodeSourceFor<std::uint16_t, Integer>);
    static_assert(!intrlay::EncodeSourceFor<std::uint64_t, Integer>);
    static_assert(!intrlay::EncodeSourceFor<std::int32_t, Integer>);
    static_assert(!intrlay::EncodeSourceFor<bool, Integer>);
    static_assert(!intrlay::EncodeSourceFor<double, Float>);
    static_assert(!intrlay::EncodeSourceFor<int, Float>);
    static_assert(intrlay::SetSourceFor<const std::uint32_t&, Integer>);
    static_assert(intrlay::SetSourceFor<volatile std::uint32_t&, Integer>);
    static_assert(!intrlay::SetSourceFor<std::uint16_t, Integer>);
    static_assert(!intrlay::SetSourceFor<std::uint64_t, Integer>);
    static_assert(!intrlay::SetSourceFor<std::int32_t, Integer>);
    static_assert(!intrlay::SetSourceFor<bool, Integer>);
    static_assert(!intrlay::SetSourceFor<double, Float>);
    static_assert(!intrlay::SetSourceFor<int, Float>);

    // Original C++ schema fields must remain valid after fixed-width normalization.
    using NativeLong = std::conditional_t<sizeof(long) == 4, std::int32_t, std::int64_t>;
    using NativeChar = std::conditional_t<std::is_signed_v<char>, std::int8_t, std::uint8_t>;
    static_assert(intrlay::EncodeSourceFor<long, intrlay::Scalar<NativeLong>>);
    static_assert(intrlay::EncodeSourceFor<long long, intrlay::Scalar<std::int64_t>>);
    static_assert(intrlay::EncodeSourceFor<char, intrlay::Scalar<NativeChar>>);
    static_assert(intrlay::SetSourceFor<long, intrlay::Scalar<NativeLong>>);
    static_assert(intrlay::SetSourceFor<char, intrlay::Scalar<NativeChar>>);
    static_assert(!intrlay::EncodeSourceFor<wchar_t, Integer>);
    static_assert(!intrlay::EncodeSourceFor<long double, intrlay::Scalar<double>>);
    using LongSequence = intrlay::SequenceCodec<intrlay::Scalar<NativeLong>, sizeof(NativeLong), alignof(NativeLong)>;
    static_assert(intrlay::EncodeSourceFor<std::span<const long>, LongSequence>);

    using Triplet = generated::TripletView;
    static_assert(!CanCreate<Triplet, std::array<double, 3>>);
    static_assert(!CanReplace<Triplet, std::array<double, 3>>);
    static_assert(!CanCreate<Triplet, std::span<const double>>);
    static_assert(CanSet<Triplet, float>);
    static_assert(!CanSet<Triplet, double>);
    static_assert(!CanSet<Triplet, int>);

    const std::array<double, 3> original{1.25, 2.5, 3.75};
    const auto converted = original | std::views::transform([](double value) {
        return static_cast<float>(value);
    });
    const std::array<float, 3> expected{1.25f, 2.5f, 3.75f};
    const auto owned = Triplet::create(expected);
    auto borrowed = Triplet::create(converted);
    assert(std::ranges::equal(owned.bytes(), borrowed.bytes()));
    borrowed.view_unchecked().replace(converted);
    assert(std::ranges::equal(owned.bytes(), borrowed.bytes()));

    // Composite items stay generic, provided every scalar leaf matches its schema.
    const generated::BranchInput<> child{19, {}};
    const auto input = generated::BranchInput{std::uint32_t{17}, std::span(&child, 1)};
    auto tree = generated::BranchView::create(input);
    assert(tree.view_unchecked().value() == 17 && tree.view_unchecked().children().get(0).value() == 19);
    struct WrongChild { std::uint16_t value; std::span<const generated::BranchInput<>> children; };
    struct WrongTree { std::uint32_t value; std::span<const WrongChild> children; };
    const WrongChild wrong_child{19, std::span(&child, 0)};
    const WrongTree wrong_tree{17, std::span(&wrong_child, 1)};
    static_assert(!CanCreate<generated::BranchView, decltype(wrong_tree)>);

    using Nested = decltype(std::declval<generated::CollectionsView>().nested());
    static_assert(CanCreate<Nested, std::vector<std::span<const float>>>);
    static_assert(!CanCreate<Nested, std::vector<std::span<const double>>>);
    const std::array<std::span<const float>, 2> nested{expected, expected};
    auto sequence = Nested::create(nested);
    assert(sequence.view_unchecked().size() == 2 && sequence.view_unchecked().get(1).get(2) == 3.75f);
}

int main(int argc, char** argv) {
    const std::filesystem::path directory = argc > 1 ? argv[1] : "target/cpp-collections";
    check_boolean_inputs(directory);
    check_numeric_inputs();
    generated::CollectionsInput<> input;
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
    intrlay::OwnedView<generated::CollectionsView> reused(reference.size(), generated::CollectionsView::alignment);
    std::ranges::fill(reused.bytes(), std::byte{0xa5});
    generated::CollectionsView::create_in_place(reused.bytes(), input);
    assert(std::ranges::equal(reused.bytes(), reference));
    auto view = owned.view_unchecked();
    using Fixed = decltype(view.fixed());
    static_assert(std::is_same_v<Fixed, intrlay::ArrayView<typename Fixed::Metadata>>);
    static_assert(Fixed::extent == 3 && Fixed::byte_size == 12);
    static_assert(CanSet<Fixed, float>);
    static_assert(CanCreate<Fixed, std::array<float, 3>>);
    static_assert(!CanCreate<Fixed, int>);
    static_assert(CanReplace<Fixed, std::span<const float>>);
    static_assert(!CanReplace<Fixed, int>);
    static_assert(!CanReplace<Fixed, std::array<std::string, 3>>);
    static_assert(!CanReplace<decltype(generated::CollectionsViewReadOnly::from_bytes(reference).value().fixed()), std::array<float, 3>>);
    static_assert(CanBorrow<std::byte> && CanBorrow<const std::byte>);
    static_assert(!CanBorrow<char> && !CanBorrow<volatile std::byte>);
    static_assert(!CanCreate<decltype(view.nested()), int>);
    static_assert(!CanSet<decltype(generated::CollectionsViewReadOnly::from_bytes(reference).value().fixed()), float>);
    static_assert(!CanSet<decltype(view.tree()), generated::BranchInput<>>);
    static_assert(CanSet<decltype(view.nested()), std::array<float, 3>>);
    static_assert(!CanSet<decltype(view.nested()), std::array<double, 3>>);
    static_assert(!CanSet<decltype(view.nested()), int>);
    static_assert(!CanSet<decltype(view.padded()), std::string>);
    static_assert(!CanSet<decltype(view.optional()), std::string>);
    static_assert(!CanSet<decltype(view.optional()), std::uint64_t>);
    static_assert(!CanSet<decltype(view.optional()), std::nullopt_t>);
    static_assert(!CanCreate<generated::BorrowOnlyView, generated::BorrowOnlyInput<>>);
    static_assert(decltype(view.nested())::extent == std::dynamic_extent);
    assert(view.nested().get(1).get(2) == 9);
    assert(view.tree().get(0).children().get(0).value() == 19);
    assert(view.tree().get(0).children().get(0).children().size() == 0);
    assert(view.padded().get(0).value() == 23 && view.padded().get(1).value() == 29);
    assert(view.empty().size() == 0 && decltype(view.empty())::extent == 0);
    assert(view.units().size() == 3 && view.units().get(2) == std::monostate{});
    assert(view.flags().get(0) && !view.flags().get(1));

    view.fixed().set(0, 41.0f);
    view.nested().set(1, std::array<float, 3>{7, 8, 43});
    view.optional().set(0, 47u);
    view.optional().set(1, 53u); // An absent element stays absent.
    view.tree().get(0).children().get(0).set_value(59u);
    view.flags().set(1, true);
    auto readonly = generated::CollectionsViewReadOnly::from_bytes(owned.bytes()).value();
    static_assert(std::ranges::forward_range<decltype(readonly.fixed())>);
    static_assert(std::ranges::forward_range<decltype(readonly.tree())>);
    static_assert(!std::ranges::contiguous_range<decltype(readonly.fixed())>);
    assert(std::ranges::equal(readonly.fixed(), std::array<float, 3>{41, 2, 3}));
    auto iterator = readonly.fixed().begin();
    auto previous = iterator++;
    assert(*previous == 41 && *iterator == 2 && previous != iterator);
    assert(std::ranges::distance(readonly.units()) == 3);
    assert(std::ranges::distance(readonly.empty()) == 0);
    std::uint32_t tree_sum = 0;
    for (auto branch : readonly.tree()) {
        tree_sum += branch.value();
        for (auto child : branch.children()) tree_sum += child.value();
    }
    assert(tree_sum == 17 + 59);
    // Iterator reads preserve the complete storage for nested relative pointers.
    for (auto branch : view.tree()) {
        for (auto child : branch.children()) child.set_value(61u);
    }
    assert(readonly.tree().get(0).children().get(0).value() == 61);
    view.tree().get(0).children().get(0).set_value(59u);
    assert(readonly.fixed().get(0) == 41 && readonly.nested().get(1).get(2) == 43);
    static_assert(CanCopy<decltype(readonly.fixed()), float>);
    static_assert(!CanCopy<decltype(readonly.fixed()), double>);
    static_assert(!CanCopy<decltype(readonly.fixed()), const float>);
    static_assert(CanCopy<decltype(readonly.nested()), std::array<float, 3>>);
    static_assert(!CanCopy<decltype(readonly.padded()), generated::PaddedInput<>>);
    std::array<float, 3> fixed_copy;
    readonly.fixed().copy_to(std::span(fixed_copy));
    assert(fixed_copy[0] == 41);
    std::vector<std::array<float, 3>> nested_copy(readonly.nested().size());
    readonly.nested().copy_to(std::span(nested_copy));
    assert(nested_copy[1][2] == 43);
    try {
        readonly.fixed().copy_to(std::span(fixed_copy).first(2));
        assert(false);
    } catch (const std::length_error&) {}
    assert(fixed_copy[0] == 41);
    std::vector<std::byte> unaligned(1, std::byte{0xa5});
    unaligned.insert(unaligned.end(), owned.bytes().begin(), owned.bytes().end());
    auto unaligned_view = generated::CollectionsViewReadOnly::from_parts(unaligned, 1).value();
    assert(std::ranges::equal(unaligned_view.fixed(), readonly.fixed()));
    unaligned_view.fixed().copy_to(std::span(fixed_copy));
    assert(fixed_copy[0] == 41 && unaligned.front() == std::byte{0xa5});
    std::array<float, 4> overlapping{1, 2, 3, 4};
    auto overlap_view = Fixed::from_bytes(std::as_writable_bytes(std::span(overlapping).first<3>())).value();
    overlap_view.copy_to(std::span(overlapping).subspan<1>());
    assert((overlapping == std::array<float, 4>{1, 1, 2, 3}));
    auto empty_sequence = decltype(view.nested())::create(std::vector<std::array<float, 3>>{});
    assert(empty_sequence.view_unchecked().begin() == empty_sequence.view_unchecked().end());
    empty_sequence.view_unchecked().copy_to(std::span<std::array<float, 3>>{});
    // Measurement rejects an unrepresentable payload before touching storage.
    alignas(4) std::array<std::byte, 8> too_small;
    too_small.fill(std::byte{0xa5});
    const auto maximum = std::size_t{UINT32_MAX};
    for (const auto count : {maximum, maximum / Fixed::byte_size}) {
        try {
            decltype(view.nested())::create_in_place(too_small, intrlay::zeroed(count));
            assert(false);
        } catch (const std::length_error&) {}
    }
    assert(std::ranges::all_of(too_small, [](auto byte) { return byte == std::byte{0xa5}; }));
    assert(readonly.optional().get(0) == 47u && !readonly.optional().get(1));
    assert(readonly.tree().get(0).children().get(0).value() == 59);
    assert(readonly.flags().get(1));

    // External const storage needs no lifetime preparation or writable cast.
    static constexpr auto bytes = std::bit_cast<std::array<std::byte, 12>>(std::array<float, 3>{61, 67, 71});
    auto borrowed = generated::IntrlayArray3OfF32ViewReadOnly::from_bytes(bytes).value();
    assert(borrowed.get(0) == 61 && borrowed.get(2) == 71);
    auto triplet = generated::TripletView::create(std::array<float, 3>{61, 67, 71});
    assert(std::ranges::equal(triplet.bytes(), bytes));
    assert(generated::TripletView::compute_byte_length(std::array<float, 3>{}) == 12);

    const std::array<float, 3> adl_values{61, 67, 71};
    const AdlSourceRange<float> adl{adl_values};
    static_assert(std::ranges::contiguous_range<const decltype(adl)>);
    static_assert(intrlay::EncodeSourceFor<decltype(adl), Fixed::Metadata>);
    assert(std::ranges::equal(Fixed::create(adl).bytes(), bytes));
    static_assert(intrlay::EncodeSourceFor<ExactIndexRange, Fixed::Metadata>);
    static_assert(std::ranges::contiguous_range<const WrongIndexRange>);
    static_assert(!intrlay::EncodeSourceFor<WrongIndexRange, Fixed::Metadata>);
    assert(std::ranges::equal(Fixed::create(ExactIndexRange{adl_values}).bytes(), bytes));
    static_assert(intrlay::EncodeSourceFor<CountedSourceRange<float>, Fixed::Metadata>);
    assert(std::ranges::equal(Fixed::create(CountedSourceRange<float>{adl_values}).bytes(), bytes));
    triplet.view_unchecked().replace(adl);
    assert(std::ranges::equal(triplet.bytes(), bytes));
    const std::array<std::array<float, 3>, 2> adl_arrays{adl_values, adl_values};
    auto adl_sequence = decltype(view.nested())::create(AdlSourceRange<std::array<float, 3>>{adl_arrays});
    assert(adl_sequence.view_unchecked().size() == 2 && adl_sequence.view_unchecked().get(1).get(2) == 71);
    auto counted_sequence = decltype(view.nested())::create(CountedSourceRange<std::array<float, 3>>{adl_arrays});
    assert(std::ranges::equal(counted_sequence.bytes(), adl_sequence.bytes()));

    // Both collection writers pass an lvalue index. Exercise non-native elements
    // so these positive cases reach indexed writing rather than the bulk-copy path.
    using PaddedArray = decltype(view.padded());
    static_assert(intrlay::EncodeSourceFor<LvalueIndexRange<generated::PaddedInput<>>, PaddedArray::Metadata>);
    static_assert(!intrlay::EncodeSourceFor<RvalueIndexRange<generated::PaddedInput<>>, PaddedArray::Metadata>);
    const std::array<generated::PaddedInput<>, 2> padded_values{{{73}, {79}}};
    auto padded_array = PaddedArray::create(LvalueIndexRange<generated::PaddedInput<>>{padded_values});
    assert(padded_array.view_unchecked().get(0).value() == 73 && padded_array.view_unchecked().get(1).value() == 79);

    using BranchSequence = decltype(view.tree());
    static_assert(intrlay::EncodeSourceFor<LvalueIndexRange<generated::BranchInput<>>, BranchSequence::Metadata>);
    static_assert(!intrlay::EncodeSourceFor<RvalueIndexRange<generated::BranchInput<>>, BranchSequence::Metadata>);
    const std::array<generated::BranchInput<>, 2> branches{{{83, {{89, {}}}}, {97, {}}}};
    auto branch_sequence = BranchSequence::create(LvalueIndexRange<generated::BranchInput<>>{branches});
    assert(branch_sequence.view_unchecked().get(0).children().get(0).value() == 89);
    assert(branch_sequence.view_unchecked().get(1).value() == 97);

    alignas(4) std::array<std::byte, 24> storage{};
    auto in_place = Fixed::create_in_place(std::span(storage).subspan(8), std::array<float, 3>{73, 79, 83});
    assert(in_place.bytes().size() == 12 && in_place.get(1) == 79);
    assert(Fixed::from_parts(storage, 8).value().get(2) == 83);
    const auto const_copy = generated::IntrlayArray3OfF32ViewReadOnly::create(std::array<float, 3>{89, 97, 101});
    assert(generated::IntrlayArray3OfF32ViewReadOnly::from_bytes(const_copy.bytes()).value().get(2) == 101);

    std::vector<std::optional<std::unique_ptr<std::uint32_t>>> scalar_input;
    scalar_input.emplace_back(std::make_unique<std::uint32_t>(103));
    scalar_input.emplace_back();
    auto sequence = generated::OptionalSequenceView::create(scalar_input);
    sequence.view_unchecked().set(0, 107u);
    sequence.view_unchecked().set(1, 109u);
    assert(sequence.view_unchecked().get(0) == 107u && !sequence.view_unchecked().get(1));
    using Empty = decltype(view.empty());
    auto empty = Empty::create(std::array<double, 0>{});
    assert(empty.bytes().empty() && empty.view_unchecked().size() == 0);
    assert(empty.view_unchecked().begin() == empty.view_unchecked().end());
    auto units = generated::UnitTripletView::from_bytes(std::span<std::byte>{}).value();
    auto unit_iterator = units.begin();
    auto first_unit = unit_iterator++;
    assert(first_unit != unit_iterator && *first_unit == *unit_iterator);
    assert(std::ranges::distance(units) == 3);

    namespace custom = cpp_collections::custom;
    auto named = custom::ApiCollectionsView::BuildCreate(input);
    assert(std::ranges::equal(named.bytes(), reference));
    // Owners use internal traits even when every public factory is renamed.
    auto named_checked = named.view();
    assert(named_checked);
    auto named_view = named_checked.value();
    auto named_fixed = named_view.ReadFixed();
    using NamedFixed = decltype(named_fixed);
    static_assert(NamedFixed::LayoutByteSize == 12 && NamedFixed::LayoutAlignment == 4);
    static_assert(NamedFixed::extent == 3);
    static_assert(CanAccessSet<NamedFixed, float>);
    static_assert(!CanAccessSet<NamedFixed, double>);
    static_assert(!CanAccessSet<NamedFixed, int>);
    static_assert(!CanAccessReplace<NamedFixed, int>);
    named_fixed.AccessSet(1, 113.0f);
    named_view.ReadOptional().AccessSet(2, 127u);
    named_view.ReadNested().AccessGet(1).AccessSet(2, 131.0f);
    assert(named_fixed.AccessGet(1) == 113 && named_fixed.ReadSize() == 3);
    static_assert(std::ranges::forward_range<NamedFixed>);
    assert(std::ranges::equal(named_fixed, std::array<float, 3>{1, 113, 3}));
    auto named_readonly = custom::ApiCollectionsViewReadOnly::BuildFromBytes(named.bytes()).value();
    static_assert(!CanAccessSet<decltype(named_readonly.ReadFixed()), float>);
    assert(named_readonly.ReadOptional().AccessGet(2) == 127u);
    assert(named_readonly.ReadNested().AccessGet(1).AccessGet(2) == 131);
    assert(named_readonly.ReadTree().AccessGet(0).ReadChildren().AccessGet(0).ReadValue() == 19);
    auto named_array = NamedFixed::BuildCreate(std::array<float, 3>{137, 139, 149});
    static_assert(std::is_same_v<decltype(named_array.view_unchecked()), NamedFixed>);
    auto named_array_checked = named_array.view();
    assert(named_array_checked && named_array_checked->AccessGet(2) == 149);
    auto named_padded = custom::ApiPaddedView::BuildCreate(custom::ApiPaddedInput{17.0f});
    assert(named_padded.view().value().ReadValue() == 17);
    intrlay::OwnedView<NamedFixed> truncated(NamedFixed::LayoutByteSize - 1, NamedFixed::LayoutAlignment);
    assert(!truncated.view() && truncated.view().error() == intrlay::ViewError::OutOfBounds);
    assert(NamedFixed::BuildComputeByteLength(std::array<float, 3>{}) == 12);
    auto named_in_place = NamedFixed::BuildCreateInPlace(storage, std::array<float, 3>{151, 157, 163});
    assert(named_in_place.AccessGet(0) == 151 && named_in_place.ReadBytes().size() == 12);
    named_in_place.AccessReplace(std::array<float, 3>{167, 173, 179});
    assert(named_in_place.AccessGet(1) == 173);
    named_in_place.AccessCopyTo(std::span(fixed_copy));
    assert(fixed_copy[1] == 173);
    std::cout << "Shared C++ collection views passed\n";
}
