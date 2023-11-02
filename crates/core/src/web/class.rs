use std::fmt;

use crate::node::Node;
use crate::reflow::{Bond, Cage, Record, Revisable};
use crate::web::AttrValue;
use crate::ViewId;

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::{JsValue, UnwrapThrowExt};

#[derive(Debug, Default)]
pub struct Classes {
    parts: Vec<Box<dyn ClassPart>>,
}
impl Classes {
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
        }
    }
    pub fn part(&mut self, part: impl ClassPart + 'static) -> &mut Self {
        self.parts.push(Box::new(part));
        self
    }

    pub fn raw_parts(&self) -> Vec<String> {
        self.parts.iter().filter_map(|part| part.to_string()).collect()
    }

    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    pub fn to_array(&self) -> js_sys::Array {
        FromIterator::from_iter(self.raw_parts().iter().map(|v| JsValue::from_str(v)))
    }
}

pub trait ClassPart: fmt::Debug {
    fn bind_view(&self, _view_id: &ViewId) {}
    fn is_revising(&self) -> bool {
        false
    }
    fn to_string(&self) -> Option<String>;
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

impl<T> ClassPart for Cage<T>
where
    T: ClassPart + fmt::Debug + Clone + 'static,
{
    fn bind_view(&self, view_id: &ViewId) {
        Revisable::bind_view(self, view_id);
    }
    fn is_revising(&self) -> bool {
        Revisable::is_revising(self)
    }
    fn to_string(&self) -> Option<String> {
        (*self.get()).to_string()
    }
}
impl<F, T> ClassPart for Bond<F, T>
where
    F: Fn() -> T + Clone + 'static,
    T: ClassPart + fmt::Debug + Clone + 'static,
{
    fn bind_view(&self, view_id: &ViewId) {
        Revisable::bind_view(self, view_id);
    }
    fn is_revising(&self) -> bool {
        Revisable::is_revising(self)
    }
    fn to_string(&self) -> Option<String> {
        (*self.get()).to_string()
    }
}

macro_rules! class_part {
    ($part_type:ty) => {
        impl ClassPart for $part_type {
            fn to_string(&self) -> Option<String> {
                Some(ToString::to_string(self))
            }
        }

        impl ClassPart for Option<$part_type> {
            fn to_string(&self) -> Option<String> {
                self.as_ref().map(|v| ToString::to_string(&v))
            }
        }
    };
}

class_part!(String);
class_part!(&String);
class_part!(&str);
class_part!(char);
