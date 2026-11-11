#pragma once
#define GLM_ENABLE_EXPERIMENTAL
#include <intrlay/annotations.hpp>
#include <intrlay/adapters/glm_dual_quaternion.hpp>
#include <glm/gtc/type_precision.hpp>
#include <optional>
#include <vector>

#define INTRLAY_TEST_GLM_VECTORS(Scalar, Prefix, Qualifier) \
    glm::vec<1, Scalar, Qualifier> Prefix##1; \
    glm::vec<2, Scalar, Qualifier> Prefix##2; \
    glm::vec<3, Scalar, Qualifier> Prefix##3; \
    glm::vec<4, Scalar, Qualifier> Prefix##4;

#define INTRLAY_TEST_GLM_FAMILIES(Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(float, f32_, Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(double, f64_, Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(bool, bool_, Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(glm::i8, i8_, Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(glm::u8, u8_, Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(glm::i16, i16_, Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(glm::u16, u16_, Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(glm::i32, i32_, Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(glm::u32, u32_, Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(glm::i64, i64_, Qualifier) \
    INTRLAY_TEST_GLM_VECTORS(glm::u64, u64_, Qualifier)

struct INTRLAY() Vectors { INTRLAY_TEST_GLM_FAMILIES(glm::packed_highp) };
#if GLM_CONFIG_ALIGNED_GENTYPES == GLM_ENABLE
struct INTRLAY() AlignedVectors { INTRLAY_TEST_GLM_FAMILIES(glm::aligned_highp) };
#endif
#undef INTRLAY_TEST_GLM_FAMILIES
#undef INTRLAY_TEST_GLM_VECTORS

#define INTRLAY_TEST_GLM_MATRICES(Scalar, Prefix, Qualifier) \
    glm::mat<2, 2, Scalar, Qualifier> Prefix##2x2; \
    glm::mat<2, 3, Scalar, Qualifier> Prefix##2x3; \
    glm::mat<2, 4, Scalar, Qualifier> Prefix##2x4; \
    glm::mat<3, 2, Scalar, Qualifier> Prefix##3x2; \
    glm::mat<3, 3, Scalar, Qualifier> Prefix##3x3; \
    glm::mat<3, 4, Scalar, Qualifier> Prefix##3x4; \
    glm::mat<4, 2, Scalar, Qualifier> Prefix##4x2; \
    glm::mat<4, 3, Scalar, Qualifier> Prefix##4x3; \
    glm::mat<4, 4, Scalar, Qualifier> Prefix##4x4;

struct INTRLAY() Matrices {
    INTRLAY_TEST_GLM_MATRICES(float, f32_, glm::packed_highp)
    INTRLAY_TEST_GLM_MATRICES(double, f64_, glm::packed_highp)
};
#if GLM_CONFIG_ALIGNED_GENTYPES == GLM_ENABLE
struct INTRLAY() AlignedMatrices {
    INTRLAY_TEST_GLM_MATRICES(float, f32_, glm::aligned_highp)
    INTRLAY_TEST_GLM_MATRICES(double, f64_, glm::aligned_highp)
};
#endif
#undef INTRLAY_TEST_GLM_MATRICES

struct INTRLAY() Quaternions {
    glm::quat rotation;
    glm::dquat precise;
    glm::dualquat dual;
    glm::ddualquat precise_dual;
};
#if GLM_CONFIG_ALIGNED_GENTYPES == GLM_ENABLE
struct INTRLAY() AlignedQuaternions {
    glm::qua<float, glm::aligned_highp> aligned;
    glm::qua<double, glm::aligned_highp> aligned_precise;
    glm::tdualquat<float, glm::aligned_highp> aligned_dual;
    glm::tdualquat<double, glm::aligned_highp> aligned_precise_dual;
};
#endif

struct INTRLAY() Nested {
    std::vector<glm::mat3> transforms;
    std::optional<glm::quat> rotation;
    std::vector<glm::vec3> positions INTRLAY(project="f32");
};
