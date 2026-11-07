#pragma once

// Schema annotations are consumed only by the build-time Clang frontend.
// Generated code and application builds require C++20, with no Clang runtime.
#if defined(__clang__)
#define INTRLAY(...) [[clang::annotate("intrlay:" #__VA_ARGS__)]]
#else
#define INTRLAY(...)
#endif
