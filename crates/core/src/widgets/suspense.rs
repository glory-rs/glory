use std::fmt;

use crate::scope::SuspenseBoundary;
use crate::{Cage, Filler, IntoFiller, Scope, ViewId, Widget};

pub struct Suspense {
    body: Option<Filler>,
    fallback: Box<dyn Fn(&mut Scope)>,
    boundary: SuspenseBoundary,
    body_ids: Vec<ViewId>,
    fallback_ids: Vec<ViewId>,
    showing_fallback: bool,
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
}

impl Widget for Suspense {
    fn build(&mut self, ctx: &mut Scope) {
        self.boundary.bind_view(ctx.view_id());
        self.fill_body(ctx);
    }

    fn flood(&mut self, ctx: &mut Scope) {
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
