#include <intrlay/annotations.hpp>

struct INTRLAY() Root;
struct Root { int value; };
struct Root;

enum class INTRLAY() Mode : unsigned;
enum class Mode : unsigned { Off = 0, On = 1 };

using Alias INTRLAY() = Root;
using Alias = Root;

struct Defined;
struct INTRLAY() Defined { int value; };

#if defined(INTRLAY_CONFLICT_SAME_TYPE)
using First INTRLAY(name="Shared") = Root;
using Second INTRLAY(name="Shared") = Root;
#elif defined(INTRLAY_CONFLICT_DIFFERENT_TYPE)
using First INTRLAY(name="Shared") = Root;
using Second INTRLAY(name="Shared") = Defined;
#elif defined(INTRLAY_CONFLICT_NAMESPACE)
namespace first { struct INTRLAY() Shared { int value; }; }
namespace second { struct INTRLAY() Shared { int value; }; }
#elif defined(INTRLAY_CONFLICT_ANNOTATION)
struct INTRLAY(align=8) Changed;
struct INTRLAY(align=16) Changed { int value; };
#endif
