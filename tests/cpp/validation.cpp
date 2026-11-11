#ifdef NDEBUG
#undef NDEBUG
#endif
#include "bindings.hpp"
#include <cassert>
#include <fstream>
#include <iostream>

using namespace intrlay_generated::conformance;

static_assert(!intrlay::InputFieldsFor<int, std::tuple<>>);
static_assert(!intrlay::detail::ValidatableDescriptor<int>);

void check_results() {
    intrlay::Expected<int, int> value(7);
    intrlay::Expected<int, int> error(intrlay::Unexpected{9});
    assert(value && *value == 7 && !error && error.error() == 9);
    intrlay::Expected<std::unique_ptr<int>, int> owner(std::make_unique<int>(11));
    auto moved = std::move(owner);
    assert(moved && **moved == 11);
    // Pointer-sized values and smaller errors share aligned inline storage.
    using Words = std::array<std::uint64_t, 3>;
    using Result = intrlay::Expected<Words, intrlay::ViewError>;
    static_assert(sizeof(Result) == sizeof(std::variant<Words, intrlay::ViewError>));
    Result words(Words{13, 17, 19});
    Result invalid(intrlay::Unexpected{intrlay::ViewError::InvalidTag});
    words = invalid;
    assert(!words && std::as_const(words).error() == intrlay::ViewError::InvalidTag);
    invalid = Result(Words{23, 29, 31});
    assert(invalid && (*invalid)[2] == 31);
    words = std::move(invalid);
    assert(words && (*words)[0] == 23);

    // Non-trivial errors retain their move semantics and own their payload.
    intrlay::Expected<std::uint64_t, std::unique_ptr<int>> owned_error(
        intrlay::Unexpected{std::make_unique<int>(37)});
    auto moved_error = std::move(owned_error);
    assert(!moved_error && *moved_error.error() == 37);
    auto error_payload = std::move(moved_error).error();
    assert(*error_payload == 37 && !moved_error.error());
    intrlay::Expected<void, int> success;
    intrlay::Expected<void, int> failure(intrlay::Unexpected{13});
    assert(success && !failure && failure.error() == 13);
    try { error.value(); assert(false); } catch (const std::bad_variant_access&) {}
    try { failure.value(); assert(false); } catch (const std::bad_variant_access&) {}
}

void check_scalars_and_strings() {
    using Boolean = intrlay::Scalar<bool, std::uint8_t>;
    static_assert(intrlay::Scalar<float>::all_bits_valid());
    static_assert(PointView::Metadata::all_bits_valid());
    static_assert(!Boolean::all_bits_valid());
    for (unsigned int value = 0; value < 256; ++value) {
        const std::array bytes{std::byte(value)};
        assert(intrlay::Validator(bytes).validate<Boolean>().has_value() == (value <= 1));
        assert(intrlay::view<Boolean>(std::span<const std::byte>(bytes)).has_value() == (value <= 1));
    }
    // Validate all UTF-8 edge cases independently of header traversal.
    for (std::string_view text : {"ASCII", "한글 🦀", "\xc2\x80", "\xe0\xa0\x80", "\xf4\x8f\xbf\xbf"}) {
        auto bytes = std::as_bytes(std::span(text));
        assert(intrlay::Validator(bytes).utf8(0, bytes.size()));
    }
    for (std::string_view text : {"\x80", "\xc0\x80", "\xc2", "\xe0\x80\x80", "\xed\xa0\x80", "\xf0\x80\x80\x80", "\xf4\x90\x80\x80", "\xf5\x80\x80\x80"}) {
        auto bytes = std::as_bytes(std::span(text));
        auto result = intrlay::Validator(bytes).utf8(0, bytes.size());
        assert(!result && result.error() == intrlay::ViewError::InvalidUtf8);
    }
    // No out-of-buffer reads for enormous references/counts or overflowing bases.
    std::array<std::byte, 8> header{};
    intrlay::detail::store<std::uint32_t>(header, 0, UINT32_MAX);
    intrlay::detail::store<std::uint32_t>(header, 4, UINT32_MAX);
    assert(!intrlay::Validator(header).validate<intrlay::String>());
    assert(!intrlay::Validator(header).validate<intrlay::String>(SIZE_MAX));
    assert(intrlay::Validator({}).validate<intrlay::Unit>());

    auto point = PointView::create(PointInput{1.0f, 2.0f, 3.0f});
    auto checked_point = point.view();
    assert(checked_point && checked_point->x() == point.view_unchecked().x());
    auto projected = intrlay::view<PointView::Metadata>(point.bytes());
    assert(projected && projected->y() == 2);
    auto projected_const = intrlay::view<PointView::Metadata>(std::as_bytes(point.bytes()));
    static_assert(std::same_as<decltype(projected_const)::value_type, PointViewReadOnly>);
    assert(projected_const && projected_const->z() == 3);
    assert(!intrlay::view<PointView::Metadata>(point.bytes(), SIZE_MAX));
    assert(intrlay::view_unchecked<PointView::Metadata>(point.bytes()).y() == 2);

    // OwnedView is also a raw allocation type. An incomplete/unwritten owner
    // cannot claim valid view storage merely because it owns the allocation.
    intrlay::OwnedView<PointView> incomplete(1, 4);
    assert(!incomplete.view());
}

