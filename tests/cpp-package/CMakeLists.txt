cmake_minimum_required(VERSION 3.20)
project(intrlay_package_test LANGUAGES CXX)

find_package(intrlay CONFIG REQUIRED)
add_executable(intrlay_package_test main.cpp)
target_link_libraries(intrlay_package_test PRIVATE intrlay::intrlay)
enable_testing()
add_test(NAME installed_runtime COMMAND intrlay_package_test)
