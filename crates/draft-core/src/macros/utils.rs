use crate::object::{MapProps, Object};

struct MacroInstance {
    decorations: Vec<String>,
    config: Object,
    bodies: Vec<String>,
}

struct MacroSchema {
    body_count: Option<u8>, // none for variable
    decoration_pool: &'static [&'static str],
    config_schema: MapProps,
}
