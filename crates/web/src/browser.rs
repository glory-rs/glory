mod dom;
mod events;
mod frame;
#[cfg(feature = "hydrate")]
mod hydrate;
mod rich_text;
#[cfg(feature = "hydrate")]
pub use hydrate::*;

use std::cell::RefCell;
use std::rc::Rc;

use glory_html::Mapper;
use glory_render::profile::Profile;
use glory_render::{
    Action, Capabilities, Error, EventKind, Input, Mutation, NodeId, Query, QueryToken, QueryValue,
    Receipt, Render, RuntimeId,
};
use glory_ui::{App, Widget};
use wasm_bindgen::JsValue;
use web_sys::Element;

use dom::Dom;
use frame::Frame;

fn backend(value: JsValue) -> Error {
    Error::Backend(format!("{value:?}"))
}

/// One DOM interpreter. It owns only nodes created by its semantic mapper.
/// Native callbacks enqueue input; business code runs in the frame host.
pub struct Renderer {
    mapper: Rc<RefCell<Mapper>>,
    dom: Rc<Dom>,
    receipt: Receipt,
    _events: events::Listeners,
}
impl Renderer {
    fn new(runtime: RuntimeId, dom: Rc<Dom>) -> Self {
        let events = events::Listeners::new(dom.clone());
        Self {
            mapper: Rc::new(RefCell::new(Mapper::new(runtime))),
            dom,
            receipt: Receipt::default(),
            _events: events,
        }
    }
}
impl Render for Renderer {
    fn preflight(
        &self,
        node: &glory_render::Descriptor,
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
    fn style_profile(&self) -> glory_render::style::Profile {
        glory_render::style::Profile::HTML
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            measure: true,
            scroll: true,
            focus: true,
            input: true,
        }
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        if let Some(error) = self.dom.error.borrow().clone() {
            return Err(error);
        }
        let mut mapper = self.mapper.borrow_mut();
        let result = mapper.apply(mutations, &mut dom::Sink(self.dom.clone()));
        self.receipt = mapper.receipt();
        result
    }
    fn listen(&mut self, node: NodeId, kind: EventKind, enabled: bool) -> Result<(), Error> {
        let mapper = self.mapper.borrow();
        let descriptor = &mapper.tree().node(node)?.descriptor;
        if enabled {
            kind.validate_target(descriptor)?;
        }
        self.dom.listen(node, kind, enabled)
    }
    fn action(&mut self, action: Action) -> Result<(), Error> {
        action.validate_target(&self.mapper.borrow().tree().node(action.node())?.descriptor)?;
        if self.dom.hold_actions.get() {
            self.dom.actions.borrow_mut().push(action);
            return Ok(());
        }
        perform(&self.dom, action)
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        self.mapper.borrow().tree().node(query.node())?;
        if self.dom.staging.get() {
            return Err(Error::Unsupported("physical query before adoption"));
        }
        let result = (|| {
            query.validate_target(&self.mapper.borrow().tree().node(query.node())?.descriptor)?;
            let entry = self.dom.entry(query.node())?;
            if !entry.first.is_connected() {
                return Err(Error::Unsupported("query on a disconnected range"));
            }
            match query {
                Query::Focus(_) => Ok(QueryValue::Focus(dom::has_focus(&entry.control()?)?)),
                Query::Bounds(_) => {
                    let rect = entry.element()?.get_bounding_client_rect();
                    Ok(QueryValue::Bounds(dom::rect(&rect)))
                }
                Query::Scroll(_) => {
                    let element = entry.element()?;
                    Ok(QueryValue::Scroll {
                        x: dom::number(&element, "scrollLeft"),
                        y: dom::number(&element, "scrollTop"),
                    })
                }
                Query::RichSelection(_) => Ok(QueryValue::RichSelection(rich_text::selection(
                    &entry.control()?,
                )?)),
                Query::Selection(_) => {
                    let element = entry.control()?;
                    let start = js_sys::Reflect::get(&element, &"selectionStart".into())
                        .map_err(backend)?
                        .as_f64()
                        .ok_or(Error::Unsupported("selection on this node"))?;
                    let end = js_sys::Reflect::get(&element, &"selectionEnd".into())
                        .map_err(backend)?
                        .as_f64()
                        .ok_or(Error::Unsupported("selection on this node"))?;
                    Ok(QueryValue::Selection {
                        start: start as usize,
                        end: end as usize,
                    })
                }
            }
        })();
        self.dom.inputs.borrow_mut().push(Input::Response {
            token,
            applied: self.receipt.applied,
            result,
        });
        self.dom.frame.input();
        Ok(())
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        self.dom.read_layout();
        inputs.append(&mut self.dom.inputs.borrow_mut());
    }
}
fn perform(dom: &Dom, action: Action) -> Result<(), Error> {
    match action {
        Action::Focus(node) => dom::focus(&dom.entry(node)?.control()?)?,
        Action::RichMarks { node, marks } => {
            rich_text::set_marks(&dom.entry(node)?.control()?, &marks)?
        }
        Action::RichSelection { node, selection } => {
            rich_text::set_selection(&dom.entry(node)?.control()?, selection)?
        }
        Action::ScrollTo { node, x, y } => {
            let entry = dom.entry(node)?;
            let element = entry.element()?;
            js_sys::Reflect::set(&element, &"scrollLeft".into(), &x.into()).map_err(backend)?;
            js_sys::Reflect::set(&element, &"scrollTop".into(), &y.into()).map_err(backend)?;
            // The setter already needs layout to clamp the scroll. Read back
            // its accepted offsets here; a numeric tolerance cannot distinguish
            // browser quantization from a later genuine one-pixel user move.
            entry.scroll_request.set(Some(glory_render::ScrollAck {
                requested: glory_render::ScrollRequest { x, y },
                x: dom::number(&element, "scrollLeft"),
                y: dom::number(&element, "scrollTop"),
            }));
            dom.layout.borrow_mut().insert((node, EventKind::Scroll));
            dom.frame.input();
        }
    }
    Ok(())
}
impl Drop for Renderer {
    fn drop(&mut self) {
        self.dom.clear();
    }
}

