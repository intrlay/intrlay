#include <intrlay/annotations.hpp>
#include <cstdint>
#include <memory>

namespace forward_fixture {
#ifndef INTRLAY_DEFINE_FIRST
struct Child;
enum class Mode : std::uint32_t;

struct INTRLAY() Root {
    std::unique_ptr<Child> child;
    Mode mode;
};
#endif

#ifdef INTRLAY_INHERITANCE
struct Base {
    std::uint32_t base_id;
};
struct Child : Base {
    std::uint32_t value;
};
#else
/// An explicitly aligned child.
struct alignas(16) INTRLAY(transparent) Child {
    /// Child identifier.
    std::uint32_t value;
};
#endif

/// Operating mode.
enum class INTRLAY(name="OperatingMode") Mode : std::uint32_t {
    /// Device is enabled.
    On INTRLAY(name="Enabled") = 7,
    Off = 9,
};

#ifdef INTRLAY_DEFINE_FIRST
struct INTRLAY() Root {
    std::unique_ptr<Child> child;
    Mode mode;
};
#endif
}
