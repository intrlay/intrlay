#pragma once
#include "adapter.hpp"
#include "expected.hpp"
#include <algorithm>
#include <array>
#include <bit>
#include <concepts>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <limits>
#include <memory>
#include <new>
#include <optional>
#include <ranges>
#include <span>
#include <stdexcept>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <variant>
#include <vector>

// Exposes the writer's non-overlap contract to supported optimizers.
#if defined(_MSC_VER)
#define INTRLAY_RESTRICT __restrict
#elif defined(__clang__) || defined(__GNUC__)
#define INTRLAY_RESTRICT __restrict__
#else
#define INTRLAY_RESTRICT
#endif

namespace intrlay {
static_assert(std::endian::native == std::endian::little, "Intrlay requires little-endian storage");

/// Views borrow byte storage; const bytes expose the same read operations.
template<class T>
concept BufferByte = std::same_as<T, std::byte> || std::same_as<T, const std::byte>;

/// Mutation requires writable bytes, independently of the view object's constness.
template<class T>
concept WritableBufferByte = std::same_as<T, std::byte>;

namespace detail {
/// Explicit construction after validation or the caller's trusted-buffer proof.
struct Unchecked {};
/// Connects a view to its checked factory independently of public naming policy.
template<class View> struct ViewTraits;

inline bool contains_region(std::span<const std::byte> bytes, std::size_t offset, std::size_t size) noexcept {
    return offset <= bytes.size() && size <= bytes.size() - offset;
}

inline Expected<std::size_t, ViewError> checked_offset(std::span<const std::byte> bytes, std::size_t base, std::size_t relative) noexcept {
    if (!contains_region(bytes, base, relative)) return Unexpected{ViewError::OutOfBounds};
    return base + relative;
}

inline bool contains_array(std::span<const std::byte> bytes, std::size_t offset, std::size_t count, std::size_t stride) noexcept {
    return offset <= bytes.size() && (stride == 0 || count <= (bytes.size() - offset) / stride);
}

template<class T> struct NativeArray : std::false_type {};
template<class T, std::size_t N> struct NativeArray<std::array<T, N>> : std::true_type {
    using Element = T;
    static constexpr auto length = N;
};
template<class T, std::size_t N> struct NativeArray<T[N]> : std::true_type {
    using Element = T;
    static constexpr auto length = N;
};

/// A descriptor may opt into copying native objects only after proving their
/// representation. Unsupported inputs retain field-wise encoding.
template<class Codec, class T>
inline constexpr bool native_layout = [] {
    if constexpr (std::is_volatile_v<std::remove_reference_t<T>>) return false;
    else if constexpr (!adapter_preserves_native_layout<T>) return false;
    else if constexpr (requires { Codec::template native_layout<std::remove_cvref_t<T>>; })
        return Codec::template native_layout<std::remove_cvref_t<T>>;
    else return false;
}();

/// Copies into live native elements; never creates native objects inside the
/// encoded byte buffer. Unaligned input and overlapping ranges are supported.
template<class Element, std::size_t Stride, BufferByte Byte, class T, std::size_t Extent>
    requires (!std::is_const_v<T> && !std::is_volatile_v<T>
        && native_layout<Element, T> && sizeof(T) == Stride)
void copy_native_elements(std::span<Byte> bytes, std::size_t offset, std::size_t count,
                          std::span<T, Extent> destination) {
    if (destination.size() != count) throw std::length_error("native copy element count");
    if (count != 0) std::memmove(destination.data(), bytes.data() + offset, count * Stride);
}

struct Cursor {
    std::size_t end;

    std::size_t reserve(std::size_t size, std::size_t alignment) {
        constexpr auto maximum = std::numeric_limits<std::uint32_t>::max();
        if (end > maximum - (alignment - 1)) {
            throw std::length_error("Intrlay alignment overflow");
        }
        auto start = (end + alignment - 1) & ~(alignment - 1);
        if (size > maximum - start) {
            throw std::length_error("Intrlay buffer exceeds u32 capacity");
        }
        end = start + size;
        return start;
    }

