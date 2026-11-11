#pragma once
#include "glm.hpp"
#include <glm/gtx/dual_quaternion.hpp>

// This optional header follows GLM's opt-in: define GLM_ENABLE_EXPERIMENTAL in
// the application before including it. The core adapter never enables extensions.
namespace intrlay::detail {

template<class T, bool Aligned>
struct GlmDualQuaternionSchema;

#define INTRLAY_DETAIL_GLM_DUAL_QUAT(Name, Scalar, Qualifier, Aligned) \
    struct alignas(alignof(glm::tdualquat<Scalar, Qualifier>)) Name { \
        glm::qua<Scalar, Qualifier> real; \
        glm::qua<Scalar, Qualifier> dual; \
    }; \
    template<> struct GlmDualQuaternionSchema<Scalar, Aligned> { using type = Name; }

INTRLAY_DETAIL_GLM_DUAL_QUAT(GlmDualQuat, float, glm::packed_highp, false);
INTRLAY_DETAIL_GLM_DUAL_QUAT(GlmDDualQuat, double, glm::packed_highp, false);
#if GLM_CONFIG_ALIGNED_GENTYPES == GLM_ENABLE
INTRLAY_DETAIL_GLM_DUAL_QUAT(GlmAlignedDualQuat, float, glm::aligned_highp, true);
INTRLAY_DETAIL_GLM_DUAL_QUAT(GlmAlignedDDualQuat, double, glm::aligned_highp, true);
#endif
#undef INTRLAY_DETAIL_GLM_DUAL_QUAT

} // namespace intrlay::detail

namespace intrlay {

/// Dual quaternions encode the real and dual quaternion without normalization.
template<class T, glm::qualifier Q>
    requires (std::same_as<T, float> || std::same_as<T, double>)
struct Adapter<glm::tdualquat<T, Q>> {
    using schema_type = typename detail::GlmDualQuaternionSchema<T, detail::glm_aligned<Q>>::type;
    static constexpr bool preserves_native_layout = true;
    static constexpr const glm::tdualquat<T, Q>& source(const glm::tdualquat<T, Q>& value) noexcept { return value; }
};

} // namespace intrlay
