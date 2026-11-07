#include <intrlay/annotations.hpp>
#include <array>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <variant>
#include <vector>

namespace fixture {
/// A point in world space.
///
///     Preserve example indentation.
struct alignas(16) INTRLAY(project = "f32") Point {
    float x; ///< Position along X.
    float y;
    float z;
};

struct Node {
    std::string name;
    std::optional<std::unique_ptr<Node>> next;
};

/// Operating mode.
enum class INTRLAY() Mode : std::uint8_t {
    /// Device is disabled.
    Off,
    /// Device is enabled.
    On = 9
};

template<class T> struct Generic { T value; std::optional<T> extra; };
using Concrete INTRLAY() = Generic<std::uint64_t>;
static_assert(sizeof(Concrete) > 0);

/** Drawing frame.
 *
 *     Keep this indentation too.
 */
struct INTRLAY() Frame {
    std::uint32_t sequence;
    std::string label;
    std::vector<Point> points;
    // Unspecified, RGBA, or grayscale.
    std::variant<std::monostate, std::array<float, 4>, double> color;
    std::unique_ptr<Node> tree;
    Mode mode;
};
}
