#pragma once
#include "codecs.hpp"

namespace intrlay {

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

    explicit ArrayView(std::span<Byte> bytes, std::size_t offset = 0)
        : buffer_(bytes), offset_(offset) {}

    std::span<Byte> bytes() const { return buffer_; }
    std::size_t size() const { return Descriptor::size(buffer_, offset_); }
    auto get(std::size_t index) const { return Descriptor::get(buffer_, offset_, index); }

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

} // namespace intrlay
