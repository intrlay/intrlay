#pragma once
#include "../adapter.hpp"
#include <glm/glm.hpp>
#include <glm/gtc/quaternion.hpp>
#include <glm/gtc/vec1.hpp>
#include <array>
#include <cstdint>
#include <ranges>

#if defined(_MSC_VER)
#pragma warning(push)
// Aligned schema mirrors intentionally include trailing padding.
#pragma warning(disable: 4324)
#endif

namespace intrlay::detail {

// Concrete schema names distinguish scalar types and alignment without exporting
// every precision qualifier as another record. GLM aliases resolve to these same
// schemas. Aligned vectors retain padding; it is initialized by the writer.
template<glm::length_t L, class T, bool Aligned>
struct GlmVectorSchema;

// GLM's int64 aliases may use long long where std::int64_t is long. Resolve
// integral spellings by width/sign, just like the scalar codec's source concept.
template<class T>
struct GlmScalar { using type = T; };

template<std::integral T>
    requires (!std::same_as<T, bool> && sizeof(T) <= 8)
struct GlmScalar<T> {
    using Unsigned = std::conditional_t<sizeof(T) == 1, std::uint8_t,
        std::conditional_t<sizeof(T) == 2, std::uint16_t,
        std::conditional_t<sizeof(T) == 4, std::uint32_t, std::uint64_t>>>;
    using type = std::conditional_t<std::is_signed_v<T>, std::make_signed_t<Unsigned>, Unsigned>;
};

#define INTRLAY_DETAIL_GLM_VECTOR(Name, Scalar, Length, Qualifier, Aligned, ...) \
    struct alignas(alignof(glm::vec<Length, Scalar, Qualifier>)) Name { __VA_ARGS__ }; \
    template<> struct GlmVectorSchema<Length, Scalar, Aligned> { using type = Name; }

#define INTRLAY_DETAIL_GLM_VECTORS(Prefix, Scalar, Qualifier, Aligned) \
    INTRLAY_DETAIL_GLM_VECTOR(Prefix##1, Scalar, 1, Qualifier, Aligned, Scalar x;); \
    INTRLAY_DETAIL_GLM_VECTOR(Prefix##2, Scalar, 2, Qualifier, Aligned, Scalar x; Scalar y;); \
    INTRLAY_DETAIL_GLM_VECTOR(Prefix##3, Scalar, 3, Qualifier, Aligned, Scalar x; Scalar y; Scalar z;); \
    INTRLAY_DETAIL_GLM_VECTOR(Prefix##4, Scalar, 4, Qualifier, Aligned, Scalar x; Scalar y; Scalar z; Scalar w;)

#define INTRLAY_DETAIL_GLM_SCALARS(Prefix, Qualifier, Aligned) \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##Vec, float, Qualifier, Aligned); \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##DVec, double, Qualifier, Aligned); \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##BVec, bool, Qualifier, Aligned); \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##I8Vec, std::int8_t, Qualifier, Aligned); \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##U8Vec, std::uint8_t, Qualifier, Aligned); \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##I16Vec, std::int16_t, Qualifier, Aligned); \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##U16Vec, std::uint16_t, Qualifier, Aligned); \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##IVec, std::int32_t, Qualifier, Aligned); \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##UVec, std::uint32_t, Qualifier, Aligned); \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##I64Vec, std::int64_t, Qualifier, Aligned); \
    INTRLAY_DETAIL_GLM_VECTORS(Prefix##U64Vec, std::uint64_t, Qualifier, Aligned)

INTRLAY_DETAIL_GLM_SCALARS(Glm, glm::packed_highp, false);
#if GLM_CONFIG_ALIGNED_GENTYPES == GLM_ENABLE
INTRLAY_DETAIL_GLM_SCALARS(GlmAligned, glm::aligned_highp, true);
#endif

#undef INTRLAY_DETAIL_GLM_SCALARS
#undef INTRLAY_DETAIL_GLM_VECTORS
#undef INTRLAY_DETAIL_GLM_VECTOR

template<glm::qualifier Q>
inline constexpr bool glm_aligned =
#if GLM_CONFIG_ALIGNED_GENTYPES == GLM_ENABLE
    Q == glm::aligned_highp || Q == glm::aligned_mediump || Q == glm::aligned_lowp;
#else
    false;
#endif

template<class T, bool Aligned>
struct GlmQuaternionSchema;

#define INTRLAY_DETAIL_GLM_QUAT(Name, Scalar, Qualifier, Aligned) \
    struct alignas(alignof(glm::qua<Scalar, Qualifier>)) Name { Scalar x; Scalar y; Scalar z; Scalar w; }; \
    template<> struct GlmQuaternionSchema<Scalar, Aligned> { using type = Name; }

INTRLAY_DETAIL_GLM_QUAT(GlmQuat, float, glm::packed_highp, false);
INTRLAY_DETAIL_GLM_QUAT(GlmDQuat, double, glm::packed_highp, false);
#if GLM_CONFIG_ALIGNED_GENTYPES == GLM_ENABLE
INTRLAY_DETAIL_GLM_QUAT(GlmAlignedQuat, float, glm::aligned_highp, true);
INTRLAY_DETAIL_GLM_QUAT(GlmAlignedDQuat, double, glm::aligned_highp, true);
#endif
#undef INTRLAY_DETAIL_GLM_QUAT

} // namespace intrlay::detail

#if defined(_MSC_VER)
#pragma warning(pop)
#endif

namespace intrlay {

/// GLM vectors expose named components, including Boolean and fixed-width integer
/// families. Precision qualifiers share a schema; packed/aligned layouts differ.
template<glm::length_t L, class T, glm::qualifier Q>
    requires requires { typename detail::GlmVectorSchema<L, typename detail::GlmScalar<T>::type, detail::glm_aligned<Q>>::type; }
struct Adapter<glm::vec<L, T, Q>> {
    using schema_type = typename detail::GlmVectorSchema<L, typename detail::GlmScalar<T>::type, detail::glm_aligned<Q>>::type;
    static constexpr bool preserves_native_layout = true;
    static constexpr const glm::vec<L, T, Q>& source(const glm::vec<L, T, Q>& value) noexcept { return value; }
};

/// Matrices are fixed arrays of columns. The range borrows the original columns;
/// measurement/writing never allocate or copy a temporary matrix.
template<glm::length_t C, glm::length_t R, class T, glm::qualifier Q>
    requires (C >= 2 && C <= 4 && R >= 2 && R <= 4) && (std::same_as<T, float> || std::same_as<T, double>)
struct Adapter<glm::mat<C, R, T, Q>> {
    using schema_type = std::array<glm::vec<R, T, Q>, C>;
    static constexpr auto source(const glm::mat<C, R, T, Q>& value) noexcept {
        return std::views::iota(glm::length_t{0}, C)
            | std::views::transform([&value](glm::length_t column) -> const auto& { return value[column]; });
    }
};

/// Quaternions always encode x, y, z, w, independently of GLM's native storage
/// order. Native-copy proofs reject mismatched offsets (including WXYZ builds).
template<class T, glm::qualifier Q>
    requires (std::same_as<T, float> || std::same_as<T, double>)
struct Adapter<glm::qua<T, Q>> {
    using schema_type = typename detail::GlmQuaternionSchema<T, detail::glm_aligned<Q>>::type;
    static constexpr bool preserves_native_layout = true;
    static constexpr const glm::qua<T, Q>& source(const glm::qua<T, Q>& value) noexcept { return value; }
};

} // namespace intrlay
