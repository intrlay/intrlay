#include "bindings.hpp"

auto child_value_reader_from_peer()
    -> decltype(&cpp_metadata::ChildView::Metadata::Field<0>::read<const std::byte>) {
    return &cpp_metadata::ChildView::Metadata::Field<0>::read<const std::byte>;
}