/// Keep this guard alive for the application lifetime. Dropping it releases the
/// DOM, observers, native listeners, animation frame and owned async tasks.
pub struct WebApp {
    app: Rc<App>,
    frame: Rc<Frame>,
}
impl WebApp {
    /// Use this app's browser Performance clock, never std::time::Instant on WASM.
    pub fn enable_profile(&self) -> Result<Profile, Error> {
        let profile = self.frame.profile()?;
        self.app.set_profile(Some(profile.clone()));
        Ok(profile)
    }
    pub fn app(&self) -> &App {
        &self.app
    }
}
impl Drop for WebApp {
    fn drop(&mut self) {
        self.frame.stop();
    }
}

/// Mount a fresh app into an empty element. SSR adoption uses a separate entry
/// point so accidentally replacing server markup is never implicit.
pub fn launch(root: Element, widget: impl Widget) -> glory_ui::Result<WebApp> {
    launch_with(root, move |_| widget)
}
pub fn launch_with<W: Widget>(
    root: Element,
    factory: impl FnOnce(&glory_ui::Scope) -> W,
) -> glory_ui::Result<WebApp> {
    if root.has_child_nodes() || root.has_attribute("data-gly-app") {
        return Err(Error::InvalidProps("web mount requires an empty, unclaimed element").into());
    }
    let document = root
        .owner_document()
        .ok_or(Error::Unsupported("mount has no document"))?;
    let window = document
        .default_view()
        .ok_or(Error::Unsupported("document has no window"))?;
    base_style(&document)?;
    let frame = Frame::new(window);
    let dom = Dom::new(document, root, frame.clone());
    let app = Rc::new(App::with_host(frame.clone(), |runtime| {
        Renderer::new(runtime, dom)
    }));
    frame.attach(&app);
    let guard = WebApp { app, frame };
    guard.app.mount_with(|| factory(guard.app.scope()))?;
    Ok(guard)
}
fn base_style(document: &web_sys::Document) -> Result<(), Error> {
    if document
        .query_selector("style[data-gly-base=\"v1\"]")
        .map_err(backend)?
        .is_none()
    {
        let style = document.create_element("style").map_err(backend)?;
        style
            .set_attribute("data-gly-base", "v1")
            .map_err(backend)?;
        style.set_text_content(Some(glory_html::BASE_CSS));
        document
            .query_selector("head")
            .map_err(backend)?
            .ok_or(Error::Unsupported("document has no head"))?
            .append_child(&style)
            .map_err(backend)?;
    }
    Ok(())
}
