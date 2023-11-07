use std::fmt;

use crate::node::Node;
use crate::reflow::{Bond, Cage, Lotus, Revisable};
use crate::web::AttrValue;
use crate::ViewId;

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::{JsValue, UnwrapThrowExt};

#[derive(Debug, Default)]
pub struct Classes {
    parts: Vec<Lotus<String>>,
}
impl Classes {
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
        }
    }
    pub fn part(&mut self, part: impl Into<Lotus<String>>) -> &mut Self {
        self.parts.push(part.into());
        self
    }

    pub fn raw_parts(&self) -> Vec<String> {
        self.parts.iter().map(|part| part.get().to_string()).collect()
    }

    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    pub fn to_array(&self) -> js_sys::Array {
        FromIterator::from_iter(self.raw_parts().iter().map(|v| JsValue::from_str(v)))
    }
}

impl AttrValue for Classes {
    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        let is_revising = self.parts.iter().any(|part| part.is_revising());
        if is_revising || first_time {
            let value = AttrValue::to_string(self);
            if let Some(value) = value {
                node.set_attribute(name, &value).unwrap_throw();
            } else {
                node.remove_attribute(name).unwrap_throw();
            }
        }
        if first_time {
            for part in &self.parts {
                part.bind_view(view_id);
            }
        }
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
    fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        let is_revising = self.parts.iter().any(|part| part.is_revising());
        if is_revising || first_time {
            let value = AttrValue::to_string(self);
            if let Some(value) = value {
                node.set_attribute(name.to_owned(), value);
            } else {
                node.remove_attribute(name);
            }
        }
        if first_time {
            for part in &self.parts {
                part.bind_view(view_id);
            }
        }
    }
    fn to_string(&self) -> Option<String> {
        let value = self.raw_parts().join(" ");
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }
}