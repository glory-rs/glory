use glory_core::reflow::{self, Revisable};
use glory_core::{Scope, ViewId, Widget};

use crate::TruckExt;

#[derive(Debug, Default)]
pub struct Graff {
    name: String,
}

impl Graff {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Widget for Graff {
    fn build(&mut self, ctx: &mut Scope) {
        ctx.truck.stuffs().bind_view(ctx.view_id());

        cfg_if! {
            if #[cfg(feature = "__single_holder")] {
                let stuff = reflow::untrack(|| ctx.truck.remove_stuff(&self.name));
            } else {
                let stuff = reflow::untrack(ctx.holder_id(), || ctx.truck.remove_stuff(&self.name));
            }
        }
        if let Some(stuff) = stuff {
            let view_id = (stuff.0)(ctx);
            ctx.attach_child(&view_id);
        }
    }

    fn patch(&mut self, ctx: &mut Scope) {
        cfg_if! {
            if #[cfg(feature = "__single_holder")] {
                let stuff = glory_core::reflow::untrack(|| ctx.truck.remove_stuff(&self.name));
            } else {
                let stuff = glory_core::reflow::untrack(ctx.holder_id(), || ctx.truck.remove_stuff(&self.name));
            }
        }
        if let Some(stuff) = stuff {
            let view_ids: Vec<ViewId> = ctx.child_views().keys().cloned().collect();
            if !view_ids.is_empty() {
                cfg_if! {
                    if #[cfg(feature = "__single_holder")] {
                        glory_core::reflow::batch(|| {
                            for view_id in &view_ids {
                                ctx.detach_child(view_id);
                            }
                        });
                    } else {
                        glory_core::reflow::batch(ctx.holder_id(), || {
                            for view_id in &view_ids {
                                ctx.detach_child(view_id);
                            }
                        });
                    }
                }
            }
            glory_core::info!("mounting stuff: {:?}", self.name);
            let view_id = (stuff.0)(ctx);
            ctx.attach_child(&view_id);
        }
    }
}
