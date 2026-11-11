#pragma once
#include <intrlay/annotations.hpp>
#include <array>
#include <cstdint>
#include <memory>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <variant>
#include <vector>

namespace external {
struct Vector3 { float x, y, z; };
struct Empty {};

class Label {
    std::string text_;
public:
    explicit Label(std::string text = {}) : text_(std::move(text)) {}
    std::string_view text() const { return text_; }
};

template<class T>
class Buffer {
    std::vector<T> values_;
public:
    Buffer(std::initializer_list<T> values) : values_(values) {}
    std::span<const T> values() const { return values_; }
};

class Pose {
    Vector3 position_;
    Label label_;
public:
    Pose(Vector3 position, Label label) : position_(position), label_(std::move(label)) {}
    const Vector3& position() const { return position_; }
    const Label& label() const { return label_; }
};

// Returning the same type still performs a conversion, not a native-copy proof.
struct Shifted { float value; };
struct Proxy { float value; };
struct Wrapped { float value; };
struct Tree { std::uint32_t value; std::vector<Tree> children; };
struct Endpoints { std::variant<Vector3, Vector3> value; };
struct LocalInteger { std::int32_t value; };
struct LocalFloat { float value; };
}

namespace local_schema {
// Block-local namesakes can share both a display name and a Clang USR.
inline auto records() {
    struct INTRLAY(name="LocalInteger") Value { std::int32_t value; };
    using Integer = Value;
    {
        struct INTRLAY(name="LocalFloat") Value { float value; };
        struct Pair { Integer integer; Value floating; };
        return Pair{};
    }
}
inline auto integer() { return records().integer; }
inline auto floating() { return records().floating; }
}

namespace structural_schema {
struct Extent { std::size_t count; };
template<Extent E> struct Samples { std::array<float, E.count> values; };

// Concrete aliases preserve structural specializations and their identity.
template<> struct Samples<Extent{3}> {
    std::array<float, 3> values;
};
using SamplesThree INTRLAY() = Samples<Extent{3}>;
using SamplesOne INTRLAY() = Samples<Extent{1}>;
using SamplesOneAlias INTRLAY() = Samples<Extent{1 + 0}>;
using ConstSamplesOne INTRLAY() = const Samples<Extent{1}>;
using SamplesTwo INTRLAY() = Samples<Extent{2}>;
using SampleChoices INTRLAY(alternative(0,name="Single"), alternative(1,name="Pair")) =
    std::variant<SamplesOne, SamplesTwo>;
}

namespace alias_schema {
struct Model { struct Payload; enum class Kind; };
struct Model::Payload { std::uint32_t value; };
enum class Model::Kind { First = 1, Last = 3 };
using NestedPayload INTRLAY() = Model::Payload;
using NestedKind INTRLAY() = Model::Kind;
}

template<> struct intrlay::Adapter<external::LocalInteger> {
    using schema_type = decltype(local_schema::integer());
    static const external::LocalInteger& source(const external::LocalInteger& value) { return value; }
};
template<> struct intrlay::Adapter<external::LocalFloat> {
    using schema_type = decltype(local_schema::floating());
    static const external::LocalFloat& source(const external::LocalFloat& value) { return value; }
};

INTRLAY_ADAPT_STRUCT(external::Vector3, Position, x, y, z);
INTRLAY_ADAPT_STRUCT(external::Empty, Empty);
INTRLAY_ADAPT_STRUCT(external::Tree, Tree, value, children);

template<> struct intrlay::Adapter<external::Label> {
    using schema_type = std::string;
    static std::string_view source(const external::Label& value) { return value.text(); }
};

template<class T> struct intrlay::Adapter<external::Buffer<T>> {
    using schema_type = std::vector<T>;
    static std::span<const T> source(const external::Buffer<T>& value) { return value.values(); }
};

template<> struct intrlay::Adapter<external::Pose> {
    struct Pose {
        /// Position in world units.
        external::Vector3 position;
        external::Label label;
    };
    using schema_type = Pose;
    struct Source { const external::Vector3& position; const external::Label& label; };
    static Source source(const external::Pose& value) { return {value.position(), value.label()}; }
};

template<> struct intrlay::Adapter<external::Shifted> {
    struct Shifted { float value; };
    using schema_type = Shifted;
    static external::Shifted source(const external::Shifted& value) { return {value.value + 1}; }
};

template<> struct intrlay::Adapter<external::Proxy> {
    using schema_type = float;
    static const float& source(const external::Proxy& value) { return value.value; }
};

template<> struct intrlay::Adapter<external::Wrapped> {
    using schema_type = float;
    static external::Proxy source(const external::Wrapped& value) { return {value.value}; }
};

template<> struct intrlay::Adapter<external::Endpoints> {
    using schema_type = std::variant<external::Vector3, external::Vector3>;
    static const schema_type& source(const external::Endpoints& value) { return value.value; }
};

namespace adapter_fixture {
struct INTRLAY() Message {
    external::Pose pose;
    INTRLAY(project = "f32") external::Buffer<external::Vector3> positions;
    std::array<std::optional<external::Vector3>, 2> optional_positions;
    std::unique_ptr<external::Vector3> boxed_position;
    std::variant<external::Vector3, external::Label, external::Shifted> choice;
    std::vector<external::Shifted> shifted;
    external::Wrapped wrapped;
    external::Tree tree;
    external::Empty empty;
};
struct INTRLAY(project = "flatten") FlattenedBounds {
    external::Vector3 min;
    std::array<float, 3> max;
};
using Scalar INTRLAY() = external::Wrapped;
using Endpoints INTRLAY(alternative(0, name = "Start"), alternative(1, name = "End")) = external::Endpoints;
using LocalInteger INTRLAY() = external::LocalInteger;
using IntegerAlias INTRLAY() = external::LocalInteger;
using LocalFloat INTRLAY() = external::LocalFloat;
using LocalIntegerChoices INTRLAY(alternative(0, name="FirstInteger"), alternative(1, name="SecondInteger")) =
    std::variant<decltype(local_schema::integer()), decltype(local_schema::integer())>;
using LocalFloatChoices INTRLAY(alternative(0, name="FirstFloat"), alternative(1, name="SecondFloat")) =
    std::variant<decltype(local_schema::floating()), decltype(local_schema::floating())>;
}
