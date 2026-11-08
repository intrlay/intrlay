use crate::{
    api::{Accessor, AccessorContext, ApiPolicy},
    plan::TypeGraph,
};

pub(super) struct Api;

impl ApiPolicy for Api {
    fn customize_accessor(
        &self,
        plan: &TypeGraph,
        accessor: &mut Accessor,
        context: AccessorContext<'_>,
    ) {
        if let Some(cache) = &accessor.cache {
            accessor.loader = super::cached_loader_name(
                plan,
                context.ty,
                &accessor.name,
                context.exposure,
                cache,
            );
        }
    }
}
