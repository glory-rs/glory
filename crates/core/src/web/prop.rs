use std::fmt;

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::{JsValue, UnwrapThrowExt};

use crate::node::Node;
use crate::reflow::{Bond, Cage, Record, Revisable};
use crate::ViewId;

pub trait PropValue: fmt::Debug {
    fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool);
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
impl PropValue for JsValue {
    fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if first_time {
            js_sys::Reflect::set(node, &JsValue::from_str(name), self).unwrap_throw();
        }
    }
}
impl PropValue for String {
    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if first_time {
            js_sys::Reflect::set(node, &JsValue::from_str(name), &self.into()).unwrap_throw();
        }
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
    fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if first_time {
            node.set_property(name.to_owned(), self.clone());
        }
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
impl<T> PropValue for Cage<T>
where
    T: Into<JsValue> + fmt::Debug + Clone + 'static,
{
    fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if self.is_revising() || first_time {
            js_sys::Reflect::set(node, &JsValue::from_str(name), &self.get_untracked().clone().into()).unwrap_throw();
        }
        if first_time {
            self.bind_view(view_id);
        }
    }
}
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
impl<T> PropValue for Cage<T>
where
    T: Into<String> + fmt::Debug + Clone + 'static,
{
    fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if self.is_revising() || first_time {
            let value: String = (*self.get()).clone().into();
            node.set_property(name.to_owned(), value);
        }
        if first_time {
            self.bind_view(view_id);
        }
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
impl<F, T> PropValue for Bond<F, T>
where
    F: Fn() -> T + Clone + 'static,
    T: Into<JsValue> + fmt::Debug + Clone + 'static,
{
    fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if self.is_revising() || first_time {
            js_sys::Reflect::set(node, &JsValue::from_str(name), &self.get_untracked().clone().into()).unwrap_throw();
        }
        if first_time {
            self.bind_view(view_id);
        }
    }
}

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
impl<F, T> PropValue for Bond<F, T>
where
    F: Fn() -> T + Clone + 'static,
    T: Into<String> + fmt::Debug + Clone + 'static,
{
    fn inject_to(&self, view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
        if self.is_revising() || first_time {
            let value: String = (*self.get()).clone().into();
            node.set_property(name.to_owned(), value);
        }
        if first_time {
            self.bind_view(view_id);
        }
    }
}

macro_rules! prop_type {
    ($prop_type:ty) => {
        impl PropValue for $prop_type {
            #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
            fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
                if first_time {
                    js_sys::Reflect::set(node, &JsValue::from_str(name), &(*self).into()).unwrap_throw();
                }
            }
            #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
            fn inject_to(&self, _view_id: &ViewId, node: &mut Node, name: &str, first_time: bool) {
                if first_time {
                    let value: String = (*self).to_string();
                    node.set_property(name.to_owned(), value);
                }
            }
        }
    };
}

prop_type!(&String);
prop_type!(&str);
prop_type!(usize);
prop_type!(u8);
prop_type!(u16);
prop_type!(u32);
prop_type!(u64);
prop_type!(u128);
prop_type!(isize);
prop_type!(i8);
prop_type!(i16);
prop_type!(i32);
prop_type!(i64);
prop_type!(i128);
prop_type!(f32);
prop_type!(f64);
prop_type!(bool);
