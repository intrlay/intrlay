//! Cached views retain their identity while discriminants are read from storage.

use super::{Id, Kind, TypeGraph};

pub struct ViewCache {
    pub value: String,
    pub state: Option<String>,
}

impl TypeGraph {
    pub fn view_cache(&self, id: Id, name: &str, tagged: bool) -> ViewCache {
        let field = |suffix: &str| {
            let name = format!("cached_{name}{suffix}");
            self.backend.api.cache_field(&self.names, &name)
        };
        ViewCache {
            value: field(""),
            state: (tagged || self.mutable_presence(id)).then(|| field("_state")),
        }
    }

    fn mutable_presence(&self, id: Id) -> bool {
        if let Some(field) = self.folded_field(id) {
            return self.mutable_presence(field.ty);
        }
        match self.nodes[id].kind {
            Kind::Optional { value, .. } => !self.has_payload(id) || self.mutable_presence(value),
            Kind::Boxed(value) => self.mutable_presence(value),
            _ => false,
        }
    }
}
