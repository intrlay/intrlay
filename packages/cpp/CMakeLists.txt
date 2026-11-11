cmake_minimum_required(VERSION 3.20)
project(intrlay VERSION 0.1.4
    HOMEPAGE_URL https://github.com/soulart/intrlay
    LANGUAGES CXX)

include(GNUInstallDirs)

add_library(intrlay INTERFACE)
add_library(intrlay::intrlay ALIAS intrlay)
target_compile_features(intrlay INTERFACE cxx_std_20)
target_compile_options(intrlay INTERFACE $<$<CXX_COMPILER_ID:MSVC>:/Zc:preprocessor>)
target_include_directories(intrlay INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>)

include(CMakePackageConfigHelpers)
install(TARGETS intrlay EXPORT intrlayTargets)
install(DIRECTORY include/ DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
install(FILES LICENSE LICENSE-APACHE LICENSE-MIT
    DESTINATION ${CMAKE_INSTALL_DATADIR}/licenses/intrlay OPTIONAL)
install(EXPORT intrlayTargets NAMESPACE intrlay:: DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/intrlay)
configure_package_config_file(cmake/intrlayConfig.cmake.in
    ${CMAKE_CURRENT_BINARY_DIR}/intrlayConfig.cmake
    INSTALL_DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/intrlay)
write_basic_package_version_file(${CMAKE_CURRENT_BINARY_DIR}/intrlayConfigVersion.cmake
    VERSION ${PROJECT_VERSION} COMPATIBILITY SameMajorVersion)
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/intrlayConfig.cmake
    ${CMAKE_CURRENT_BINARY_DIR}/intrlayConfigVersion.cmake
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/intrlay)
