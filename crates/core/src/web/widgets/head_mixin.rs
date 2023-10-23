use educe::Educe;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::UnwrapThrowExt;

use crate::widget::{Filler, IntoFiller};
use crate::{Node, Scope, ViewId, Widget};

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub const DEPOT_HEAD_MIXIN_KEY: &str = "glory::web::head_mixin";

#[derive(Educe)]
#[educe(Debug)]
pub struct HeadMixin {
    #[educe(Debug(ignore))]
    #[allow(clippy::type_complexity)]
    pub fillers: Vec<Filler>,

    head_node: Node,
}

impl Widget for HeadMixin {
    fn build(&mut self, ctx: &mut Scope) {
        ctx.graff_node = Some(self.head_node.clone());

        let fillers = std::mem::take(&mut self.fillers);
        for filler in fillers {
            filler.fill(ctx);
        }

        #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
        ctx.truck_mut().insert(DEPOT_HEAD_MIXIN_KEY, self.head_node.clone());
    }

    fn flood(&mut self, ctx: &mut Scope) {
        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for id in ids {
            ctx.attach_child(&id);
        }
        self.patch(ctx);
    }
}
impl Default for HeadMixin {
    fn default() -> Self {
        cfg_if! {
            if #[cfg(all(target_arch = "wasm32", feature = "web-csr"))] {
                Self {
                    fillers: vec![],
                    head_node: crate::web::document().head().unwrap_throw().into(),
                }
            } else {
                Self {
                    fillers: vec![],
                    head_node: Node::new("head", false),
                }
            }
        }
    }
}

impl HeadMixin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fill(mut self, filler: impl IntoFiller) -> Self {
        self.fillers.push(filler.into_filler());
        self
    }
}

pub fn head_mixin() -> HeadMixin {
    HeadMixin::new()
}
