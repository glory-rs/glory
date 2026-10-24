//! Pull-driven SSR. The first frame is available before any loader is polled;
//! demand polls ready local tasks and commits incremental HTML edits. Dropping
//! the body cancels its App and futures. HTTP/DOM bootstrap is an outer adapter.
mod edits;
mod host;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use glory_html::Mapper;
use glory_render::{Capabilities, Input, Mutation, Query, QueryToken, Receipt, Render, RuntimeId};
use glory_resource::Resources;
use glory_ui::{App, Scope, Widget};
use serde_json::json;

#[derive(Clone, Debug)]
pub enum Error {
    Ui(glory_ui::Error),
    Resources(glory_resource::Error),
    Json(String),
    Limit(&'static str),
    Config,
}
impl From<glory_ui::Error> for Error {
    fn from(error: glory_ui::Error) -> Self {
        Self::Ui(error)
    }
}
impl From<glory_resource::Error> for Error {
    fn from(error: glory_resource::Error) -> Self {
        Self::Resources(error)
    }
}
impl From<glory_render::Error> for Error {
    fn from(error: glory_render::Error) -> Self {
        Self::Ui(error.into())
    }
}
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub chunk_bytes: usize,
    pub tasks: usize,
    pub polls_per_tick: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            chunk_bytes: 8 * 1024 * 1024,
            tasks: 4096,
            polls_per_tick: 64,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    Shell,
    Patch,
    End,
}
/// Shell body is a pending stream HTML fragment, deliberately not yet adoptable
/// by web::hydrate. Patch/End bodies are versioned JSON
/// envelopes; they never contain RuntimeId/NodeId. The receiver applies edits,
/// then relabels keys with End.rekey before ordinary final-state hydration.
#[derive(Clone, Debug)]
pub struct Frame {
    kind: FrameKind,
    sequence: u64,
    body: String,
    initial: Option<String>,
    limit: usize,
}
impl Frame {
    pub fn kind(&self) -> FrameKind {
        self.kind
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn body(&self) -> &str {
        &self.body
    }
    /// Parser-blocking external bootstrap executes after its complete inert
    /// data block. Serve glory_web::STREAM_SCRIPT at script_url; no inline eval.
    /// Emit Bootstrap::head_script before the shell to capture early editing.
    pub fn to_html(&self, bootstrap: &Bootstrap) -> Result<String, Error> {
        bootstrap.validate()?;
        if self.sequence > 9_007_199_254_740_991 {
            return Err(Error::Limit("HTML frame sequence"));
        }
        let data: std::borrow::Cow<'_, str> = if self.kind == FrameKind::Shell {
            // The manifest is already trusted serialized JSON. Avoid parsing
            // and rebuilding its complete node tree only to wrap it for HTML.
            std::borrow::Cow::Owned(format!(
                "{{\"version\":2,\"kind\":\"start\",\"sequence\":0,\"data_id\":{},\"nodes\":{}}}",
                serde_json::to_string(&bootstrap.data_id)?,
                self.initial.as_deref().expect("shell manifest")
            ))
        } else {
            std::borrow::Cow::Borrowed(&self.body)
        };
        let nonce = bootstrap
            .nonce
            .as_ref()
            .map(|value| format!(" nonce=\"{}\"", glory_html::escape(value)))
            .unwrap_or_default();
        let root = glory_html::escape(&bootstrap.root_id);
        let block = format!(
            "<script type=\"application/json\" data-gly-stream=\"{root}\"{nonce}>{}</script><script src=\"{}\"{nonce}></script>",
            glory_html::escape_script_data(&data),
            glory_html::escape(&bootstrap.script_url)
        );
        let html = if self.kind == FrameKind::Shell {
            format!(
                "<div id=\"{root}\" data-gly-stream-pending=\"\">{}</div>{block}",
                self.body
            )
        } else {
            block
        };
        if html.len() > self.limit || html.len() > 8 * 1024 * 1024 {
            return Err(Error::Limit("HTML frame bytes"));
        }
        Ok(html)
    }
}
#[derive(Clone, Debug)]
pub struct Bootstrap {
    pub root_id: String,
    pub data_id: String,
    pub script_url: String,
    pub nonce: Option<String>,
}
impl Bootstrap {
    fn validate(&self) -> Result<(), Error> {
        if self.root_id.is_empty()
            || self.data_id.is_empty()
            || self.root_id == self.data_id
            || self.script_url.is_empty()
        {
            Err(Error::Config)
        } else {
            Ok(())
        }
    }
    /// Install the shared preload event hub before parsing editable shell DOM.
    /// The HTTP host supplies script-src/nonce policy and serves STREAM_SCRIPT.
    pub fn head_script(&self) -> Result<String, Error> {
        self.validate()?;
        let nonce = self
            .nonce
            .as_ref()
            .map(|value| format!(" nonce=\"{}\"", glory_html::escape(value)))
            .unwrap_or_default();
        Ok(format!(
            "<script src=\"{}\"{nonce}></script>",
            glory_html::escape(&self.script_url)
        ))
    }
}
struct Renderer {
    mapper: Rc<RefCell<Mapper>>,
    edits: Rc<RefCell<edits::Edits>>,
    inputs: Vec<Input>,
    receipt: Receipt,
}
impl Render for Renderer {
    fn preflight(
        &self,
        node: glory_render::DescriptorView<'_>,
        patch: Option<&glory_render::Patch>,
    ) -> Result<(), glory_render::Error> {
        glory_html::preflight(node, patch)
    }
    fn preflight_place(
        &self,
        tree: &glory_render::memory::Memory,
        child: glory_render::memory::Subtree<'_>,
        parent: Option<glory_render::NodeId>,
    ) -> Result<(), glory_render::Error> {
        glory_html::preflight_place(tree, child, parent)
    }
    fn style_profile(&self) -> glory_render::style::Profile {
        glory_render::style::Profile::HTML
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
    fn listen(
        &mut self,
        node: glory_render::NodeId,
        kind: glory_render::EventKind,
        enabled: bool,
    ) -> Result<(), glory_render::Error> {
        let mapper = self.mapper.borrow();
        let descriptor = mapper.tree().node(node)?.descriptor.view();
        if enabled {
            kind.validate_target(descriptor)?;
        }
        Ok(())
    }
    fn action(&mut self, action: glory_render::Action) -> Result<(), glory_render::Error> {
        action.validate_target(&self.mapper.borrow().tree().node(action.node())?.descriptor)?;
        Err(glory_render::Error::Unsupported(
            "SSR has no physical actions",
        ))
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, glory_render::Error> {
        let mut mapper = self.mapper.borrow_mut();
        let result = mapper.apply(mutations, &mut *self.edits.borrow_mut());
        self.receipt = mapper.receipt();
        result
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), glory_render::Error> {
        let result = query
            .validate_target(&self.mapper.borrow().tree().node(query.node())?.descriptor)
            .and(Err(glory_render::Error::Unsupported(
                "SSR has no physical geometry, selection or focus",
            )));
        self.inputs.push(Input::Response {
            token,
            applied: self.receipt.applied,
            result,
        });
        Ok(())
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        inputs.append(&mut self.inputs);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub tasks: usize,
    pub frames: u64,
    pub nodes: usize,
}
/// A local (!Send) request body. It owns the entire UI/request lifetime; keep
/// polling on the host's local executor. There is no eager producer or channel.
pub struct Stream {
    app: Option<App>,
    mapper: Option<Rc<RefCell<Mapper>>>,
    edits: Rc<RefCell<edits::Edits>>,
    executor: Rc<host::Executor>,
    resources: Resources,
    shell: Option<String>,
    initial: Option<String>,
    sequence: u64,
    config: Config,
    done: bool,
}
pub fn stream_with<W: Widget>(
    resources: Resources,
    config: Config,
    factory: impl FnOnce(&Scope, &Resources) -> W,
) -> Result<Stream, Error> {
    if config.chunk_bytes == 0 || config.tasks == 0 || config.polls_per_tick == 0 {
        return Err(Error::Config);
    }
    let executor = Rc::new(host::Executor::new(config.tasks));
    let edits = Rc::new(RefCell::new(edits::Edits::new(config.chunk_bytes)));
    let mut mapper = None;
    let app = App::with_host(executor.clone(), |runtime: RuntimeId| {
        let model = Rc::new(RefCell::new(Mapper::new(runtime)));
        mapper = Some(model.clone());
        Renderer {
            mapper: model,
            edits: edits.clone(),
            receipt: Receipt::default(),
            inputs: Vec::new(),
        }
    });
    app.mount_with(|| factory(app.scope(), &resources))?;
    let mapper = mapper.expect("renderer created");
    let shell = format!(
        "<!--gly:stream:2-->{}<!--gly:/stream:2-->",
        mapper.borrow().html()?
    );
    let initial = {
        let model = mapper.borrow();
        let mut ids = edits.borrow().ids();
        ids.sort_by_key(|id| model.key(*id));
        let nodes = ids.into_iter().map(|id| {
            let node = model.tree().node(id)?;
            Ok(json!({"key":model.key(id).expect("key").to_string(),"shape":edits::shape(glory_html::shape(&node.descriptor)?),"parent":node.parent.and_then(|id|model.key(id)).map(|key|key.to_string()),"children":node.children.iter().map(|id|model.key(*id).expect("key").to_string()).collect::<Vec<_>>(),"placed":node.placed,"suspended":node.suspended}))
        }).collect::<Result<Vec<_>,glory_render::Error>>()?;
        serde_json::to_string(&nodes)?
    };
    if shell.len().saturating_add(initial.len()) > config.chunk_bytes {
        return Err(Error::Limit("shell bytes"));
    }
    edits.borrow_mut().recording = true;
    Ok(Stream {
        app: Some(app),
        mapper: Some(mapper),
        edits,
        executor,
        resources,
        shell: Some(shell),
        initial: Some(initial),
        sequence: 0,
        config,
        done: false,
    })
}
impl Stream {
    pub fn stats(&self) -> Stats {
        Stats {
            tasks: self.executor.len(),
            frames: self.sequence,
            nodes: self.app.as_ref().map_or(0, |app| app.stats().nodes),
        }
    }
    pub fn cancel(&mut self) {
        self.done = true;
        self.shell = None;
        self.initial = None;
        drop(self.app.take());
        drop(self.mapper.take());
        self.executor.close();
        self.edits.borrow_mut().clear();
        self.resources.discard_unused();
    }
    pub async fn next_frame(&mut self) -> Option<Result<Frame, Error>> {
        futures_util::future::poll_fn(|context| Pin::new(&mut *self).poll_frame(context)).await
    }
    fn frame(&mut self, kind: FrameKind, body: String) -> Result<Frame, Error> {
        if body.len() > self.config.chunk_bytes {
            return Err(Error::Limit("frame bytes"));
        }
        let sequence = self.sequence;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(Error::Limit("frame sequence"))?;
        let initial = if kind == FrameKind::Shell {
            self.initial.take()
        } else {
            None
        };
        Ok(Frame {
            kind,
            body,
            sequence,
            initial,
            limit: self.config.chunk_bytes,
        })
    }
    fn advance(&mut self, context: &mut Context<'_>) -> Poll<Result<Frame, Error>> {
        self.executor.observe(context.waker());
        if let Some(shell) = self.shell.take() {
            return Poll::Ready(self.frame(FrameKind::Shell, shell));
        }
        for _ in 0..self.config.polls_per_tick {
            let app = self.app.as_ref().expect("active stream");
            if self.executor.dirty() {
                app.flush()?;
            }
            let edits = self.edits.borrow_mut().take();
            if !edits.is_empty() {
                let body = serde_json::to_string(
                    &json!({"version":2,"kind":"patch","sequence":self.sequence,"ops":edits}),
                )?;
                return Poll::Ready(self.frame(FrameKind::Patch, body));
            }
            if app.runtime_stats().tasks == 0 && self.executor.is_empty() {
                let resources = self.resources.snapshot()?.to_json()?;
                let pairs = self
                    .mapper
                    .as_ref()
                    .expect("mapper")
                    .borrow_mut()
                    .canonicalize()?;
                let pairs: Vec<_> = pairs
                    .into_iter()
                    .map(|(old, new)| [old.to_string(), new.to_string()])
                    .collect();
                let body = format!(
                    "{{\"version\":2,\"kind\":\"end\",\"sequence\":{},\"html_version\":{},\"rekey\":{},\"resources\":{resources}}}",
                    self.sequence,
                    glory_html::FORMAT_VERSION,
                    serde_json::to_string(&pairs)?
                );
                let frame = self.frame(FrameKind::End, body);
                self.cancel();
                return Poll::Ready(frame);
            }
            if !self.executor.poll_one() {
                return Poll::Pending;
            }
        }
        // Yield a busy local producer fairly. No future is polled while the
        // consumer is waiting for a network write or otherwise not demanding.
        if self.executor.work() {
            context.waker().wake_by_ref();
        }
        Poll::Pending
    }
    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame, Error>>> {
        if self.done {
            return Poll::Ready(None);
        }
        match self.advance(context) {
            Poll::Ready(Err(error)) => {
                self.cancel();
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(Ok(frame)) => Poll::Ready(Some(Ok(frame))),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl futures_core::Stream for Stream {
    type Item = Result<Frame, Error>;
    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_frame(context)
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.cancel();
    }
}
