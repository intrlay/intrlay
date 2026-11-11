#pragma once
#include <concepts>
#include <memory>
#include <optional>
#include <type_traits>
#include <utility>
#include <variant>

namespace intrlay {

/// Distinguishes an error from a successful value, including when T and E match.
template<class E>
    requires (std::is_object_v<E> && !std::is_array_v<E>)
struct Unexpected {
    E error;
};

/// A C++20 value-or-error result. Storage is inline, with no allocation or boxing.
/// Inspect has_value()/operator bool before dereferencing, or use value() for
/// checked access (std::bad_variant_access on an error). No implicit T conversion.
template<class T, class E>
    requires (((std::is_object_v<T> && !std::is_array_v<T>) || std::is_void_v<T>)
        && std::is_object_v<E> && !std::is_array_v<E>)
class [[nodiscard]] Expected {
    // Give trivial error storage the union's alignment. A short alternative
    // otherwise makes scalar replacement split pointer-sized view members into
    // smaller loads/stores. This padding does not increase the variant's size.
#if defined(_MSC_VER)
#pragma warning(push)
#pragma warning(disable: 4324) // This wrapper intentionally pads the error to the union's alignment.
#endif
    struct alignas(alignof(T) > alignof(E) ? alignof(T) : alignof(E)) AlignedError {
        E value;
    };
#if defined(_MSC_VER)
#pragma warning(pop)
#endif
    static constexpr bool align_error = std::is_trivially_copyable_v<E> && alignof(E) < alignof(T);
    using ErrorStorage = std::conditional_t<align_error, AlignedError, E>;
    using Storage = std::variant<T, ErrorStorage>;
    Storage storage_;

    static decltype(auto) error_storage(E& error) noexcept {
        if constexpr (align_error) return AlignedError{std::move(error)};
        // Non-trivial errors construct directly in the variant, without an extra move.
        else return std::move(error);
    }
    template<class S> requires std::same_as<std::remove_const_t<S>, Storage>
    static auto& stored_error(S& storage) noexcept {
        if constexpr (align_error) return std::get_if<1>(&storage)->value;
        else return *std::get_if<1>(&storage);
    }

public:
    using value_type = T;
    using error_type = E;

    Expected(const T& value) requires std::copy_constructible<T>
        : storage_(std::in_place_index<0>, value) {}
    Expected(T&& value) noexcept(std::is_nothrow_move_constructible_v<T>)
        requires std::move_constructible<T>
        : storage_(std::in_place_index<0>, std::move(value)) {}
    Expected(Unexpected<E> error) noexcept(std::is_nothrow_move_constructible_v<E>)
        : storage_(std::in_place_index<1>, error_storage(error.error)) {}

    bool has_value() const noexcept { return storage_.index() == 0; }
    explicit operator bool() const noexcept { return has_value(); }

    T& value() & { return std::get<0>(storage_); }
    const T& value() const & { return std::get<0>(storage_); }
    T&& value() && { return std::get<0>(std::move(storage_)); }
    const T&& value() const && { return std::get<0>(std::move(storage_)); }

    T& operator*() & noexcept { return *std::get_if<0>(&storage_); }
    const T& operator*() const & noexcept { return *std::get_if<0>(&storage_); }
    T&& operator*() && noexcept { return std::move(*std::get_if<0>(&storage_)); }
    const T&& operator*() const && noexcept { return std::move(*std::get_if<0>(&storage_)); }
    T* operator->() noexcept { return std::get_if<0>(&storage_); }
    const T* operator->() const noexcept { return std::get_if<0>(&storage_); }

    /// Like operator*, error() requires the corresponding alternative to be active.
    E& error() & noexcept { return stored_error(storage_); }
    const E& error() const & noexcept { return stored_error(storage_); }
    E&& error() && noexcept { return std::move(stored_error(storage_)); }
    const E&& error() const && noexcept { return std::move(stored_error(storage_)); }
};

/// Result of validation or another operation with no successful return value.
template<class E>
class [[nodiscard]] Expected<void, E> {
    std::optional<E> error_;

public:
    using value_type = void;
    using error_type = E;

    Expected() noexcept = default;
    Expected(Unexpected<E> error) noexcept(std::is_nothrow_move_constructible_v<E>)
        : error_(std::move(error.error)) {}

    bool has_value() const noexcept { return !error_.has_value(); }
    explicit operator bool() const noexcept { return has_value(); }
    void value() const { if (error_) throw std::bad_variant_access{}; }
    E& error() & noexcept { return *error_; }
    const E& error() const & noexcept { return *error_; }
    E&& error() && noexcept { return std::move(*error_); }
    const E&& error() const && noexcept { return std::move(*error_); }
};

/// Errors detected while accessing encoded bytes, independent of schema names.
enum class ViewError {
    OutOfBounds,
    InvalidReference,
    InvalidTag,
    InvalidUtf8,
    DepthLimit,
};

} // namespace intrlay