void check_moved_owners() {
    auto source = PointView::create(PointInput{1.0f, 2.0f, 3.0f});
    const auto allocation = source.bytes().data();
    auto moved = std::move(source);
    assert(moved.bytes().data() == allocation && moved.view()->z() == 3);
    assert(source.bytes().empty() && std::as_const(source).bytes().empty());
    assert(!source.view() && source.view().error() == intrlay::ViewError::OutOfBounds);

    // Move assignment also replaces an allocation with a different alignment.
    intrlay::OwnedView<PointView> assigned(PointView::Metadata::byte_size, 64);
    assigned = std::move(moved);
    assert(assigned.bytes().data() == allocation && assigned.view()->x() == 1);
    assert(moved.bytes().empty() && !moved.view());
    auto* self = &assigned;
    assigned = std::move(*self);
    assert(assigned.bytes().data() == allocation && assigned.view()->y() == 2);

    // A moved-from owner remains assignable, including from another empty owner.
    source = std::move(moved);
    assert(source.bytes().empty() && !source.view());
    source = std::move(assigned);
    assert(source.view()->z() == 3 && assigned.bytes().empty());
}

void check_failed_union_replacement() {
    // The same four bytes are raw u8s in one variant and bools in the other.
    // Committing just the bool tag before rejecting a short source is invalid.
    using Raw = intrlay::ArrayCodec<intrlay::Scalar<std::uint8_t>, 1, 4>;
    using Flags = intrlay::ArrayCodec<intrlay::Scalar<bool, std::uint8_t>, 1, 4>;
    using Choice = intrlay::EnumCodec<
        intrlay::VariantCodec<0, intrlay::RecordCodec<intrlay::FieldCodec<4, Raw, intrlay::AlternativeValue{}>>>,
        intrlay::VariantCodec<1, intrlay::RecordCodec<intrlay::FieldCodec<4, Flags, intrlay::AlternativeValue{}>>>>;
    using Input = std::variant<std::array<std::uint8_t, 4>, std::span<const bool>>;
    std::array<std::byte, 8> bytes{};
    intrlay::replace<Choice>(bytes, 0, Input{std::array<std::uint8_t, 4>{255, 255, 255, 255}});
    const auto before = bytes;
    const std::array flags{true};
    try {
        intrlay::replace<Choice>(bytes, 0, Input{std::span<const bool>(flags)});
        assert(false);
    } catch (const std::length_error&) {}
    assert(bytes == before && intrlay::detail::load<std::uint32_t>(bytes, 0) == 0);

    // Large fallible replacements use bounded stack space and preserve the same
    // guarantee. Native fixed arrays keep their non-throwing direct-write path.
    using Large = intrlay::ArrayCodec<intrlay::Scalar<std::uint32_t>, 4, 1024>;
    std::array<std::byte, 4096> large;
    large.fill(std::byte{0xa5});
    const std::array<std::uint32_t, 1024> values{};
    intrlay::detail::Cursor cursor{0};
    static_assert(noexcept(Large::write(large, 0, values, cursor)));
    try {
        intrlay::replace<Large>(large, 0, std::span(values).first(1023));
        assert(false);
    } catch (const std::length_error&) {}
    assert(std::ranges::all_of(large, [](auto value) { return value == std::byte{0xa5}; }));
    intrlay::replace<Large>(large, 0, std::span(values));
    assert(std::ranges::all_of(large, [](auto value) { return value == std::byte{}; }));
}

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    check_results();
    check_scalars_and_strings();
    check_moved_owners();
    check_failed_union_replacement();
    std::ifstream input(std::string(argv[1]) + "/rust.bin", std::ios::binary | std::ios::ate);
    assert(input);
    std::vector<std::byte> bytes(static_cast<std::size_t>(input.tellg()));
    input.seekg(0);
    input.read(reinterpret_cast<char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
    assert(input);

    auto checked = MessageView::from_bytes(bytes);
    assert(checked && checked->title() == "fixture 🦀 한글");
    auto trusted = MessageView::from_bytes_unchecked(bytes);
    assert(checked->sequence() == trusted.sequence());
    for (std::size_t length = 0; length < bytes.size(); ++length)
        assert(!MessageViewReadOnly::from_bytes(std::span(bytes).first(length)));
    assert(!MessageView::from_parts(bytes, SIZE_MAX));

    std::vector<std::byte> prefixed(3, std::byte{0xa5});
    prefixed.insert(prefixed.end(), bytes.begin(), bytes.end());
    auto relocated = MessageView::from_parts(prefixed, 3);
    assert(relocated && relocated->title() == trusted.title());
    relocated->set_sequence(std::uint64_t{42});
    assert(relocated->sequence() == 42 && prefixed.front() == std::byte{0xa5});

    // Point the string payload into the inline point-sequence header. Each
    // range separately fits, but the payload/header alias must be rejected.
    auto corrupted = bytes;
    constexpr auto title = MessageView::Metadata::Field<1>::offset;
    constexpr auto points = MessageView::Metadata::Field<2>::offset;
    intrlay::detail::store<std::uint32_t>(corrupted, title, static_cast<std::uint32_t>(points - title));
    assert(!MessageView::from_bytes(corrupted));

    // The outer vector is valid; a malformed active enum still fails validation.
    corrupted = bytes;
    constexpr auto commands = MessageView::Metadata::Field<3>::offset;
    const auto first_command = intrlay::detail::dereference(std::span<const std::byte>(corrupted), commands);
    intrlay::detail::store<std::uint32_t>(corrupted, first_command, UINT32_MAX);
    auto invalid_tag = MessageView::from_bytes(corrupted);
    assert(!invalid_tag && invalid_tag.error() == intrlay::ViewError::InvalidTag);

    assert(!intrlay::Validator(bytes, 1).validate<MessageView::Metadata>());
    std::cout << "C++20: checked/unchecked views, tags, UTF-8, overlap and truncation passed\n";
}
