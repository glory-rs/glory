//! Synchronous HTML snapshots using the same mapper as the browser backend.
//! Request resources, hydration and streaming build on this model separately.
use std::cell::RefCell;
use std::rc::Rc;

use glory_html::{Mapper, Op, Sink};
use glory_render::{
    Action, Capabilities, Error, EventKind, Input, Mutation, Query, QueryToken, Receipt, Render,
    RuntimeId,
};
use glory_ui::{App, Widget};

#[cfg(feature = "resources")]
mod resources;
#[cfg(feature = "resources")]
pub use resources::*;
#[cfg(all(feature = "stream", not(target_arch = "wasm32")))]
pub mod stream;
#[cfg(all(feature = "stream", not(target_arch = "wasm32")))]
pub use stream::stream_with;

struct Discard;
impl Sink for Discard {
    fn apply(&mut self, _op: Op) -> Result<(), Error> {
        Ok(())
    }
}
struct State {
    mapper: Mapper,
    receipt: Receipt,
    inputs: Vec<Input>,
}
#[derive(Clone)]
pub struct Renderer(Rc<RefCell<State>>);
impl Renderer {
    pub fn new(runtime: RuntimeId) -> Self {
        Self(Rc::new(RefCell::new(State {
            mapper: Mapper::new(runtime),
            receipt: Receipt::default(),
            inputs: Vec::new(),
        })))
    }
    pub fn html(&self) -> Result<String, Error> {
        self.0.borrow().mapper.html()
    }
    pub fn snapshot(&self) -> Result<String, Error> {
        let mut state = self.0.borrow_mut();
        state.mapper.canonicalize()?;
        state.mapper.html()
    }
}
impl Render for Renderer {
    fn preflight(
        &self,
        node: glory_render::DescriptorView<'_>,
        patch: Option<&glory_render::Patch>,
    ) -> Result<(), Error> {
        glory_html::preflight(node, patch)
    }
    fn preflight_place(
        &self,
        tree: &glory_render::memory::Memory,
        child: glory_render::memory::Subtree<'_>,
        parent: Option<glory_render::NodeId>,
    ) -> Result<(), Error> {
        glory_html::preflight_place(tree, child, parent)
    }
    fn listen(
        &mut self,
        node: glory_render::NodeId,
        kind: EventKind,
        enabled: bool,
    ) -> Result<(), Error> {
        let state = self.0.borrow();
        let descriptor = state.mapper.tree().node(node)?.descriptor.view();
        if enabled {
            kind.validate_target(descriptor)?;
        }
        Ok(())
    }
    fn action(&mut self, action: Action) -> Result<(), Error> {
        self.0
            .borrow()
            .mapper
            .tree()
            .validate_clipboard_action(action)?;
        action.validate_target(
            &self
                .0
                .borrow()
                .mapper
                .tree()
                .node(action.node())?
                .descriptor,
        )?;
        Err(Error::Unsupported("SSR has no physical actions"))
    }
    fn style_profile(&self) -> glory_render::style::Profile {
        glory_render::style::Profile::HTML
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        let mut state = self.0.borrow_mut();
        let result = state.mapper.apply(mutations, &mut Discard);
        state.receipt = state.mapper.receipt();
        result
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        let result = query
            .validate_target(&state.mapper.tree().node(query.node())?.descriptor)
            .and(Err(Error::Unsupported(
                "SSR has no physical geometry, selection or focus",
            )));
        let applied = state.receipt.applied;
        state.inputs.push(Input::Response {
            token,
            applied,
            result,
        });
        Ok(())
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        inputs.append(&mut self.0.borrow_mut().inputs);
    }
}

pub fn render(widget: impl Widget) -> glory_ui::Result<String> {
    render_with(move |_| widget)
}
pub fn render_with<W: Widget>(
    factory: impl FnOnce(&glory_ui::Scope) -> W,
) -> glory_ui::Result<String> {
    let (_app, output) = mount_with(factory)?;
    Ok(output.snapshot()?)
}
fn mount_with<W: Widget>(
    factory: impl FnOnce(&glory_ui::Scope) -> W,
) -> glory_ui::Result<(App, Renderer)> {
    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        output = Some(renderer.clone());
        renderer
    });
    app.mount_with(|| factory(app.scope()))?;
    Ok((app, output.expect("created renderer")))
}

/// Insert this versioned fragment verbatim into an exclusively owned mount.
/// Request resource values use a separate, explicitly serialized payload.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub html: String,
}
pub fn snapshot(widget: impl Widget) -> glory_ui::Result<Snapshot> {
    snapshot_with(move |_| widget)
}
pub fn snapshot_with<W: Widget>(
    factory: impl FnOnce(&glory_ui::Scope) -> W,
) -> glory_ui::Result<Snapshot> {
    let body = render_with(factory)?;
    Ok(fragment(body))
}
fn fragment(body: String) -> Snapshot {
    Snapshot {
        html: format!(
            "<!--gly:v:{}-->{body}<!--gly:/v:{}-->",
            glory_html::FORMAT_VERSION,
            glory_html::FORMAT_VERSION
        ),
    }
}

#[derive(Clone, Debug, Default)]
pub struct Document {
    pub title: String,
    pub lang: Option<String>,
    pub stylesheets: Vec<String>,
    #[cfg(feature = "css")]
    pub sheets: Vec<glory_html::Sheet>,
    pub nonce: Option<String>,
}
pub fn render_document(widget: impl Widget, document: &Document) -> glory_ui::Result<String> {
    // Validate all sheets before constructing widgets or executing business
    // factories. Compiling assets must not partially mount a failed request.
    #[cfg(feature = "css")]
    let css = document
        .sheets
        .iter()
        .map(|value| glory_html::sheet(value, glory_render::style::Profile::HTML))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| Error::Backend(err.to_string()))?;
    let body = render(widget)?;
    let mut html = String::from("<!doctype html><html");
    if let Some(lang) = &document.lang {
        html.push_str(&format!(" lang=\"{}\"", glory_html::escape(lang)));
    }
    let nonce = document
        .nonce
        .as_ref()
        .map(|value| format!(" nonce=\"{}\"", glory_html::escape(value)))
        .unwrap_or_default();
    html.push_str(&format!(
        "><head><meta charset=\"utf-8\"><title>{}</title><style data-gly-base=\"v1\"{nonce}>{}</style>",
        glory_html::escape(&document.title),
        glory_html::BASE_CSS
    ));
    let mut seen = std::collections::HashSet::new();
    #[cfg(feature = "css")]
    for value in &css {
        html.push_str(&format!(
            "<style data-gly-sheet=\"v1\"{nonce}>{value}</style>"
        ));
    }
    for href in document
        .stylesheets
        .iter()
        .filter(|href| seen.insert(href.as_str()))
    {
        html.push_str(&format!(
            "<link rel=\"stylesheet\" href=\"{}\">",
            glory_html::escape(href)
        ));
    }
    html.push_str("</head><body>");
    html.push_str(&body);
    html.push_str("</body></html>");
    Ok(html)
}
