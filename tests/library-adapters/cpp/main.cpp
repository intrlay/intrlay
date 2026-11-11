#ifdef NDEBUG
#undef NDEBUG
#endif
#include "schema.hpp"
#include "bindings.hpp"
#include <cassert>

namespace wire = intrlay_generated::glm_adapters;

template<class Vector>
void fill_vector(Vector& value) {
    for (glm::length_t i = 0; i < value.length(); ++i) {
        if constexpr (std::same_as<typename Vector::value_type, bool>) value[i] = (i % 2 == 0);
        else value[i] = static_cast<typename Vector::value_type>(i + 1);
    }
}

template<class View, class Vector>
void check_vector(const View& view, const Vector& value) {
    assert(view.x() == value[0]);
    if constexpr (requires { value.y; }) assert(view.y() == value[1]);
    if constexpr (requires { value.z; }) assert(view.z() == value[2]);
    if constexpr (requires { value.w; }) assert(view.w() == value[3]);
}

template<class View, class Native>
void vectors() {
    Native value;
#define FILL(Prefix) fill_vector(value.Prefix##1); fill_vector(value.Prefix##2); fill_vector(value.Prefix##3); fill_vector(value.Prefix##4);
    FILL(f32_) FILL(f64_) FILL(bool_) FILL(i8_) FILL(u8_) FILL(i16_)
    FILL(u16_) FILL(i32_) FILL(u32_) FILL(i64_) FILL(u64_)
#undef FILL
    // Test integer ranges that cannot survive conversion through a floating point value.
    value.i64_3.x = std::numeric_limits<glm::i64>::min();
    value.u64_4.w = std::numeric_limits<glm::u64>::max();
    auto encoded = View::create(value);
    auto checked = View::from_bytes(encoded.bytes());
    assert(checked);
    auto& view = *checked;
#define CHECK(Prefix) check_vector(view.Prefix##1(), value.Prefix##1); check_vector(view.Prefix##2(), value.Prefix##2); \
    check_vector(view.Prefix##3(), value.Prefix##3); check_vector(view.Prefix##4(), value.Prefix##4);
    CHECK(f32_) CHECK(f64_) CHECK(bool_) CHECK(i8_) CHECK(u8_) CHECK(i16_)
    CHECK(u16_) CHECK(i32_) CHECK(u32_) CHECK(i64_) CHECK(u64_)
#undef CHECK
}

template<class Matrix>
void fill_matrix(Matrix& value) {
    for (glm::length_t column = 0; column < Matrix::length(); ++column) {
        for (glm::length_t row = 0; row < Matrix::col_type::length(); ++row) {
            value[column][row] = static_cast<typename Matrix::value_type>(column * 4 + row + 1);
        }
    }
}

template<class View, class Matrix>
void check_matrix(const View& view, const Matrix& value) {
    assert(view.size() == static_cast<std::size_t>(Matrix::length()));
    for (glm::length_t column = 0; column < Matrix::length(); ++column) check_vector(view.get(column), value[column]);
}

template<class View, class Native>
void matrices() {
    Native value;
#define EACH(Operation, Prefix) Operation(Prefix##2x2) Operation(Prefix##2x3) Operation(Prefix##2x4) \
    Operation(Prefix##3x2) Operation(Prefix##3x3) Operation(Prefix##3x4) \
    Operation(Prefix##4x2) Operation(Prefix##4x3) Operation(Prefix##4x4)
#define FILL(Field) fill_matrix(value.Field);
    EACH(FILL, f32_) EACH(FILL, f64_)
#undef FILL
    auto encoded = View::create(value);
    auto checked = View::from_bytes(encoded.bytes());
    assert(checked);
#define CHECK(Field) check_matrix(checked->Field(), value.Field);
    EACH(CHECK, f32_) EACH(CHECK, f64_)
#undef CHECK
#undef EACH
}

int main() {
    vectors<wire::VectorsView, Vectors>();
    matrices<wire::MatricesView, Matrices>();
#if GLM_CONFIG_ALIGNED_GENTYPES == GLM_ENABLE
    vectors<wire::AlignedVectorsView, AlignedVectors>();
    matrices<wire::AlignedMatricesView, AlignedMatrices>();
    static_assert(!intrlay::detail::native_layout<wire::GlmAlignedVec3View::Metadata, glm::vec<3, float, glm::aligned_highp>>);
    auto aligned = wire::GlmAlignedVec3View::create(glm::vec<3, float, glm::aligned_highp>(1, 2, 3));
    assert(aligned.bytes().size() == 16);
    for (std::size_t i = 12; i < 16; ++i) assert(aligned.bytes()[i] == std::byte{});
#endif
    static_assert(intrlay::detail::native_layout<wire::GlmVec3View::Metadata, glm::vec<3, float, glm::packed_highp>>);
    static_assert(intrlay::detail::native_layout<wire::GlmQuatView::Metadata, glm::quat>
#ifdef GLM_FORCE_QUAT_DATA_WXYZ
        == false
#endif
    );

    Quaternions value{};
    value.rotation = glm::quat(4, 1, 2, 3); // GLM's constructor takes w first.
    value.precise = glm::dquat(8, 5, 6, 7);
    value.dual = glm::dualquat(value.rotation, glm::quat(12, 9, 10, 11));
    value.precise_dual = glm::ddualquat(value.precise, glm::dquat(16, 13, 14, 15));
    auto rotations = wire::QuaternionsView::create(value);
    auto rotation = rotations.view_unchecked();
    assert(rotation.rotation().x() == 1 && rotation.rotation().w() == 4);
    assert(rotation.precise().x() == 5 && rotation.precise().w() == 8);
    assert(rotation.dual().real().y() == 2 && rotation.dual().dual().z() == 11);
    assert(rotation.precise_dual().dual().w() == 16);
#if GLM_CONFIG_ALIGNED_GENTYPES == GLM_ENABLE
    AlignedQuaternions aligned_value{value.rotation, value.precise, value.dual, value.precise_dual};
    auto aligned_rotations = wire::AlignedQuaternionsView::create(aligned_value);
    auto aligned_rotation = aligned_rotations.view_unchecked();
    assert(aligned_rotation.aligned().x() == 1 && aligned_rotation.aligned_precise().w() == 8);
    assert(aligned_rotation.aligned_dual().dual().z() == 11 && aligned_rotation.aligned_precise_dual().dual().w() == 16);
#endif
    Nested nested{{glm::mat3(1), glm::mat3(2)}, value.rotation, {{3, 4, 5}, {6, 7, 8}}};
    auto data = wire::NestedView::create(nested);
    assert(data.view_unchecked().transforms().get(1).get(2).z() == 2);
    assert(data.view_unchecked().rotation()->x() == 1);
    assert(data.view_unchecked().positions().get(1).y() == 7);
    assert(wire::GlmVec3View::create(glm::vec<3, float, glm::packed_lowp>(9, 10, 11)).view_unchecked().z() == 11);
    assert(wire::GlmVec3View::create(glm::vec<3, float, glm::packed_mediump>(9, 10, 11)).view_unchecked().y() == 10);
}
