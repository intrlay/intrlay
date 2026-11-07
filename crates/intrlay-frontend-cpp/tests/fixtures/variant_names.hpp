#include <intrlay/annotations.hpp>
#include <cstdint>
#include <memory>
#include <variant>

#if defined(INTRLAY_DUPLICATE)
using Choice INTRLAY() = std::variant<std::uint32_t, std::uint32_t>;
#elif defined(INTRLAY_OUT_OF_RANGE)
using Choice INTRLAY(alternative(2, name = "Third")) = std::variant<bool, std::uint32_t>;
#elif defined(INTRLAY_NOT_VARIANT)
using Scalar INTRLAY(alternative(0, name = "Scalar")) = std::uint32_t;
#elif defined(INTRLAY_CONFLICTING_ALIASES)
using Choice INTRLAY(alternative(0, name = "First")) = std::variant<bool, std::uint32_t>;
using Alias INTRLAY(alternative(0, name = "Other")) = Choice;
#elif defined(INTRLAY_COLLIDING_NAMES)
using Choice INTRLAY(alternative(0, name = "Same"), alternative(1, name = "Same")) = std::variant<bool, std::uint32_t>;
#else
struct Node;
using Branch INTRLAY() = std::variant<std::monostate, std::unique_ptr<Node>>;
struct Node { Branch next; };
#endif
