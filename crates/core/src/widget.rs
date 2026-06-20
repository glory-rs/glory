use std::any::Any;
use std::fmt;

use crate::{Node, Scope, View, ViewId};

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct BoundaryError {
    message: String,
    source: Option<String>,
}

impl BoundaryError {
    pub fn new(message: impl Into<String>, source: Option<ViewId>) -> Self {
        Self {
            message: message.into(),
            source: source.map(|view_id| view_id.to_string()),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    pub(crate) fn from_panic(payload: Box<dyn Any + Send>, source: Option<ViewId>) -> Self {
        let message = match payload.downcast::<String>() {
            Ok(message) => *message,
            Err(payload) => match payload.downcast::<&'static str>() {
                Ok(message) => (*message).to_owned(),
                Err(_) => "non-string panic payload".to_owned(),
            },
        };
        Self {
            message,
            source: source.map(|view_id| view_id.to_string()),
        }
    }
}

/// A reactive component.
///
/// `Widget` is the trait every renderable Glory type implements. The
/// lifecycle, in order, is:
///
/// 1. **Construction** — the widget is built as a plain Rust value
///    (e.g. `div().class("…")`). Builder methods only set fields;
///    nothing reactive happens yet.
/// 2. **`store_in(parent)` / `show_in(parent)`** — registers the
///    widget as a child of `parent`'s [`Scope`]. `show_in` additionally
///    marks it visible and attaches it if the parent is already
///    attached. `mount_to(scope, parent_node)` is the entry point for
///    a root widget; it inserts into the global `ROOT_VIEWS` map and
///    drives the rest of the lifecycle inside a [`crate::reflow::batch`].
/// 3. **`build(ctx)`** — runs once. Set up child widgets (via
///    `show_in(ctx)`), wire event handlers, register reactive
///    subscriptions. Anything captured by `.get()` on a `Cage` or
///    `Bond` here will trigger `patch` later.
/// 4. **`attach(ctx)`** — default no-op. Hook for "I've just been
///    inserted into the DOM tree" side-effects (autofocus, IO start,
///    timers, etc.).
/// 5. **`flood(ctx)`** — attaches the widget's children. Default
///    implementation calls `attach_child` on each. Element widgets
///    override this to first position their own node in the parent
///    DOM, then attach children.
/// 6. **`patch(ctx)`** — fires whenever a subscribed `Cage` / `Bond`
///    revises. The default is a no-op; override when the widget owns
///    derived structure (e.g. `Each::patch` reorders its children).
/// 7. **`detach(ctx)`** — runs when the parent removes this widget.
///    Default detaches all children.
///
/// # Visibility rules
///
/// - The widget's value is moved into a [`View`] via `store_in` /
///   `show_in`. After that, the original Rust binding is gone; the
///   `View` owns the widget and exposes only the trait methods.
/// - Each widget's reactive subscriptions are scoped to its own
///   `ViewId`. Drop the parent view and all descendant subscriptions
///   are released.
///
/// # Implementing your own
///
/// For HTML elements, prefer the `generate_tags!` macro in
/// `crate::web::widgets` rather than hand-writing the boilerplate.
/// For higher-level components (containers, controllers, etc.),
/// implement `Widget` directly and call `show_in` on child widgets
/// inside `build`.
pub trait Widget: fmt::Debug + 'static {
    fn store_in(self, parent: &mut Scope) -> ViewId
    where
        Self: Sized,
    {
        let view = View::new(parent.beget(), self);
        #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
        let view = {
            let mut view = view;
            if parent.compact_fill_allowed() {
                view.scope.fixed_parent_node = parent.render_node.clone();
            }
            view
        };
        let view_id = view.id.clone();
        parent.child_views.insert(view.id.clone(), view);
        view_id
    }
    fn show_in(self, parent: &mut Scope) -> ViewId
    where
        Self: Sized,
    {
        let view_id = self.store_in(parent);
        parent.visible_views.insert(view_id.clone());
        if parent.is_attached() && !parent.is_building() {
            parent.attach_child(&view_id);
        }
        view_id
    }
    fn fill_in(self, parent: &mut Scope)
    where
        Self: Sized,
    {
        self.show_in(parent);
    }
    fn can_fill_compact(&self) -> bool {
        false
    }

    fn mount_to(self, ctx: Scope, parent_node: &Node) -> ViewId
    where
        Self: Sized,
    {
        let mut view = View::new(ctx, self);

        view.scope.parent_node = Some(parent_node.clone());
        view.scope.render_node = Some(parent_node.clone());
        view.scope.visible_views.insert(view.id.clone());

        let view_id = view.id.clone();
        cfg_if! {
            if #[cfg(not(feature = "single-app"))] {
                let holder_id = view.holder_id();
            }
        }
        let process = || {
            crate::ROOT_VIEWS.with(|root_views| {
                let mut root_views = root_views.borrow_mut();
                cfg_if! {
                    if #[cfg(not(feature = "single-app"))] {
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
            if #[cfg(feature = "single-app")] {
                crate::reflow::batch(process);
            } else {
                crate::reflow::batch(holder_id, process);
            }
        }
        view_id
    }

    fn attach(&mut self, _ctx: &mut Scope) {}
    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    fn hydrate(&mut self, _ctx: &mut Scope) {}
    fn build(&mut self, _ctx: &mut Scope);
    fn capture_error(&mut self, _ctx: &mut Scope, _error: BoundaryError) -> bool {
        false
    }

    /// Attach children.
    ///
    /// The default implementation simply walks any children that were
    /// inserted via `store_in` during `build` and attaches them.  It must
    /// NOT pre-set their `scope.placement` — already-attached children have
    /// `placement == Unset` (reset by the end of `attach_child`), and
    /// forcing it back to `Tail` here would survive `attach_child`'s
    /// early-return on `is_attached` and break neighbour-relative
    /// re-positioning during later patches (e.g. `Each` reordering).
    fn flood(&mut self, ctx: &mut Scope) {
        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for id in ids {
            ctx.attach_child(&id);
        }
    }
    fn patch(&mut self, _ctx: &mut Scope) {}

    fn suspend(&mut self, ctx: &mut Scope) {
        self.suspend_children(ctx);
    }
    fn suspend_children(&mut self, ctx: &mut Scope) {
        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        for id in ids {
            ctx.hide_child(&id);
        }
    }

    fn detach(&mut self, ctx: &mut Scope) {
        self.detach_children(ctx);
    }
    fn detach_children(&mut self, ctx: &mut Scope) {
        let ids: Vec<ViewId> = ctx.child_views.keys().cloned().collect();
        if !ids.is_empty() {
            cfg_if! {
                if #[cfg(feature = "single-app")] {
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
                w.fill_in(ctx);
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
            self.fill_in(ctx);
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
                widget.fill_in(ctx);
            })
        } else {
            Filler::new(|_| {})
        }
    }
}
