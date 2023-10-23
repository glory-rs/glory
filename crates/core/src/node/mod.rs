use std::cell::{Ref, RefCell};
use std::fmt;
use std::rc::Rc;

#[derive(Default, Debug, Clone)]
pub struct NodeRef<N: fmt::Debug> {
    node: Rc<RefCell<Option<N>>>,
}

impl<N: fmt::Debug> NodeRef<N> {
    pub fn new() -> Self {
        Self {
            node: Rc::new(RefCell::new(None)),
        }
    }
    pub fn get(&self) -> Ref<'_, Option<N>> {
        self.node.borrow()
    }
    pub fn set(&self, node: N) {
        let _ = self.node.borrow_mut().insert(node);
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub type Node = web_sys::Element;

cfg_feature! {
    #![not(all(target_arch = "wasm32", feature = "web-csr"))]
    mod ssr;
    pub use ssr::*;
}
