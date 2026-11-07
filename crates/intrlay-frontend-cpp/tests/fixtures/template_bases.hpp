#include <intrlay/annotations.hpp>

struct Base { int inherited; };

#if INTRLAY_CASE >= 13
template<class T> struct Outer {
    template<class U> struct Inner : Base { U value; };
};
#if INTRLAY_CASE == 13
template<> template<class U> struct Outer<int>::Inner<U*> : Base { U value; };
#elif INTRLAY_CASE == 14
template<> template<class U> struct Outer<int>::Inner<U*> { U value; };
#elif INTRLAY_CASE == 15
template<> template<class U> struct Outer<int>::Inner : Base { U value; };
#elif INTRLAY_CASE == 16
template<> template<> struct Outer<int>::Inner<int*> { int value; };
#elif INTRLAY_CASE == 17
template<> template<> struct Outer<int>::Inner<int*> : Base { int value; };
#endif
using Root INTRLAY() = Outer<int>::Inner<int*>;
#elif INTRLAY_CASE == 1
template<class T> struct Item : T { int value; };
using Root INTRLAY() = Item<Base>;
#elif INTRLAY_CASE == 2 || INTRLAY_CASE == 6
template<class T> struct Item : Base { T value; };
#if INTRLAY_CASE == 2
template<class T> struct Item<T*> : Base { T value; };
#else
template<class T> struct Item<T*> { T value; };
#endif
using Root INTRLAY() = Item<int*>;
#elif INTRLAY_CASE == 3
template<class T> struct Item : Base {};
using Root INTRLAY() = Item<int>;
#elif INTRLAY_CASE == 4 || INTRLAY_CASE == 5 || INTRLAY_CASE == 7 || INTRLAY_CASE == 10 || INTRLAY_CASE == 12
template<class T> struct Item : Base { T value; };
#if INTRLAY_CASE == 4
template<> struct Item<int> : Base { int value; };
#elif INTRLAY_CASE == 5
template<> struct Item<int> { int value; };
#elif INTRLAY_CASE == 10
template /* specialization */ < /* empty parameters */ > struct Item<int> { int value; };
#elif INTRLAY_CASE == 12
#define SPECIALIZE(T) template<> struct Item<T> { T value; };
SPECIALIZE(int)
#else
template<> struct Item<int> {};
#endif
using Root INTRLAY() = Item<int>;
#else
template<class T> struct Item : Base { T value; };
#if INTRLAY_CASE == 8
template struct Item<int>;
#elif INTRLAY_CASE == 9
extern template struct Item<int>;
#elif INTRLAY_CASE == 11
#define INSTANTIATE(T) template struct Item<T>;
INSTANTIATE(int)
#endif
using Root INTRLAY() = Item<int>;
#endif

static_assert(sizeof(Root));
