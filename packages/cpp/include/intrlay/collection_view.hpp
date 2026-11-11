#pragma once
#include "codecs.hpp"
#include "validation.hpp"

namespace intrlay {

/// Traverses encoded elements by advancing a byte cursor, without materializing
/// native objects. Element supplies borrowed reads, Byte preserves const access,
/// and Stride is the element's wire size, including padding.
template<class Element, BufferByte Byte, std::size_t Stride>
    requires requires(std::span<Byte> bytes, std::size_t offset) { Element::read(bytes, offset); }
class ArrayIterator {
    std::span<Byte> buffer_;
    Byte* position_ = nullptr;
    std::size_t remaining_ = 0;

public:
    using value_type = decltype(Element::read(std::declval<std::span<Byte>>(), std::size_t{}));
    using difference_type = std::ptrdiff_t;
    using iterator_concept = std::forward_iterator_tag;
    // Legacy forward iterators require references; these reads return values/views.
    using iterator_category = std::input_iterator_tag;

    ArrayIterator() noexcept = default;
    /// Internal/trusted construction with a validated element region.
    ArrayIterator(std::span<Byte> bytes, std::size_t offset, std::size_t count, detail::Unchecked) noexcept
        : buffer_(bytes), position_(offset == 0 ? bytes.data() : bytes.data() + offset), remaining_(count) {}

    /// Requires a non-end iterator. Child views retain the complete parent buffer.
    value_type operator*() const noexcept {
        if constexpr (Stride == 0) {
            // A nonempty array of units may occupy an empty, null-backed buffer.
            return Element::read(buffer_, position_ == nullptr ? 0 : static_cast<std::size_t>(position_ - buffer_.data()));
        } else {
            return Element::read(buffer_, static_cast<std::size_t>(position_ - buffer_.data()));
        }
    }

    /// Requires a non-end iterator; a count also supports zero-size elements.
    ArrayIterator& operator++() noexcept {
        if constexpr (Stride != 0) position_ += Stride;
        --remaining_;
        return *this;
    }
    ArrayIterator operator++(int) noexcept {
        auto previous = *this;
        ++*this;
        return previous;
    }

    friend bool operator==(const ArrayIterator& left, const ArrayIterator& right) noexcept {
        return left.buffer_.data() == right.buffer_.data()
            && left.position_ == right.position_ && left.remaining_ == right.remaining_;
    }
    friend bool operator==(const ArrayIterator& iterator, std::default_sentinel_t) noexcept {
        return iterator.remaining_ == 0;
    }
};

/// Buffer requirements resolved by the generator, including indirect payloads.
template<std::size_t Size, std::size_t Alignment, bool FullyWritten = false>
struct BufferLayout {
    static constexpr std::size_t byte_size = Size;
    static constexpr std::size_t alignment = Alignment;
    static constexpr bool fully_written = FullyWritten;
};

/// Borrows a fixed array or a sequence header without creating native elements.
/// Descriptor supplies its resolved layout, extent, codecs, and element access.
/// Byte controls mutable/const access; Constructible enables buffer factories.
template<class Descriptor, BufferByte Byte = std::byte, bool Constructible = true>
class ArrayView {
    std::span<Byte> buffer_;
    std::size_t offset_;

public:
    using Metadata = Descriptor;
    static constexpr std::size_t byte_size = Descriptor::byte_size;
    static constexpr std::size_t alignment = Descriptor::alignment;
    static constexpr std::size_t extent = Descriptor::extent;

    /// Internal/trusted construction; callers guarantee a complete valid encoding.
    explicit ArrayView(std::span<Byte> bytes, std::size_t offset, detail::Unchecked) noexcept
        : buffer_(bytes), offset_(offset) {}

    static Expected<ArrayView, ViewError> from_bytes(std::span<Byte> bytes) noexcept {
        return from_parts(bytes, 0);
    }
    static Expected<ArrayView, ViewError> from_parts(std::span<Byte> bytes, std::size_t offset) noexcept {
        return detail::CheckedViewFactory<ArrayView, Descriptor>::from_parts(bytes, offset);
    }
    static ArrayView from_bytes_unchecked(std::span<Byte> bytes) noexcept {
        return from_parts_unchecked(bytes, 0);
    }
    static ArrayView from_parts_unchecked(std::span<Byte> bytes, std::size_t offset) noexcept {
        return ArrayView(bytes, offset, detail::Unchecked{});
    }

    std::span<Byte> bytes() const { return buffer_; }
    std::size_t size() const noexcept { return Descriptor::size(buffer_, offset_); }
    /// Like span indexing, requires index < size(); construction validates the storage.
    auto get(std::size_t index) const noexcept { return Descriptor::get(buffer_, offset_, index); }

    /// Sequential reads use a byte cursor instead of recomputing index * stride.
    /// Scalar elements are values; record elements are borrowed views.
    auto begin() const noexcept { return Descriptor::begin(buffer_, offset_); }
    std::default_sentinel_t end() const noexcept { return {}; }

    /// Copies elements into existing native objects with a proven identical
    /// representation. Throws length_error if the destination count differs.
    /// Available on both mutable and read-only views; the input bytes stay borrowed.
    template<class T, std::size_t Extent>
    void copy_to(std::span<T, Extent> destination) const
        requires requires { Descriptor::copy_to(buffer_, offset_, destination); } {
        Descriptor::copy_to(buffer_, offset_, destination);
    }

    /// Replaces an inline element or its existing fixed payload; length stays fixed.
    template<class Value>
    void set(std::size_t index, const Value& value) const
        requires (WritableBufferByte<Byte> && requires {
            Descriptor::set(buffer_, offset_, index, value);
        }) {
        Descriptor::set(buffer_, offset_, index, value);
    }

    template<class Value>
    void replace(const Value& value) const
        requires (WritableBufferByte<Byte> && FixedEncodeSourceFor<Value, Descriptor>) {
        intrlay::replace<Descriptor>(buffer_, offset_, value);
    }

    template<class Value>
    static std::size_t compute_byte_length(const Value& value)
        requires (Constructible && EncodeSourceFor<Value, Descriptor>) {
        return ViewFactory<ArrayView, Descriptor, byte_size, alignment, Descriptor::fully_written>::compute_byte_length(value);
    }

    template<class Value>
    static OwnedView<ArrayView> create(const Value& value)
        requires (Constructible && EncodeSourceFor<Value, Descriptor>) {
        return ViewFactory<ArrayView, Descriptor, byte_size, alignment, Descriptor::fully_written>::create(value);
    }

    template<class Value>
    static ArrayView create_in_place(std::span<std::byte> bytes, const Value& value)
        requires (Constructible && EncodeSourceFor<Value, Descriptor>) {
        return ViewFactory<ArrayView, Descriptor, byte_size, alignment, Descriptor::fully_written>::create_in_place(bytes, value);
    }
};

namespace detail {
template<ValidatableDescriptor Descriptor, BufferByte Byte, bool Constructible>
struct ViewTraits<ArrayView<Descriptor, Byte, Constructible>>
    : CheckedViewFactory<ArrayView<Descriptor, Byte, Constructible>, Descriptor> {};
} // namespace detail

} // namespace intrlay
