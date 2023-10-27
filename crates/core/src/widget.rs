use std::fmt;

use crate::{view::ViewPosition, Node, Scope, View, ViewId};

pub trait Widget: fmt::Debug + 'static {
    fn store_in(self, parent: &mut Scope) -> ViewId
    where
        Self: Sized,
    {
        let view = View::new(parent.beget(), self);
        let view_id = view.id.clone();
        parent.child_views.insert(view.id.clone(), view);
        view_id
    }
    fn show_in(self, parent: &mut Scope) -> ViewId
    where
        Self: Sized,
    {
        let view_id = self.store_in(parent);
        parent.show_list.insert(view_id.clone());
        view_id
    }

    fn mount_to(self, ctx: Scope, parent_node: &Node) -> ViewId
    where
        Self: Sized,
    {
        let mut view = View::new(ctx, self);

        view.scope.parent_node = Some(parent_node.clone());
        view.scope.graff_node = Some(parent_node.clone());
        view.scope.show_list.insert(view.id.clone());

        let view_id = view.id.clone();
        cfg_if! {
            if #[cfg(not(feature = "__single_holder"))] {
                let holder_id = view.holder_id();
            }
        }
        let process = || {
            crate::ROOT_VIEWS.with(|root_views| {
                let mut root_views = root_views.borrow_mut();
                cfg_if! {
                    if #[cfg(not(feature = "__single_holder"))] {
                        let holder_id = view.holder_id();
                        let root_views = root_views.entry(holder_id).or_default();
                    }
                }
                root_views.insert(view_id.clone(), view);
                let view = root_views.get_mut(&view_id).unwrap();
                view.attach();
            })
        };
        cfg_if! {
            if #[cfg(feature = "__single_holder")] {
                crate::reflow::batch(process);
            } else {
                crate::reflow::batch(holder_id, process);
            }
        }
        view_id
    }

    // fn mount_to(self, parent: impl AsRef<web_sys::Element>, truck: Rc<RefCell<Truck>>)
    // where
    //     Self: Sized,
    // {
    //     self.into_view(truck).mount_to(parent)
    // }

    fn attach(&mut self, _ctx: &mut Scope) {}
    fn build(&mut self, _ctx: &mut Scope);

    /// Attach children
    fn flood(&mut self, ctx: &mut Scope) {
        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for view in ctx.child_views.values_mut() {
            view.scope.position = ViewPosition::Tail;
        }
        for id in ids {
            ctx.attach_child(&id);
        }
    }
    fn patch(&mut self, _ctx: &mut Scope) {}

    fn detach(&mut self, ctx: &mut Scope) {
        self.detach_children(ctx);
    }
    fn detach_children(&mut self, ctx: &mut Scope) {
        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        if !ids.is_empty() {
            cfg_if! {
                if #[cfg(feature = "__single_holder")] {
                    crate::reflow::batch(|| {
                        for id in ids {
                            ctx.detach_child(&id);
                        }
                    });
                } else {
                    crate::reflow::batch(ctx.holder_id(), || {
                        for id in ids {
                            ctx.detach_child(&id);
                        }
                    });
                }
            }
        }
    }
    // fn wreck(mut self, ctx: &mut Scope) {
    //     let child_views = std::mem::replace(&mut ctx.child_views, vec![]);
    //     for (_, view) in child_views {
    //         self.wreck_child(view);
    //     }
    // }
}

pub struct Filler(Box<dyn FnOnce(&mut Scope)>);
impl Filler {
    pub fn new(filler: impl FnOnce(&mut Scope) + 'static) -> Self {
        Self(Box::new(filler))
    }
    pub fn fill(self, parent: &mut Scope) {
        (self.0)(parent);
    }
}

pub trait IntoFiller {
    fn into_filler(self) -> Filler;
}

impl<W> IntoFiller for Vec<W>
where
    W: Widget,
{
    fn into_filler(self) -> Filler {
        Filler::new(move |ctx: &mut Scope| {
            for w in self {
                w.show_in(ctx);
            }
        })
    }
}
impl<W> IntoFiller for W
where
    W: Widget,
{
    fn into_filler(self) -> Filler {
        Filler::new(move |ctx: &mut Scope| {
            self.show_in(ctx);
        })
    }
}

impl<W> IntoFiller for Option<W>
where
    W: Widget,
{
    fn into_filler(self) -> Filler {
        if let Some(widget) = self {
            Filler::new(move |ctx: &mut Scope| {
                widget.show_in(ctx);
            })
        } else {
            Filler::new(|_|{})
        }
    }
}
