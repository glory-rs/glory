use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::config::GloryConfig;
use crate::reflow::scheduler::{BATCHING, RUNNING};
use crate::reflow::{PENDING_ITEMS, REVISING_ITEMS};
use crate::renderer::CommandQueue;
use crate::renderer::ssr_dom::SsrDocument;
use crate::web::widgets::*;
use crate::{Holder, HolderId, ROOT_VIEWS, Scope, Truck, ViewId, Widget};

const DEPOT_URL_KEY: &str = "glory::url";

pub struct ServerHolder {
    id: HolderId,
    pub config: Arc<GloryConfig>,
    pub truck: Rc<RefCell<Truck>>,
    pub host_node: HtmlDiv,
    /// The command stream every widget mutation of this holder lands in;
    /// rendering replays it into an [`SsrDocument`].
    queue: CommandQueue,
    next_root_view_id: AtomicU64,
    /// When set, the holder mounts with Suspense streaming armed: async
    /// resources defer instead of blocking, so the shell flushes with
    /// fallbacks, then resolved bodies stream in as out-of-order patches.
    streaming: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HtmlChunk {
    DocumentStart(String),
    BodyOpen(String),
    App(String),
    Placeholder { id: String, fallback_html: String },
    PlaceholderPatch { id: String, html: String },
    DocumentEnd(&'static str),
}

impl HtmlChunk {
    pub fn into_string(self) -> String {
        match self {
            HtmlChunk::DocumentStart(value) | HtmlChunk::BodyOpen(value) | HtmlChunk::App(value) => value,
            HtmlChunk::Placeholder { id, fallback_html } => {
                format!(r#"<template data-glory-placeholder="{}">{}</template>"#, escape_html_attr(&id), fallback_html)
            }
            HtmlChunk::PlaceholderPatch { id, html } => {
                let id_attr = escape_html_attr(&id);
                let id_json = serde_json::to_string(&id).expect("placeholder id can always be encoded as JSON");
                format!(
                    r#"<template data-glory-placeholder-patch="{id_attr}">{html}</template><script>window.__gloryStreamHydrate&&window.__gloryStreamHydrate.patchFromTemplate({id_json});</script>"#
                )
            }
            HtmlChunk::DocumentEnd(value) => value.to_owned(),
        }
    }
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl ServerHolder {
    pub fn new(config: impl Into<Arc<GloryConfig>>, url: impl Into<String>) -> Self {
        Self::with_streaming(config, url, false)
    }

    /// Like [`new`](Self::new) but arms Suspense streaming SSR: the holder
    /// flushes its shell with Suspense fallbacks immediately, defers async
    /// resources instead of blocking on them, and streams each resolved body
    /// back as an out-of-order `<template data-glory-placeholder-patch>` the
    /// client runtime swaps in. Use this when an app has `Suspense`/`resource`
    /// data you want progressively rendered; plain [`new`](Self::new) keeps
    /// the blocking render-fully-resolved behaviour.
    pub fn new_streaming(config: impl Into<Arc<GloryConfig>>, url: impl Into<String>) -> Self {
        Self::with_streaming(config, url, true)
    }

    fn with_streaming(config: impl Into<Arc<GloryConfig>>, url: impl Into<String>, streaming: bool) -> Self {
        let mut truck = Truck::new();
        truck.insert(DEPOT_URL_KEY, url.into());
        let queue = CommandQueue::new();
        let host_node = {
            let _guard = queue.make_current();
            crate::web::widgets::div()
        };
        let id = HolderId::next();
        crate::renderer::command::register_holder_queue(id, queue.clone());
        Self {
            id,
            config: config.into(),
            truck: Rc::new(RefCell::new(truck)),
            host_node,
            queue,
            next_root_view_id: AtomicU64::new(0),
            streaming,
        }
    }

    /// Replays the recorded command stream into the legacy-exact SSR tree.
    /// Non-draining: rendering is repeatable.
    pub fn replay(&self) -> SsrDocument {
        SsrDocument::replay(&self.queue.commands())
    }

    /// Rendered HTML of the mounted app subtree (what becomes the
    /// [`HtmlChunk::App`] chunk).
    pub fn app_html(&self) -> String {
        self.replay().inner_html(self.host_node.node().id())
    }

    pub fn html_chunks(&self) -> Vec<HtmlChunk> {
        let document = self.replay();
        let (head, mid, tail) = crate::web::utils::html_parts_separated(&self.config, &self.truck.borrow(), &document);
        let mut chunks = vec![HtmlChunk::DocumentStart(head), HtmlChunk::BodyOpen(mid)];
        chunks.extend(document.inner_html_chunks(self.host_node.node().id()).into_iter().map(HtmlChunk::App));
        #[cfg(not(target_arch = "wasm32"))]
        chunks.extend(resource_hydration_chunk());
        chunks.push(HtmlChunk::DocumentEnd(tail));
        chunks
    }

    /// The chunk sequence actually rendered for this holder: the streaming
    /// pipeline when armed via [`new_streaming`](Self::new_streaming),
    /// otherwise the blocking-resolved [`html_chunks`](Self::html_chunks).
    fn rendered_chunks(&self) -> Vec<HtmlChunk> {
        #[cfg(not(target_arch = "wasm32"))]
        if self.streaming {
            return self.streaming_chunks();
        }
        self.html_chunks()
    }

    /// Streaming SSR pipeline: serialize the shell with pending Suspense
    /// boundaries collapsed to `<template data-glory-placeholder>` markers,
    /// drive the deferred async resources to completion, then emit a
    /// `PlaceholderPatch` carrying each resolved body.
    #[cfg(not(target_arch = "wasm32"))]
    fn streaming_chunks(&self) -> Vec<HtmlChunk> {
        use std::collections::HashSet;

        use crate::renderer::ssr_dom::SsrNode;

        // Disarm streaming and collect the boundaries registered during mount.
        let boundaries = crate::stream_ssr::finish();
        // Pending *before* draining = the boundaries that need a placeholder
        // now and a streamed patch later.
        let pending: HashSet<String> = boundaries
            .iter()
            .filter(|registration| registration.boundary.pending_count() > 0)
            .map(|registration| registration.placeholder_id.clone())
            .collect();

        let host_id = self.host_node.node().id();
        let initial_doc = self.replay();

        let pending_for_initial = pending.clone();
        let initial_replace = move |node: &SsrNode| -> Option<String> {
            let id = node.get_attribute("data-glory-suspense")?;
            let inner = node.inner_html_with(&unwrap_suspense);
            if pending_for_initial.contains(&id) {
                Some(format!(
                    r#"<template data-glory-placeholder="{}">{}</template>"#,
                    escape_html_attr(&id),
                    inner
                ))
            } else {
                // Resolved synchronously (no async work) — unwrap inline.
                Some(inner)
            }
        };

        let (head, mid, tail) = crate::web::utils::html_parts_separated(&self.config, &self.truck.borrow(), &initial_doc);
        let mut chunks = vec![HtmlChunk::DocumentStart(head), HtmlChunk::BodyOpen(mid)];
        chunks.extend(initial_doc.inner_html_chunks_with(host_id, &initial_replace).into_iter().map(HtmlChunk::App));

        // Resolve the deferred async resources; Suspense boundaries flip to
        // their bodies as each resource commits.
        crate::spawn::drive_deferred();
        crate::spawn::end_deferred(None);

        if !pending.is_empty() {
            let resolved_doc = self.replay();
            for registration in &boundaries {
                if !pending.contains(&registration.placeholder_id) {
                    continue;
                }
                let html = resolved_doc
                    .node(registration.wrapper_id)
                    .map(|node| node.inner_html_with(&unwrap_suspense))
                    .unwrap_or_default();
                chunks.push(HtmlChunk::PlaceholderPatch {
                    id: registration.placeholder_id.clone(),
                    html,
                });
            }
        }

        // Emit the resolved-resource payload after draining so streamed
        // hydratable resources are included.
        chunks.extend(resource_hydration_chunk());

        chunks.push(HtmlChunk::DocumentEnd(tail));
        chunks
    }

    pub fn render_stream(&self) -> futures::stream::Iter<std::vec::IntoIter<HtmlChunk>> {
        futures::stream::iter(self.rendered_chunks())
    }

    pub fn render_string(&self) -> String {
        self.rendered_chunks().into_iter().map(HtmlChunk::into_string).collect()
    }
}

/// Recursively unwraps streaming Suspense wrappers (`data-glory-suspense`
/// nodes) into their children, so serialized output never leaks the internal
/// `<glory-suspense>` element.
#[cfg(not(target_arch = "wasm32"))]
fn unwrap_suspense(node: &crate::renderer::ssr_dom::SsrNode) -> Option<String> {
    node.get_attribute("data-glory-suspense")?;
    Some(node.inner_html_with(&unwrap_suspense))
}

/// Builds the `<script>window.__gloryResource=…</script>` payload chunk from
/// the resolved [`resource_hydratable_in`](crate::reflow::resource_hydratable_in)
/// values captured during the render, or `None` when none were recorded.
#[cfg(not(target_arch = "wasm32"))]
fn resource_hydration_chunk() -> Option<HtmlChunk> {
    let data = crate::stream_ssr::take_resource_data();
    if data.is_empty() {
        return None;
    }

    let mut object = String::from("{");
    for (index, (token, json)) in data.iter().enumerate() {
        if index > 0 {
            object.push(',');
        }
        // Tokens are framework-generated, but encode them as JSON strings so
        // any future change stays valid; values are already JSON.
        object.push_str(&serde_json::to_string(token).expect("resource token encodes as JSON"));
        object.push(':');
        object.push_str(json);
    }
    object.push('}');

    // Never let an embedded `</script>` terminate the inline script early.
    let safe = object.replace("</", "<\\/");
    Some(HtmlChunk::App(format!(
        r#"<script>window.__gloryResource=Object.assign(window.__gloryResource||{{}},{safe});</script>"#
    )))
}

impl Debug for ServerHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerHolder").finish()
    }
}
impl Drop for ServerHolder {
    fn drop(&mut self) {
        crate::renderer::command::unregister_holder_queue(self.id);
        ROOT_VIEWS.with_borrow_mut(|root_views| {
            root_views.shift_remove(&self.id);
        });

        REVISING_ITEMS.with_borrow_mut(|revising_items| {
            revising_items.shift_remove(&self.id);
        });
        PENDING_ITEMS.with_borrow_mut(|pending_items| {
            pending_items.shift_remove(&self.id);
        });

        RUNNING.with_borrow_mut(|running| {
            running.shift_remove(&self.id);
        });
        BATCHING.with_borrow_mut(|batching| {
            batching.shift_remove(&self.id);
        });
    }
}

impl Holder for ServerHolder {
    fn mount(self, widget: impl Widget) -> Self {
        let _guard = self.queue.make_current();
        // Capture resolved `resource_hydratable_in` values for the hydration
        // payload (both render paths).
        #[cfg(not(target_arch = "wasm32"))]
        crate::stream_ssr::arm_resource_capture();
        // Arm streaming SSR before building so Suspense wraps its region and
        // `resource`/`spawn_local` defers instead of blocking. The deferred
        // futures are drained later by `streaming_chunks`.
        #[cfg(not(target_arch = "wasm32"))]
        if self.streaming {
            crate::stream_ssr::begin();
            let _ = crate::spawn::begin_deferred();
        }
        let view_id = ViewId::new(self.id, self.next_root_view_id.fetch_add(1, Ordering::Relaxed).to_string());
        let scope = Scope::new_root(view_id, self.truck.clone());
        widget.mount_to(scope, self.host_node.node());
        self
    }
    fn enable(self, enabler: impl crate::holder::Enabler + 'static) -> Self {
        // Route handlers may build widgets/nodes; keep them on this
        // holder's queue.
        let _guard = self.queue.make_current();
        enabler.enable(self.truck());
        self
    }
    fn truck(&self) -> Rc<RefCell<Truck>> {
        self.truck.clone()
    }
}

cfg_feature! {
    #![feature = "salvo"]
    use std::convert::Infallible;

    use educe::Educe;
    use futures::StreamExt;
    use salvo::prelude::{Depot, FlowCtrl, Request, Response, Scribe, StatusCode};
    use salvo::{async_trait};

    impl Scribe for ServerHolder {
        fn render(self, res: &mut Response) {
            res.add_header("content-type", "text/html", true).ok();
            res.status_code(StatusCode::OK);
            res.stream(self.render_stream().map(|chunk| Result::<_, Infallible>::Ok(chunk.into_string())));
        }
    }

    #[derive(Clone, Educe)]
    #[educe(Debug)]
    pub struct SalvoHandler {
        pub config: Arc<GloryConfig>,
        #[educe(Debug(ignore))]
        pub holder_factory: Box<Arc<dyn Fn(Arc<GloryConfig>, String) -> ServerHolder +  Sync +Send + 'static>>,
    }

    impl SalvoHandler {
        pub async fn new<H>(holder_factory: H ) -> Self where H: Fn(Arc<GloryConfig>, String) -> ServerHolder + Sync + Send + 'static{
            Self {config: Arc::new(GloryConfig::load(None).await.unwrap()),  holder_factory: Box::new(Arc::new(holder_factory)) }
        }
        pub fn config(mut self, config: impl Into<Arc<GloryConfig>>) -> Self {
            self.config = config.into();
            self
        }
    }

    #[async_trait]
    impl salvo::Handler for SalvoHandler {
        async fn handle(&self, req: &mut Request, _depot: &mut Depot, res: &mut Response, _ctrl: &mut FlowCtrl) {
            let holder = (self.holder_factory)(self.config.clone(), req.uri().to_string());
            res.render(holder);
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use crate::web::widgets::{div, li, ul};
    use crate::{Holder, Scope, Widget};

    use super::*;

    #[derive(Debug)]
    struct StreamWidget;

    impl Widget for StreamWidget {
        fn build(&mut self, ctx: &mut Scope) {
            div().text("streamed").show_in(ctx);
        }
    }

    #[test]
    fn holder_renders_named_html_chunks() {
        let holder = ServerHolder::new(GloryConfig::default(), "/").mount(StreamWidget);
        let chunks = holder.html_chunks();

        assert!(matches!(chunks[0], HtmlChunk::DocumentStart(_)));
        assert!(matches!(chunks[1], HtmlChunk::BodyOpen(_)));
        let app_html = chunks
            .iter()
            .filter_map(|chunk| match chunk {
                HtmlChunk::App(value) => Some(value.as_str()),
                _ => None,
            })
            .collect::<String>();
        assert_eq!(app_html, "<div gly-id=\"0-0\">streamed</div>");
        assert!(matches!(chunks[3], HtmlChunk::DocumentEnd(_)));
        assert!(holder.render_string().contains("streamed"));
    }

    #[derive(Debug)]
    struct NestedStreamWidget;

    impl Widget for NestedStreamWidget {
        fn build(&mut self, ctx: &mut Scope) {
            ul().fill(vec![li().text("a"), li().text("b"), li().text("c")]).show_in(ctx);
        }
    }

    #[test]
    fn render_stream_yields_dom_boundary_chunks() {
        let holder = ServerHolder::new(GloryConfig::default(), "/").mount(NestedStreamWidget);
        let expected = holder.render_string();
        let chunks = holder.html_chunks();
        let app_chunk_count = chunks.iter().filter(|chunk| matches!(chunk, HtmlChunk::App(_))).count();
        assert!(app_chunk_count > 1, "{chunks:?}");
        assert_eq!(chunks.clone().into_iter().map(HtmlChunk::into_string).collect::<String>(), expected);

        let mut stream = holder.render_stream();
        let first = futures::executor::block_on(stream.next()).unwrap();
        let second = futures::executor::block_on(stream.next()).unwrap();
        assert!(matches!(first, HtmlChunk::DocumentStart(_)));
        assert!(matches!(second, HtmlChunk::BodyOpen(_)));
    }

    #[test]
    fn placeholder_chunks_render_marker_and_patch_script() {
        let marker = HtmlChunk::Placeholder {
            id: "user:1".to_string(),
            fallback_html: "<span>Loading</span>".to_string(),
        }
        .into_string();
        let patch = HtmlChunk::PlaceholderPatch {
            id: "user:1".to_string(),
            html: "<strong>Chris</strong>".to_string(),
        }
        .into_string();

        assert_eq!(marker, r#"<template data-glory-placeholder="user:1"><span>Loading</span></template>"#);
        assert!(patch.contains(r#"data-glory-placeholder-patch="user:1""#));
        assert!(patch.contains(r#"patchFromTemplate("user:1")"#));
        assert!(patch.contains("<strong>Chris</strong>"));
    }
}
