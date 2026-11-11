#pragma once
#include "runtime.hpp"

namespace intrlay {

class Validator;

namespace detail {
template<class Descriptor>
concept ValidatableDescriptor = requires(Validator& validator, std::size_t offset) {
    { Descriptor::byte_size } -> std::convertible_to<std::size_t>;
    { Descriptor::all_bits_valid() } -> std::convertible_to<bool>;
    { Descriptor::validate_fields(validator, offset) } noexcept -> std::same_as<Expected<void, ViewError>>;
};
} // namespace detail

/// One pass validates a tree without copying payloads or allocating visited sets.
/// Reservations follow wire order. Forward gaps are allowed; overlapping payloads
/// are rejected so a fixed mutation cannot overwrite another value's references.
class Validator {
    std::span<const std::byte> bytes_;
    std::size_t reserved_end_ = 0;
    std::size_t remaining_depth_;

public:
    explicit Validator(std::span<const std::byte> bytes, std::size_t max_depth = 128) noexcept
        : bytes_(bytes), remaining_depth_(max_depth) {}

    template<detail::ValidatableDescriptor Descriptor>
    Expected<void, ViewError> validate(std::size_t offset = 0) noexcept {
        if (!detail::contains_region(bytes_, offset, Descriptor::byte_size)) return Unexpected{ViewError::OutOfBounds};
        reserved_end_ = offset + Descriptor::byte_size;
        return field<Descriptor>(offset);
    }

    template<detail::ValidatableDescriptor Descriptor>
    Expected<void, ViewError> field(std::size_t offset) noexcept {
        if (!detail::contains_region(bytes_, offset, Descriptor::byte_size)) return Unexpected{ViewError::OutOfBounds};
        if constexpr (Descriptor::all_bits_valid()) return {};
        else {
            if (remaining_depth_ == 0) return Unexpected{ViewError::DepthLimit};
            --remaining_depth_;
            auto result = Descriptor::validate_fields(*this, offset);
            ++remaining_depth_;
            return result;
        }
    }

    template<class T>
        requires (std::is_arithmetic_v<T> && !std::same_as<T, bool>)
    Expected<T, ViewError> read(std::size_t offset) const noexcept {
        if (!detail::contains_region(bytes_, offset, sizeof(T))) return Unexpected{ViewError::OutOfBounds};
        return detail::load<T>(bytes_, offset);
    }

    Expected<std::size_t, ViewError> payload(std::size_t offset, std::size_t header_size,
                                           std::size_t count, std::size_t stride) noexcept {
        if (!detail::contains_region(bytes_, offset, header_size)) return Unexpected{ViewError::OutOfBounds};
        const auto relative = detail::load<std::uint32_t>(bytes_, offset);
        if (relative < header_size) return Unexpected{ViewError::InvalidReference};
        const auto target = detail::checked_offset(bytes_, offset, relative);
        if (!target) return Unexpected{target.error()};
        if (*target < reserved_end_) return Unexpected{ViewError::InvalidReference};
        if (!detail::contains_array(bytes_, *target, count, stride)) return Unexpected{ViewError::OutOfBounds};
        reserved_end_ = *target + count * stride;
        return *target;
    }

    template<detail::ValidatableDescriptor Element>
    Expected<void, ViewError> elements(std::size_t offset, std::size_t count, std::size_t stride) noexcept {
        if (!detail::contains_array(bytes_, offset, count, stride)) return Unexpected{ViewError::OutOfBounds};
        if constexpr (!Element::all_bits_valid()) {
            for (std::size_t i = 0; i < count; ++i) {
                auto result = field<Element>(offset + i * stride);
                if (!result) return result;
            }
        }
        return {};
    }

    Expected<void, ViewError> utf8(std::size_t offset, std::size_t length) const noexcept {
        if (!detail::contains_region(bytes_, offset, length)) return Unexpected{ViewError::OutOfBounds};
        const auto end = offset + length;
        while (offset < end) {
            // ASCII runs need no per-byte decoding. memcpy-based loads support
            // unaligned buffers and never read past the validated string region.
            if (end - offset >= sizeof(std::uint64_t)
                && (detail::load<std::uint64_t>(bytes_, offset) & UINT64_C(0x8080808080808080)) == 0) {
                offset += sizeof(std::uint64_t);
                continue;
            }
            const auto first = std::to_integer<unsigned char>(bytes_[offset++]);
            if (first < 0x80) continue;
            const std::size_t remaining = first >= 0xc2 && first <= 0xdf ? 1
                : first >= 0xe0 && first <= 0xef ? 2 : first >= 0xf0 && first <= 0xf4 ? 3 : 0;
            if (remaining == 0 || remaining > end - offset) return Unexpected{ViewError::InvalidUtf8};
            const auto second = std::to_integer<unsigned char>(bytes_[offset]);
            if ((first == 0xe0 && second < 0xa0) || (first == 0xed && second >= 0xa0)
                || (first == 0xf0 && second < 0x90) || (first == 0xf4 && second >= 0x90))
                return Unexpected{ViewError::InvalidUtf8};
            for (std::size_t i = 0; i < remaining; ++i) {
                if ((std::to_integer<unsigned char>(bytes_[offset++]) & 0xc0) != 0x80)
                    return Unexpected{ViewError::InvalidUtf8};
            }
        }
        return {};
    }
};

namespace detail {
/// Shared validation for public view factories and owning-buffer entry points.
/// View construction is delayed until its type and descriptor are complete.
template<class View, ValidatableDescriptor Descriptor>
struct CheckedViewFactory {
    template<BufferByte Byte>
        requires std::constructible_from<View, std::span<Byte>, std::size_t, Unchecked>
    static Expected<View, ViewError> from_parts(std::span<Byte> bytes, std::size_t offset) noexcept {
        auto result = Validator(bytes).template validate<Descriptor>(offset);
        if (!result) return Unexpected{result.error()};
        return View(bytes, offset, Unchecked{});
    }

    template<BufferByte Byte>
        requires std::constructible_from<View, std::span<Byte>, std::size_t, Unchecked>
    static Expected<View, ViewError> from_bytes(std::span<Byte> bytes) noexcept {
        return from_parts(bytes, 0);
    }
};
} // namespace detail

} // namespace intrlay
