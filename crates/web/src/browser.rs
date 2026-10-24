mod clipboard;
mod collapse;
mod dom;
mod events;
mod file_document;
mod files;
mod frame;
#[cfg(feature = "hydrate")]
mod hydrate;
mod images;
mod input_edit;
mod layers;
mod rich;
mod scroll_into_view;
mod select;
mod sheet;
mod swipe;
mod translation;
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
use wasm_bindgen::{JsCast, JsValue};
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
    supports_layers: bool,
    supports_translations: bool,
    supports_files: bool,
}
impl Renderer {
    fn new(runtime: RuntimeId, dom: Rc<Dom>) -> Self {
        if let Err(error) = dom.configure_collapse() {
            dom.error.borrow_mut().get_or_insert(error);
        }
        let supports_layers =
            js_sys::Reflect::get(&dom.root, &"showPopover".into()).is_ok_and(|v| v.is_function());
        let supports_translations =
            js_sys::Reflect::get(&dom.root, &"animate".into()).is_ok_and(|v| v.is_function());
        let supports_files = dom
            .document
            .create_element("input")
            .ok()
            .is_some_and(|element| {
                js_sys::Reflect::get(&element, &"showPicker".into())
                    .is_ok_and(|method| method.is_function())
            });
        dom.frame.attach_dom(&dom);
        let events = events::Listeners::new(dom.clone());
        Self {
            mapper: Rc::new(RefCell::new(Mapper::new(runtime))),
            dom,
            receipt: Receipt::default(),
            _events: events,
            supports_layers,
            supports_translations,
            supports_files,
        }
    }
    fn check_layers<'a>(
        &self,
        node: impl Into<glory_render::DescriptorView<'a>>,
        patch: Option<&glory_render::Patch>,
    ) -> Result<(), Error> {
        let node = node.into();
        use glory_render::{Change, Patch};
        let value = match patch {
            None => node.semantics.as_deref(),
            Some(Patch::Semantics(Change::Set(value))) => value.as_deref(),
            _ => None,
        };
        if !self.supports_translations && value.is_some_and(|s| s.translation.is_some()) {
            return Err(Error::Unsupported("browser finite translation motion"));
        }
        if self.supports_layers {
            return Ok(());
        }
        if value.is_some_and(|s| s.layer.is_some()) {
            return Err(Error::Unsupported("browser top layer"));
        }
        Ok(())
    }
}
impl Render for Renderer {
    fn preflight(
        &self,
        node: glory_render::DescriptorView<'_>,
        patch: Option<&glory_render::Patch>,
    ) -> Result<(), Error> {
        self.check_layers(node, patch)?;
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
            clipboard_write: true,
            files: Some(glory_render::FileCapabilities {
                document: self.dom.files.borrow().document,
                picker: self.supports_files,
                drop: true,
                read: true,
                preview: true,
            }),
            measure: true,
            scroll: true,
            focus: true,
            input: true,
            collapse_completion: true,
        }
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        if let Some(error) = self.dom.error.borrow().clone() {
            return Err(error);
        }
        let mut mapper = self.mapper.borrow_mut();
        let result = if self.supports_layers && self.supports_translations {
            mapper.apply(mutations, &mut dom::Sink(self.dom.clone()))
        } else {
            // Legacy hosts reject per operation and retain accepted prefixes.
            (|| {
                for mutation in mutations {
                    match mutation {
                        Mutation::CreatePlan { plan, .. } => {
                            for node in plan.nodes() {
                                self.check_layers(&node.descriptor, None)?;
                            }
                        }
                        Mutation::Create { node, .. } => self.check_layers(node, None)?,
                        Mutation::Patch { id, patch } => {
                            self.check_layers(&mapper.tree().node(*id)?.descriptor, Some(patch))?
                        }
                        _ => {}
                    }
                    mapper.apply(
                        std::slice::from_ref(mutation),
                        &mut dom::Sink(self.dom.clone()),
                    )?;
                }
                Ok(mapper.receipt())
            })()
        };
        self.receipt = mapper.receipt();
        swipe::update(&self.dom, mapper.tree());
        self.dom
            .sheets
            .borrow_mut()
            .update(mapper.tree(), mutations);
        if self.dom.sheets.borrow().needs_frame() {
            self.dom.frame.input();
        }
        self.dom
            .layers
            .borrow_mut()
            .update(mapper.tree(), mutations);
        self.dom.update_collapses(mapper.tree(), mutations);
        self.dom.update_translations(mapper.tree(), mutations);
        self.dom.update_clipboard(mapper.tree());
        self.dom.update_files(mapper.tree());
        rich::update(&self.dom, mapper.tree())?;
        drop(mapper);
        if !self.dom.staging.get() && self.dom.live.get() {
            sheet::sync_accepted(&self.dom)?;
            layers::sync(&self.dom)?;
        }
        result
    }
    fn listen(&mut self, node: NodeId, kind: EventKind, enabled: bool) -> Result<(), Error> {
        let mapper = self.mapper.borrow();
        let descriptor = mapper.tree().node(node)?.descriptor.view();
        if enabled {
            kind.validate_target(descriptor)?;
            if kind == EventKind::CollapseComplete {
                self.dom.admit_collapse(mapper.tree(), node);
            }
        }
        self.dom.listen(node, kind, enabled)
    }
    fn action(&mut self, action: Action) -> Result<(), Error> {
        action.validate_target(&self.mapper.borrow().tree().node(action.node())?.descriptor)?;
        if matches!(
            action,
            Action::PickFiles { .. } | Action::ClearFiles { .. } | Action::RemoveFile(_)
        ) {
            let mut mapper = self.mapper.borrow_mut();
            mapper.tree().validate_file_action(action)?;
            if self.dom.staging.get() || self.dom.hold_actions.get() || !self.dom.live.get() {
                return Err(Error::Unsupported("file action before adoption"));
            }
            if matches!(action, Action::PickFiles { .. })
                && !self
                    .dom
                    .document_layers
                    .allows(Some(self.dom.entry(action.node())?.control()?.as_ref()))
            {
                return Err(Error::Unsupported(
                    "file control outside document modal scope",
                ));
            }
            mapper.admit_file_action(action)?;
            match action {
                Action::PickFiles {
                    node,
                    revision,
                    request,
                } => {
                    drop(mapper);
                    if let Err(reason) = self.dom.pick_files(node, revision, request) {
                        self.dom.reject_picker(
                            &mut self.mapper.borrow_mut(),
                            node,
                            revision,
                            request,
                            reason,
                        )?;
                    }
                }
                Action::ClearFiles { node, .. } => {
                    self.dom.clear_file_control(node);
                }
                Action::RemoveFile(_) => self.dom.update_files(mapper.tree()),
                _ => unreachable!(),
            }
            return Ok(());
        }
        if let Action::CopyText {
            node,
            revision,
            request,
        } = action
        {
            let mut mapper = self.mapper.borrow_mut();
            mapper.tree().validate_clipboard_action(action)?;
            if self.dom.staging.get() || self.dom.hold_actions.get() || !self.dom.live.get() {
                return Err(Error::Unsupported("clipboard write before adoption"));
            }
            let target = self.dom.entry(node)?.control()?;
            if !self.dom.document_layers.allows(Some(target.as_ref())) {
                return Err(Error::Unsupported(
                    "clipboard source outside document modal scope",
                ));
            }
            mapper.admit_clipboard(action)?;
            let text = mapper
                .tree()
                .node(node)?
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.clipboard.as_ref())
                .expect("validated source")
                .text
                .clone();
            drop(mapper);
            self.dom.write_clipboard(node, revision, request, text);
            return Ok(());
        }
        if matches!(action, Action::RaiseLayer(_))
            && (!self.mapper.borrow().tree().active(action.node())
                || !self.mapper.borrow().tree().enabled(action.node()))
        {
            return Err(Error::Unsupported("raise on an inactive layer"));
        }
        if let Action::SelectRich { node, .. } = action {
            let mapper = self.mapper.borrow();
            let glory_render::Props::RichEditor(expected) =
                mapper.tree().node(node)?.descriptor.view().props
            else {
                unreachable!()
            };
            let (actual, _) = rich::read(&self.dom.entry(node)?.control()?)?;
            if actual != expected.document {
                return Err(Error::Unsupported("rich selection document changed"));
            }
        }
        if let Action::SelectText { node, .. } = action {
            let mapper = self.mapper.borrow();
            let glory_render::Props::Input(expected) =
                mapper.tree().node(node)?.descriptor.view().props
            else {
                unreachable!()
            };
            let target = self.dom.entry(node)?.control()?;
            let current = js_sys::Reflect::get(&target, &"value".into()).map_err(backend)?;
            if !current.is_string()
                || js_sys::JsString::from(current.clone()).length() as usize != expected.value.len()
                || current.as_string().as_deref() != Some(expected.value.as_str())
            {
                return Err(Error::Unsupported("text selection value changed"));
            }
        }
        if matches!(
            action,
            Action::Focus(_)
                | Action::RaiseLayer(_)
                | Action::ScrollIntoView(_)
                | Action::ExitMenu { .. }
                | Action::SelectRich { .. }
        ) && self.mapper.borrow().tree().inert(action.node())
        {
            return Err(Error::Unsupported("focus in an inert region"));
        }
        if self.dom.hold_actions.get() {
            self.dom.actions.borrow_mut().push(action);
            return Ok(());
        }
        if let Action::RevealOption { node } = action
            && (!self.mapper.borrow().tree().active(node)
                || !self.mapper.borrow().tree().enabled(node))
        {
            return Ok(());
        }
        perform(&self.dom, action)
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        if matches!(query, Query::FileRead(_) | Query::FilePreview(_)) {
            let result = self.dom.request_file(
                self.mapper.borrow().tree(),
                query,
                token,
                self.receipt.applied,
            );
            if let Err(error) = result {
                self.dom.inputs.borrow_mut().push(Input::Response {
                    token,
                    applied: self.receipt.applied,
                    result: Err(error),
                });
                self.dom.frame.input();
            }
            return Ok(());
        }

        self.mapper.borrow().tree().node(query.node())?;
        if self.dom.staging.get() {
            return Err(Error::Unsupported("physical query before adoption"));
        }
        let result = (|| {
            self.dom.read_translations(self.mapper.borrow().tree())?;
            query.validate_target(&self.mapper.borrow().tree().node(query.node())?.descriptor)?;
            let entry = self.dom.entry(query.node())?;
            if !entry.first.is_connected() {
                return Err(Error::Unsupported("query on a disconnected range"));
            }
            match query {
                Query::RichSelection(_) => {
                    let control = entry.control()?;
                    let (document, blocks) = rich::read(&control)?;
                    let selection = rich::selection(&control, &document, &blocks)?;
                    let editing = entry.editing.borrow();
                    Ok(QueryValue::RichSelection(Box::new(
                        glory_render::RichSnapshot {
                            document,
                            selection,
                            revision: editing.revision,
                            composing: editing.composing,
                        },
                    )))
                }
                Query::FileRead(_) | Query::FilePreview(_) => {
                    Err(Error::Unsupported("browser file driver not installed"))
                }
                Query::Focus(_) => Ok(QueryValue::Focus(dom::has_focus(&entry.control()?)?)),
                Query::Bounds(_) => {
                    let rect = entry.element()?.get_bounding_client_rect();
                    Ok(QueryValue::Bounds(dom::rect(&rect)))
                }
                Query::ScrollMetrics(_) => {
                    let element = entry.element()?;
                    Ok(QueryValue::ScrollMetrics(glory_render::ScrollMetrics {
                        x: dom::number(&element, "scrollLeft"),
                        y: dom::number(&element, "scrollTop"),
                        viewport_width: dom::number(&element, "clientWidth"),
                        viewport_height: dom::number(&element, "clientHeight"),
                        content_width: dom::number(&element, "scrollWidth"),
                        content_height: dom::number(&element, "scrollHeight"),
                    }))
                }
                Query::Scroll(_) => {
                    let element = entry.element()?;
                    Ok(QueryValue::Scroll {
                        x: dom::number(&element, "scrollLeft"),
                        y: dom::number(&element, "scrollTop"),
                    })
                }
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
        let rich_writes = match rich::observe(&self.dom, self.mapper.borrow().tree()) {
            Ok(writes) => writes,
            Err(error) => {
                self.dom.error.borrow_mut().get_or_insert(error);
                Vec::new()
            }
        };
        if let Err(error) = self.dom.read_files(&mut self.mapper.borrow_mut()) {
            self.dom.error.borrow_mut().get_or_insert(error);
        }
        if let Err(error) = self.dom.read_clipboard(&mut self.mapper.borrow_mut()) {
            self.dom.error.borrow_mut().get_or_insert(error);
        }
        if let Err(error) = self.dom.read_translations(self.mapper.borrow().tree()) {
            self.dom.error.borrow_mut().get_or_insert(error);
        }
        if let Err(error) = self.dom.read_collapses(self.mapper.borrow().tree()) {
            self.dom.error.borrow_mut().get_or_insert(error);
        }
        if let Err(error) = self.dom.read_images(self.mapper.borrow().tree()) {
            self.dom.error.borrow_mut().get_or_insert(error);
        }
        self.dom.read_layout();
        for write in rich_writes {
            if let Err(error) = write.apply() {
                self.dom.error.borrow_mut().get_or_insert(error);
            }
        }
        inputs.append(&mut self.dom.inputs.borrow_mut());
    }
}
fn perform(dom: &Dom, action: Action) -> Result<(), Error> {
    match action {
        Action::SelectRich {
            node,
            revision,
            selection,
        } => {
            let entry = dom.entry(node)?;
            if entry.editing.borrow().revision != revision || entry.editing.borrow().composing {
                return Err(Error::Unsupported(
                    "rich selection revision/composition changed",
                ));
            }
            let control = entry.control()?;
            if !dom.document_layers.allows(Some(control.as_ref()))
                || !dom.document_layers.allows_scope(control.as_ref())
            {
                return Err(Error::Unsupported("rich selection outside modal scope"));
            }
            rich::select(&control, selection)?;
        }
        Action::PickFiles { .. } | Action::ClearFiles { .. } | Action::RemoveFile(_) => {
            return Err(Error::Unsupported("browser file driver not installed"));
        }
        Action::CopyText { .. } => return Err(Error::Unsupported("text clipboard write")),
        Action::SelectText {
            node,
            revision,
            selection,
        } => {
            let entry = dom.entry(node)?;
            if entry.editing.borrow().revision != revision || entry.editing.borrow().composing {
                return Err(Error::Unsupported(
                    "text selection revision or composition changed",
                ));
            }
            let target = entry.control()?;
            if !dom.document_layers.allows(Some(target.as_ref()))
                || !dom.document_layers.allows_scope(target.as_ref())
            {
                return Err(Error::Unsupported(
                    "text selection outside document modal scope",
                ));
            }
            let raw = js_sys::Reflect::get(&target, &"value".into())
                .map_err(backend)?
                .as_string()
                .ok_or(Error::Unsupported("selection on this node"))?;
            if !raw.is_ascii() {
                return Err(Error::Unsupported(
                    "exact selection currently requires ASCII input text",
                ));
            }
            selection.validate(raw.len())?;
            let method = js_sys::Reflect::get(&target, &"setSelectionRange".into())
                .map_err(backend)?
                .dyn_into::<js_sys::Function>()
                .map_err(backend)?;
            method
                .call3(
                    &target,
                    &(selection.start as f64).into(),
                    &(selection.end as f64).into(),
                    &selection.direction.as_str().into(),
                )
                .map_err(backend)?;
        }
        Action::RevealOption { node } => select::reveal(dom, node)?,
        Action::RaiseLayer(node) => layers::raise(dom, node)?,
        Action::ScrollIntoView(node) => scroll_into_view::reveal(dom, node)?,
        Action::ExitMenu { node, backwards } => layers::exit_menu(dom, node, backwards)?,
        Action::Focus(node) => {
            let target = dom.entry(node)?.control()?;
            if !dom.document_layers.allows(Some(target.as_ref())) {
                return Err(Error::Unsupported("focus outside document modal scope"));
            }
            dom::focus(&target)?;
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
    let dom = Dom::new(document, root, frame.clone())?;
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
