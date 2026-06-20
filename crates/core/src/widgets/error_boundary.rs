use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::UnwrapThrowExt;

use crate::{BoundaryError, Filler, IntoFiller, Scope, ViewId, Widget};

pub struct ErrorBoundary {
    body: Option<Filler>,
    fallback: Box<dyn Fn(&BoundaryError, &mut Scope)>,
    error: Option<BoundaryError>,
}

impl fmt::Debug for ErrorBoundary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorBoundary")
            .field("has_body", &self.body.is_some())
            .field("error", &self.error)
            .finish()
    }
}

impl ErrorBoundary {
    pub fn new(body: impl IntoFiller, fallback: impl Fn(&BoundaryError, &mut Scope) + 'static) -> Self {
        Self {
            body: Some(body.into_filler()),
            fallback: Box::new(fallback),
            error: None,
        }
    }

    pub fn error(&self) -> Option<&BoundaryError> {
        self.error.as_ref()
    }

    fn fill_body(&mut self, ctx: &mut Scope) -> Result<(), BoundaryError> {
        let Some(body) = self.body.take() else {
            return Ok(());
        };
        let boundary = ctx.view_id.clone();
        let previous = ctx.error_boundary.replace(boundary);
        let result = catch_unwind(AssertUnwindSafe(|| body.fill(ctx)));
        ctx.error_boundary = previous;
        result.map_err(|payload| BoundaryError::from_panic(payload, None))
    }

    fn render_fallback(&self, ctx: &mut Scope) {
        let Some(error) = &self.error else {
            return;
        };
        let result = catch_unwind(AssertUnwindSafe(|| (self.fallback)(error, ctx)));
        if let Err(payload) = result {
            resume_unwind(payload);
        }
    }

    fn replace_with_fallback(&mut self, ctx: &mut Scope, error: BoundaryError) {
        self.error = Some(error);
        clear_children(ctx);
        self.render_fallback(ctx);
        #[cfg(feature = "web-ssr")]
        save_state(ctx, self.error.as_ref().unwrap());
    }

    fn attach_body_children(&mut self, ctx: &mut Scope) {
        let ids = ctx.child_views.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            let result = catch_unwind(AssertUnwindSafe(|| ctx.attach_child(&id)));
            if let Err(payload) = result {
                let error = BoundaryError::from_panic(payload, Some(id));
                self.replace_with_fallback(ctx, error);
                return;
            }
        }
    }
}

impl Widget for ErrorBoundary {
    fn build(&mut self, ctx: &mut Scope) {
        #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
        if crate::web::is_hydrating()
            && let Some(error) = load_state(ctx)
        {
            self.error = Some(error);
        }

        if self.error.is_some() {
            self.render_fallback(ctx);
            return;
        }

        if let Err(error) = self.fill_body(ctx) {
            self.replace_with_fallback(ctx, error);
        }
    }

    fn flood(&mut self, ctx: &mut Scope) {
        if self.error.is_some() {
            let ids = ctx.child_views.keys().cloned().collect::<Vec<_>>();
            for id in ids {
                ctx.attach_child(&id);
            }
            return;
        }
        self.attach_body_children(ctx);
    }

    fn capture_error(&mut self, ctx: &mut Scope, error: BoundaryError) -> bool {
        self.replace_with_fallback(ctx, error);
        true
    }
}

fn clear_children(ctx: &mut Scope) {
    let ids = ctx.child_views.keys().cloned().collect::<Vec<ViewId>>();
    for id in ids {
        let _ = ctx.detach_child(&id);
        ctx.child_views.shift_remove(&id);
    }
    ctx.visible_views.clear();
}

#[cfg(feature = "web-ssr")]
fn save_state(ctx: &Scope, error: &BoundaryError) {
    pub use base64::prelude::*;
    if let Some(parent_node) = &ctx.parent_node {
        let key = format!("gly-error-{}", ctx.view_id());
        let data = serde_json::to_string(error).expect("boundary error serializes");
        parent_node.set_attribute(key, BASE64_STANDARD_NO_PAD.encode(&data));
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
fn load_state(ctx: &Scope) -> Option<BoundaryError> {
    let key = format!("gly-error-{}", ctx.view_id());
    if let Some(parent_node) = &ctx.parent_node
        && let Some(data) = parent_node.get_attribute(&key)
    {
        parent_node.remove_attribute(&key).ok();
        let data = crate::web::window().atob(&data).unwrap_throw();
        return serde_json::from_str(&data).ok();
    }
    None
}
