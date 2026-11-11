#include <intrlay/reflection.hpp>
#include <array>

struct ByteArrayDescriptor;
template<class Byte>
using ByteArrayView = intrlay::ArrayView<ByteArrayDescriptor, Byte>;
struct ByteArrayDescriptor : intrlay::Array<ByteArrayView, intrlay::Scalar<std::uint8_t>, 1, 3>,
    intrlay::BufferLayout<3, 1> {};

int main() {
    const std::array<std::byte, 3> bytes{std::byte{1}, std::byte{7}, std::byte{3}};
    const auto result = ByteArrayView<const std::byte>::from_bytes(bytes);
    if (!result) return 1;
    const auto values = *result;
    std::array<std::byte, 4> encoded{};
    intrlay::detail::Cursor cursor{4};
    intrlay::ScalarCodec<std::uint32_t>::write(encoded, 0, 0x1234u, cursor);
    return values.size() == 3 && values.get(1) == 7 &&
        intrlay::detail::load<std::uint32_t>(encoded, 0) == 0x1234u ? 0 : 1;
}
