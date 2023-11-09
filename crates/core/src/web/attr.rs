use std::fmt;
use std::ops::Deref;

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::UnwrapThrowExt;

use crate::node::Node;
use crate::reflow::{Bond, Cage, Revisable};
use crate::ViewId;

/// Represents the different possible values an attribute node could have.
///
/// This mostly exists for the [`view`](https://docs.rs/glory_macro/latest/glory_macro/macro.view.html)
/// macro’s use. You usually won't need to interact with it directly.
pub trait AttrValue: fmt::Debug {
    fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool);
    fn to_string(&self) -> Option<String>;
}

impl<T> AttrValue for Cage<T>
where
    T: AttrValue + fmt::Debug + Clone + 'static,
{
    fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if self.is_revising() || first_time {
            (*self.get_untracked()).inject_to(view_id, node, name, true);
        }
        if first_time {
            self.bind_view(view_id);
        }
    }
    fn to_string(&self) -> Option<String> {
        (*self.get_untracked()).to_string()
    }
}
impl<T> AttrValue for Bond<T>
where
    T: AttrValue + fmt::Debug + Clone + 'static,
{
    fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if self.is_revising() || first_time {
            (*self.get_untracked()).inject_to(view_id, node, name, true);
        }
        if first_time {
            self.bind_view(view_id);
        }
    }
    fn to_string(&self) -> Option<String> {
        (*self.get_untracked()).to_string()
    }
}

impl AttrValue for bool {
    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if first_time {
            if *self && node.get_attribute("name").as_deref() != Some(name) {
                node.set_attribute(name, name).unwrap_throw();
            } else if !*self && node.has_attribute("name") {
                node.remove_attribute(name).unwrap_throw();
            }
        }
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
    fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if first_time {
            if *self {
                node.set_attribute(name.to_owned(), name.to_owned());
            } else {
                node.remove_attribute(name);
            }
        }
    }
    fn to_string(&self) -> Option<String> {
        if *self {
            Some("".into())
        } else {
            None
        }
    }
}

impl AttrValue for ViewId {
    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if first_time && node.get_attribute("name").as_deref() != Some(self.deref()) {
            node.set_attribute(name, self.deref()).unwrap_throw();
        }
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
    fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if first_time {
            node.set_attribute(name.to_owned(), self.deref().to_owned());
        }
    }
    fn to_string(&self) -> Option<String> {
        Some(self.deref().to_owned())
    }
}

macro_rules! attr_type {
    ($attr_type:ty) => {
        impl AttrValue for $attr_type {
            #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
            fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
                if first_time {
                    let value = ToString::to_string(self);
                    if name == "inner_html" || name == "inner_text" {
                        if node.inner_html() != value {
                            node.set_inner_html(&value);
                        }
                    } else if node.get_attribute(name).as_ref() != Some(&value) {
                        node.set_attribute(name, &value).unwrap_throw();
                    }
                }
            }
            #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
            fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
                if first_time {
                    node.set_attribute(name.to_owned(), ToString::to_string(self));
                }
            }
            fn to_string(&self) -> Option<String> {
                Some(ToString::to_string(self))
            }
        }

        impl AttrValue for Option<$attr_type> {
            fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
                if first_time {
                    if let Some(value) = self {
                        AttrValue::inject_to(value, view_id, node, name, first_time);
                    } else {
                        #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
                        node.remove_attribute(name).unwrap_throw();
                        #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
                        node.remove_attribute(name);
                    }
                }
            }
            fn to_string(&self) -> Option<String> {
                self.as_ref().map(|v| ToString::to_string(&v))
            }
        }
    };
}

attr_type!(String);
attr_type!(&String);
attr_type!(&str);
attr_type!(usize);
attr_type!(u8);
attr_type!(u16);
attr_type!(u32);
attr_type!(u64);
attr_type!(u128);
attr_type!(isize);
attr_type!(i8);
attr_type!(i16);
attr_type!(i32);
attr_type!(i64);
attr_type!(i128);
attr_type!(f32);
attr_type!(f64);
attr_type!(char);