    // Combine alignment padding and, when needed, an incomplete fixed header in
    // one initialization. Fully overwritten payloads need only the padding.
    // Measurement has checked u32 arithmetic; the factory has checked capacity.
    // As with the codec's stores, writing requires that same, stable source.
    template<bool ClearPayload = false>
    std::size_t reserve(std::span<std::byte> bytes, std::size_t size, std::size_t alignment) {
        const auto previous = end;
        const auto start = (previous + alignment - 1) & ~(alignment - 1);
        end = start + size;
        const auto initialized_end = ClearPayload ? end : start;
        if (initialized_end != previous)
            std::fill_n(bytes.data() + previous, initialized_end - previous, std::byte{});
        return start;
    }
};

inline std::size_t array_bytes(std::size_t count, std::size_t stride) {
    constexpr auto maximum = std::numeric_limits<std::uint32_t>::max();
    if (count > maximum || (stride != 0 && count > maximum / stride)) {
        throw std::length_error("Intrlay array size overflow");
    }
    return count * stride;
}

template<class T>
    requires (std::is_trivially_copyable_v<T> && std::default_initializable<T>
        && std::same_as<T, std::remove_cv_t<T>>)
T load(std::span<const std::byte> bytes, std::size_t offset) noexcept {
    T value;
    std::memcpy(&value, bytes.data() + offset, sizeof(T));
    return value;
}
template<class T>
    requires std::is_trivially_copyable_v<T>
void store(std::span<std::byte> bytes, std::size_t offset, T value) noexcept {
    std::memcpy(bytes.data() + offset, &value, sizeof(T));
}
inline std::size_t dereference(std::span<const std::byte> bytes, std::size_t offset) noexcept {
    return offset + load<std::uint32_t>(bytes, offset);
}

inline std::string_view string_at(std::span<const std::byte> bytes, std::size_t offset) noexcept {
    const auto start = dereference(bytes, offset);
    const auto length = load<std::uint32_t>(bytes, offset + 4);
    return {reinterpret_cast<const char*>(bytes.data() + start), length};
}

inline void reference(std::span<std::byte> bytes, std::size_t offset, std::size_t target) {
    store<std::uint32_t>(bytes, offset, static_cast<std::uint32_t>(target - offset));
}
template<class T>
const auto& variant(const T& value) {
    if constexpr (requires { value.value; }) {
        return value.value;
    } else {
        return value;
    }
}

struct AlignedDelete {
    std::size_t alignment;

    void operator()(std::byte* bytes) const noexcept {
        ::operator delete(bytes, std::align_val_t(alignment));
    }
};
} // namespace detail

template<class View>
class OwnedView {
    std::unique_ptr<std::byte, detail::AlignedDelete> storage_;
    std::size_t size_;
public:
    explicit OwnedView(std::size_t size, std::size_t alignment, bool clear = true)
        : storage_(
              static_cast<std::byte*>(::operator new(
                  std::max<std::size_t>(1, size),
                  std::align_val_t(std::max(alignment, alignof(std::max_align_t))))),
              detail::AlignedDelete{std::max(alignment, alignof(std::max_align_t))}),
          size_(size) {
        if (clear) std::fill_n(storage_.get(), size_, std::byte{});
    }

    OwnedView(const OwnedView&) = delete;
    OwnedView& operator=(const OwnedView&) = delete;
    OwnedView(OwnedView&& other) noexcept
        : storage_(std::move(other.storage_)), size_(std::exchange(other.size_, 0)) {}
    OwnedView& operator=(OwnedView&& other) noexcept {
        if (this != &other) {
            storage_ = std::move(other.storage_);
            size_ = std::exchange(other.size_, 0);
        }
        return *this;
    }

    std::span<std::byte> bytes() { return {storage_.get(), size_}; }
    std::span<const std::byte> bytes() const { return {storage_.get(), size_}; }
    /// Validates the current bytes before borrowing them. Raw bytes() access may
    /// have modified metadata since construction, so ownership alone is not a proof.
    Expected<View, ViewError> view() noexcept
        requires requires(std::span<std::byte> bytes) { { detail::ViewTraits<View>::from_bytes(bytes) } -> std::same_as<Expected<View, ViewError>>; } {
        return detail::ViewTraits<View>::from_bytes(bytes());
    }
    /// Borrows an initialized, unchanged encoding without another validation pass.
    View view_unchecked() noexcept
        requires std::constructible_from<View, std::span<std::byte>, std::size_t, detail::Unchecked> {
        return View(bytes(), 0, detail::Unchecked{});
    }
};

} // namespace intrlay
