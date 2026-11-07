#include <intrlay/annotations.hpp>
#include <cstdint>
#include <string>
#include <variant>

namespace codec_fixture {
struct ValueRecord {
    std::uint32_t value;
    std::string label;
};

struct INTRLAY() CodecCases {
    float raw[3];
    std::variant<std::monostate, ValueRecord> choice;
};

struct CharacterTraits : std::char_traits<char> {};

struct INTRLAY() Text {
    std::basic_string<char, CharacterTraits> value;
};

struct INTRLAY() Repeated {
    std::variant<ValueRecord, ValueRecord> choice;
};

// Names are resolved even when the alias follows an earlier use of the type.
using Endpoints INTRLAY(
    alternative(0, name = "Start"),
    alternative(1, name = "End")
) = std::variant<ValueRecord, ValueRecord>;
}
