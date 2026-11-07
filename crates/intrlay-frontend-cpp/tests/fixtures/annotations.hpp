#include <intrlay/annotations.hpp>
#include <cstdint>

namespace annotation_fixture {
struct alignas(16) INTRLAY(transparent) CppId {
    std::uint32_t value;
};
struct INTRLAY() RenamedField {
    INTRLAY(name="exported_value", typescript.name="literalValue", python.name="literal_value")
    std::uint32_t native_value;
    CppId identifier;
};
enum class INTRLAY() CppChoice : std::uint32_t {
    Original INTRLAY(name="Common", typescript.name="Specific") = 4
};
}
