from bindings import WrappedScalarsView, ProjectionSetView, CachedChildrenView

def invalid(wrapped: WrappedScalarsView, projection: ProjectionSetView, cached: CachedChildrenView) -> None:
    wrapped.read_only = 1  # EXPECT: misc
    print(wrapped.write_only)  # EXPECT: attr-defined
    if cached.nested is not None:
        cached.nested.value = None  # EXPECT: misc
