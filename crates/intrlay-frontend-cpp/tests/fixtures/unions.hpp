#include <intrlay/annotations.hpp>
#include <cstdint>

namespace union_fixture {
#if defined(INTRLAY_DIRECT_UNION)
union INTRLAY() Value {
#else
union Value {
#endif
    std::uint32_t integer;
    float floating;
};

#if defined(INTRLAY_UNION_ALIAS)
using Export INTRLAY() = Value;
#elif !defined(INTRLAY_DIRECT_UNION)
struct INTRLAY() Container { Value value; };
#endif
}
