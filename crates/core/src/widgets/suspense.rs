use std::fmt;

use crate::scope::SuspenseBoundary;
use crate::{Cage, Filler, IntoFiller, Scope, ViewId, Widget};

#[cfg(all(not(feature = "single-app"), feature = "web-ssr", not(target_arch = "wasm32")))]
use crate::node::Node;

pub struct Suspense {
    body: Option<Filler>,
    fallback: Box<dyn Fn(&mut Scope)>,
    boundary: SuspenseBoundary,
    body_ids: Vec<ViewId>,
    fallback_ids: Vec<ViewId>,
    showing_fallback: bool,
    /// Streaming-SSR wrapper node tagged `data-glory-suspense`. Present only
    /// while a `ServerHolder` streaming mount is active; its children are the
    /// boundary region the holder swaps fallback ↔ resolved body inside.
    #[cfg(all(not(feature = "single-app"), feature = "web-ssr", not(target_arch = "wasm32")))]
    wrapper: Option<Node>,
}

impl fmt::Debug for Suspense {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Suspense")
            .field("pending", &self.boundary.pending_count())
            .field("body_ids", &self.body_ids)
            .field("fallback_ids", &self.fallback_ids)
            .field("showing_fallback", &self.showing_fallback)
            .finish()
    }
}

impl Suspense {
    pub fn new(body: impl IntoFiller, fallback: impl Fn(&mut Scope) + 'static) -> Self {
        Self {
            body: Some(body.into_filler()),
            fallback: Box::new(fallback),
            boundary: SuspenseBoundary::new(Cage::new(0)),
            body_ids: Vec::new(),
            fallback_ids: Vec::new(),
            showing_fallback: false,
            #[cfg(all(not(feature = "single-app"), feature = "web-ssr", not(target_arch = "wasm32")))]
            wrapper: None,
        }
    }

    pub fn pending_count(&self) -> usize {
        self.boundary.pending_count()
    }

    fn fill_body(&mut self, ctx: &mut Scope) {
        let Some(body) = self.body.take() else {
            return;
        };
        let previous = ctx.suspense_boundary.replace(self.boundary);
        body.fill(ctx);
        ctx.suspense_boundary = previous;
        self.body_ids = ctx.child_views.keys().cloned().collect();
    }

    fn fill_fallback(&mut self, ctx: &mut Scope) {
        let before = ctx.child_views.keys().cloned().collect::<Vec<_>>();
        (self.fallback)(ctx);
        self.fallback_ids = ctx
            .child_views
            .keys()
            .filter(|id| !self.body_ids.contains(id) && !before.contains(id))
            .cloned()
            .collect();
    }

    fn show_ids(ctx: &mut Scope, ids: &[ViewId]) {
        for id in ids {
            ctx.attach_child(id);
        }
    }

    fn hide_ids(ctx: &mut Scope, ids: &[ViewId]) {
        for id in ids {
            ctx.hide_child(id);
        }
    }

    /// Streaming SSR only: create the wrapper node, route the boundary's
    /// children under it, and register it so the holder can stream the
    /// resolved body. No-op for every other render path.
    #[cfg(all(not(feature = "single-app"), feature = "web-ssr", not(target_arch = "wasm32")))]
    fn stream_setup(&mut self, ctx: &mut Scope) {
        use crate::renderer::{BackendRenderer, Renderer};

        if !crate::stream_ssr::is_streaming() {
            return;
        }

        let renderer = BackendRenderer::default();
        let wrapper = renderer.create_element("glory-suspense".into(), false);
        let placeholder_id = crate::stream_ssr::next_placeholder_id();
        renderer.set_attribute(&wrapper, "data-glory-suspense".into(), placeholder_id.clone().into());
        let wrapper_id = wrapper.id();

        // The boundary region lives under the wrapper; children attach here.
        ctx.render_node = Some(wrapper.clone());
        ctx.first_child_node = Some(wrapper.clone());
        ctx.last_child_node = Some(wrapper.clone());

        crate::stream_ssr::register_boundary(crate::stream_ssr::BoundaryRegistration {
            placeholder_id,
            wrapper_id,
            boundary: self.boundary,
        });
        self.wrapper = Some(wrapper);
    }

    /// Streaming SSR only: place the wrapper under the boundary's parent,
    /// honouring this view's sibling placement (mirrors `Element::flood`).
    #[cfg(all(not(feature = "single-app"), feature = "web-ssr", not(target_arch = "wasm32")))]
    fn stream_insert_wrapper(&self, ctx: &mut Scope) {
        use crate::renderer::{BackendRenderer, InsertPosition, Renderer};
        use crate::view::ViewPlacement;

        let Some(wrapper) = self.wrapper.as_ref() else {
            return;
        };
        let Some(parent_node) = ctx.parent_node.as_ref() else {
            return;
        };
        let renderer = BackendRenderer::default();
        match &ctx.placement {
            ViewPlacement::Head => renderer.insert_child(parent_node, wrapper, InsertPosition::Head),
            ViewPlacement::Before(next_node) => renderer.insert_child(parent_node, wrapper, InsertPosition::Before(next_node)),
            ViewPlacement::After(prev_node) => renderer.insert_child(parent_node, wrapper, InsertPosition::After(prev_node)),
            ViewPlacement::Tail | ViewPlacement::Unset => renderer.insert_child(parent_node, wrapper, InsertPosition::Tail),
        }
    }

    #[cfg(not(all(not(feature = "single-app"), feature = "web-ssr", not(target_arch = "wasm32"))))]
    fn stream_setup(&mut self, _ctx: &mut Scope) {}

    #[cfg(not(all(not(feature = "single-app"), feature = "web-ssr", not(target_arch = "wasm32"))))]
    fn stream_insert_wrapper(&self, _ctx: &mut Scope) {}
}

impl Widget for Suspense {
    fn build(&mut self, ctx: &mut Scope) {
        self.boundary.bind_view(ctx.view_id());
        self.stream_setup(ctx);
        self.fill_body(ctx);
    }

    fn flood(&mut self, ctx: &mut Scope) {
        self.stream_insert_wrapper(ctx);
        Self::show_ids(ctx, &self.body_ids);
        self.showing_fallback = self.boundary.pending_count() > 0;
        if self.showing_fallback {
            Self::hide_ids(ctx, &self.body_ids);
            if self.fallback_ids.is_empty() {
                self.fill_fallback(ctx);
            }
            Self::show_ids(ctx, &self.fallback_ids);
        }
    }

    fn patch(&mut self, ctx: &mut Scope) {
        let should_show_fallback = self.boundary.pending_count() > 0;
        if should_show_fallback == self.showing_fallback {
            return;
        }

        if should_show_fallback {
            Self::hide_ids(ctx, &self.body_ids);
            if self.fallback_ids.is_empty() {
                self.fill_fallback(ctx);
            } else {
                Self::show_ids(ctx, &self.fallback_ids);
            }
        } else {
            Self::hide_ids(ctx, &self.fallback_ids);
            Self::show_ids(ctx, &self.body_ids);
        }
        self.showing_fallback = should_show_fallback;
    }
}
