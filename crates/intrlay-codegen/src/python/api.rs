use crate::{api::ApiPolicy, names::Names};

pub(super) struct Api;

impl ApiPolicy for Api {
    fn cache_field(&self, _names: &Names, name: &str) -> String {
        // A single underscore avoids class-name mangling of private caches.
        format!("_{name}")
    }
}
