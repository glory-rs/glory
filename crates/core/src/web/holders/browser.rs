use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use wasm_bindgen::{JsCast, UnwrapThrowExt};

use crate::{Holder, Scope, Truck, ViewId, Widget};

pub struct BrowerHolder {
    pub truck: Rc<RefCell<Truck>>,
    pub host_node: web_sys::Element,
    next_root_view_id: AtomicU64,
}
impl BrowerHolder {
    pub fn new() -> Self {
        Self::with_host_node(crate::web::document().body().expect("body not found"))
    }
    pub fn with_host_node(host_node: impl AsRef<web_sys::Element>) -> Self {
        Self {
            truck: Default::default(),
            host_node: host_node.as_ref().clone(),
            next_root_view_id: AtomicU64::new(0),
        }
    }
}

impl Debug for BrowerHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowerHolder").finish()
    }
}

impl Holder for BrowerHolder {
    fn mount(self, widget: impl Widget) -> Self {
        crate::web::HYDRATING.store(true, Ordering::Relaxed);
        let view_id = ViewId::new(self.next_root_view_id.fetch_add(1, Ordering::Relaxed).to_string());
        let scope = Scope::new_root(view_id, self.truck.clone());
        widget.mount_to(scope, &self.host_node);
        crate::web::HYDRATING.store(false, Ordering::Relaxed);
        if let Ok(list) = crate::web::document().query_selector_all("[gly-id]") {
            for i in 0..list.length() {
                let ele = list.item(i).unwrap().unchecked_into::<web_sys::HtmlElement>();
                crate::info!("[hydrating]: remove element: {}", ele.outer_html());
                ele.remove();
            }
        }
        self
    }
    fn truck(&self) -> Rc<RefCell<Truck>> {
        self.truck.clone()
    }
    // fn clone_boxed(&self) -> Box<dyn Holder> {
    //     Box::new(Self {
    //         truck: self.truck.clone(),
    //         host_node: self.host_node.clone(),
    //     })
    // }
}
