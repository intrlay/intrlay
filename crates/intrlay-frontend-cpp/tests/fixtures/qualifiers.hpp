#include <intrlay/annotations.hpp>
#include <array>
#include <cstdint>
#include <memory>
#include <variant>

namespace qualifier_fixture {
struct Child { std::uint32_t value; };
enum class Mode { Off, On };

template<class T> struct Holder { T value; };
using PlainHolder INTRLAY() = Holder<Child>;
using ConstHolder INTRLAY() = Holder<const Child>;
static_assert(sizeof(PlainHolder) > 0 && sizeof(ConstHolder) > 0);

using Choice = std::variant<std::monostate, Child>;
struct INTRLAY() Qualified {
    Child plain_child;
    const Child const_child;
    volatile Child volatile_child;
    const volatile Child qualified_child;
    Mode plain_mode;
    const Mode const_mode;
    volatile Mode volatile_mode;
    const volatile Mode qualified_mode;
    Choice plain_choice;
    const Choice const_choice;
    std::unique_ptr<const Child> boxed;
    std::array<const Child, 2> fixed;
    PlainHolder plain_holder;
    const PlainHolder const_holder;
    ConstHolder qualified_argument;
};

struct INTRLAY() Node {
    std::unique_ptr<const Node> next;
};
}
