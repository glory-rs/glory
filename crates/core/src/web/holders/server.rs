use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::config::GloryConfig;
use crate::reflow::scheduler::{BATCHING, RUNNING};
use crate::reflow::{PENDING_ITEMS, REVISING_ITEMS};
use crate::{Holder, HolderId, ROOT_VIEWS, Scope, Truck, ViewId, Widget};
use crate::web::widgets::*;

const DEPOT_URL_KEY: &str = "glory::url";

pub struct ServerHolder {
    id: HolderId,
    pub config: Arc<GloryConfig>,
    pub truck: Rc<RefCell<Truck>>,
    pub host_node: HtmlDiv,
    next_root_view_id: AtomicU64,
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
        let mut truck = Truck::new();
        truck.insert(DEPOT_URL_KEY, url.into());
        Self {
            id: HolderId::next(),
            config: config.into(),
            truck: Rc::new(RefCell::new(truck)),
            host_node: crate::web::widgets::div(),
            next_root_view_id: AtomicU64::new(0),
        }
    }

    pub fn html_chunks(&self) -> Vec<HtmlChunk> {
        let (head, mid, tail) = crate::web::utils::html_parts_separated(&self.config, &*self.truck.borrow());
        vec![
            HtmlChunk::DocumentStart(head),
            HtmlChunk::BodyOpen(mid),
            HtmlChunk::App(self.host_node.node().inner_html()),
            HtmlChunk::DocumentEnd(tail),
        ]
    }

    pub fn render_stream(&self) -> futures::stream::Iter<std::vec::IntoIter<HtmlChunk>> {
        futures::stream::iter(self.html_chunks())
    }

    pub fn render_string(&self) -> String {
        self.html_chunks().into_iter().map(HtmlChunk::into_string).collect()
    }
}

impl Debug for ServerHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerHolder").finish()
    }
}
impl Drop for ServerHolder {
    fn drop(&mut self) {
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
        let view_id = ViewId::new(self.id, self.next_root_view_id.fetch_add(1, Ordering::Relaxed).to_string());
        let scope = Scope::new_root(view_id, self.truck.clone());
        widget.mount_to(scope, self.host_node.node());
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

    // impl Clone for SalvoHandler {
    //     fn clone(&self) -> Self {
    //         Self {
    //             holder_factory: self.holder_factory.clone(),
    //         }
    //     }
    // }

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
    use crate::web::widgets::div;
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
        assert_eq!(chunks[2], HtmlChunk::App("<div gly-id=\"0-0\">streamed</div>".to_string()));
        assert!(matches!(chunks[3], HtmlChunk::DocumentEnd(_)));
        assert!(holder.render_string().contains("streamed"));
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
