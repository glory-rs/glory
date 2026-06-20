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

/// Browser CSR: widgets operate directly on live DOM elements.
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub type Node = web_sys::Element;

/// Every other target speaks the command stream: widgets operate on
/// id-based [`CommandNode`](crate::renderer::CommandNode) handles whose
/// mutations are recorded as serializable
/// [`Command`](crate::renderer::Command)s. Consumers decide what a batch
/// means — the SSR holder replays it into an
/// [`SsrDocument`](crate::renderer::ssr_dom::SsrDocument) for HTML,
/// desktop ships it over IPC to a webview, native/TUI interpret it
/// themselves.
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub type Node = crate::renderer::CommandNode;
