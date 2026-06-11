//! Applies Glory command batches to a blitz-dom document.
//!
//! Mapping notes (vs the reference semantics in
//! `glory_core::renderer::command_dom`):
//! - Glory node ids are u64 allocated by the producer; blitz node ids are
//!   `usize` allocated by the document — [`BlitzConsumer`] keeps the
//!   translation map.
//! - `SetText` maps to `DocumentMutator::set_node_text` (clears children,
//!   browser `textContent` semantics — matching the reference interpreter).
//! - `AddClass`/`RemoveClass` are reconciled into the `class` attribute
//!   (blitz has no classList API).
//! - `SetHtml` would need an HTML parser pass (blitz-html); rejected for
//!   now — widgets on the native backend should compose nodes instead.
//! - Events (`AttachEvent`/`DetachEvent`) are recorded. With the `shell`
//!   feature, `GloryBlitzDocument` forwards matching Blitz DOM events back
//!   into the held Glory `CommandHolder`.

use std::collections::{BTreeSet, HashMap};
#[cfg(feature = "shell")]
use std::{
    any::Any,
    cell::RefCell,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use blitz_dom::{Attribute, BaseDocument, DocumentConfig, QualName};
use glory_core::renderer::{Command, CommandInsertPosition};
#[cfg(feature = "shell")]
use glory_core::renderer::{EventData, PointerData, TargetData};
#[cfg(feature = "shell")]
use glory_core::web::holders::CommandHolder;
#[cfg(feature = "shell")]
use glory_core::{Holder, Widget};

pub struct BlitzConsumer {
    doc: BaseDocument,
    /// Glory command-stream id → blitz node id.
    ids: HashMap<u64, usize>,
    /// Blitz node id → Glory command-stream id.
    blitz_to_glory: HashMap<usize, u64>,
    /// Class sets per glory id (blitz exposes only whole attributes).
    classes: HashMap<u64, BTreeSet<String>>,
    /// (glory id, event name) listeners — recorded for the event stage.
    listeners: BTreeSet<(u64, String)>,
}

fn qual(name: &str) -> QualName {
    QualName::new(
        None,
        blitz_dom::Namespace::from("http://www.w3.org/1999/xhtml"),
        blitz_dom::LocalName::from(name),
    )
}

impl Default for BlitzConsumer {
    fn default() -> Self {
        Self::new()
    }
}

impl BlitzConsumer {
    pub fn new() -> Self {
        let mut doc = BaseDocument::new(DocumentConfig::default());
        // A fresh BaseDocument is just the document node (slab id 0);
        // give it the html/body skeleton and host the app in <body>.
        let body = {
            let mut mutator = doc.mutate();
            let html = mutator.create_element(qual("html"), Vec::new());
            let body = mutator.create_element(qual("body"), Vec::new());
            mutator.append_children(0, &[html]);
            mutator.append_children(html, &[body]);
            body
        };
        let mut ids = HashMap::new();
        // Reserved host root (glory id 0) → <body>.
        ids.insert(0, body);
        let mut blitz_to_glory = HashMap::new();
        blitz_to_glory.insert(body, 0);
        Self {
            doc,
            ids,
            blitz_to_glory,
            classes: HashMap::new(),
            listeners: BTreeSet::new(),
        }
    }

    pub fn document(&self) -> &BaseDocument {
        &self.doc
    }

    pub fn listeners(&self) -> &BTreeSet<(u64, String)> {
        &self.listeners
    }

    pub fn glory_id_for_blitz(&self, blitz_id: usize) -> Option<u64> {
        self.blitz_to_glory.get(&blitz_id).copied()
    }

    fn blitz_id(&self, glory_id: u64) -> Option<usize> {
        self.ids.get(&glory_id).copied()
    }

    pub fn apply_batch(&mut self, batch: &[Command]) {
        for command in batch {
            self.apply(command);
        }
    }

    pub fn apply(&mut self, command: &Command) {
        match command {
            Command::Create { id, name, .. } => {
                let node_id = self.doc.mutate().create_element(qual(name), Vec::new());
                self.ids.insert(*id, node_id);
                self.blitz_to_glory.insert(node_id, *id);
            }
            Command::SetAttribute { id, name, value } => {
                if let Some(node) = self.blitz_id(*id) {
                    self.doc.mutate().set_attribute(node, qual(name), value);
                }
            }
            Command::RemoveAttribute { id, name } => {
                if let Some(node) = self.blitz_id(*id) {
                    self.doc.mutate().clear_attribute(node, qual(name));
                }
            }
            // blitz has no property bag distinct from attributes; treat
            // properties as attributes for the spike.
            Command::SetProperty { id, name, value } => {
                if let Some(node) = self.blitz_id(*id) {
                    self.doc.mutate().set_attribute(node, qual(name), value);
                }
            }
            Command::RemoveProperty { id, name } => {
                if let Some(node) = self.blitz_id(*id) {
                    self.doc.mutate().clear_attribute(node, qual(name));
                }
            }
            Command::AddClass { id, value } => {
                let classes = self.classes.entry(*id).or_default();
                for part in value.split_whitespace() {
                    classes.insert(part.to_owned());
                }
                self.sync_classes(*id);
            }
            Command::RemoveClass { id, value } => {
                let classes = self.classes.entry(*id).or_default();
                for part in value.split_whitespace() {
                    classes.remove(part);
                }
                self.sync_classes(*id);
            }
            Command::SetText { id, value } => {
                if let Some(node) = self.blitz_id(*id) {
                    // textContent semantics: replace children with one text
                    // node (blitz's set_node_text only edits text nodes).
                    let mut mutator = self.doc.mutate();
                    mutator.remove_and_drop_all_children(node);
                    let text = mutator.create_text_node(value);
                    mutator.append_children(node, &[text]);
                }
            }
            Command::SetHtml { id, .. } => {
                let _ = id;
                // SetHtml requires an HTML parser pass (blitz-html); the
                // native backend composes nodes instead.
            }
            Command::Insert { parent, child, position } => {
                let (Some(parent), Some(child)) = (self.blitz_id(*parent), self.blitz_id(*child)) else {
                    return;
                };
                let mut mutator = self.doc.mutate();
                match position {
                    CommandInsertPosition::Tail => mutator.append_children(parent, &[child]),
                    CommandInsertPosition::Head => match mutator.child_ids(parent).first().copied() {
                        Some(first) => mutator.insert_nodes_before(first, &[child]),
                        None => mutator.append_children(parent, &[child]),
                    },
                    CommandInsertPosition::Before(anchor) => {
                        drop(mutator);
                        match self.blitz_id(*anchor) {
                            Some(anchor) => self.doc.mutate().insert_nodes_before(anchor, &[child]),
                            None => self.doc.mutate().append_children(parent, &[child]),
                        }
                    }
                    CommandInsertPosition::After(anchor) => {
                        drop(mutator);
                        match self.blitz_id(*anchor) {
                            Some(anchor) => self.doc.mutate().insert_nodes_after(anchor, &[child]),
                            None => self.doc.mutate().append_children(parent, &[child]),
                        }
                    }
                }
            }
            Command::Remove { child, .. } => {
                if let Some(blitz_child) = self.blitz_id(*child) {
                    self.doc.mutate().remove_node(blitz_child);
                    self.ids.remove(child);
                    self.blitz_to_glory.remove(&blitz_child);
                }
            }
            Command::AttachEvent { id, name, .. } => {
                self.listeners.insert((*id, name.clone()));
            }
            Command::DetachEvent { id, name } => {
                self.listeners.remove(&(*id, name.clone()));
            }
            Command::Query { .. } => {
                // Layout queries need blitz's layout pass; future stage.
            }
        }
    }

    fn sync_classes(&mut self, glory_id: u64) {
        let value = self
            .classes
            .get(&glory_id)
            .map(|set| set.iter().cloned().collect::<Vec<_>>().join(" "))
            .unwrap_or_default();
        if let Some(node) = self.blitz_id(glory_id) {
            self.doc.mutate().set_attribute(node, qual("class"), &value);
        }
    }

    /// Tag names of `glory_id`'s children, in order (test/inspection aid).
    pub fn child_tags(&mut self, glory_id: u64) -> Vec<String> {
        let Some(node) = self.blitz_id(glory_id) else { return Vec::new() };
        let child_ids = self.doc.mutate().child_ids(node);
        let mutator = self.doc.mutate();
        child_ids
            .iter()
            .filter_map(|id| mutator.element_name(*id).map(|name| name.local.to_string()))
            .collect()
    }

    /// Attribute value lookup by glory id (test/inspection aid).
    pub fn attribute(&self, glory_id: u64, name: &str) -> Option<String> {
        let node = self.blitz_id(glory_id)?;
        let node = self.doc.get_node(node)?;
        let element = node.element_data()?;
        element
            .attrs
            .iter()
            .find(|attr| attr.name.local.as_ref() == name)
            .map(|attr| attr.value.clone())
    }

    #[allow(dead_code)]
    fn unused(_: Attribute) {}
}

#[cfg(feature = "shell")]
pub fn launch_blitz_window(widget: impl Widget) {
    let holder = CommandHolder::new().mount(widget);
    let mut consumer = BlitzConsumer::new();
    consumer.apply_batch(&holder.take_batch());

    let event_loop = blitz_shell::create_default_event_loop::<blitz_shell::BlitzShellEvent>();
    let renderer = anyrender_vello::VelloWindowRenderer::new();
    let window = blitz_shell::WindowConfig::new(Box::new(GloryBlitzDocument { holder, consumer }), renderer);

    let mut application = blitz_shell::BlitzApplication::new(event_loop.create_proxy());
    application.add_window(window);
    event_loop.run_app(&mut application).expect("run Glory Blitz event loop");
}

#[cfg(feature = "shell")]
struct GloryBlitzDocument {
    holder: CommandHolder,
    consumer: BlitzConsumer,
}

#[cfg(feature = "shell")]
impl Deref for GloryBlitzDocument {
    type Target = BaseDocument;

    fn deref(&self) -> &Self::Target {
        &self.consumer.doc
    }
}

#[cfg(feature = "shell")]
impl DerefMut for GloryBlitzDocument {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.consumer.doc
    }
}

#[cfg(feature = "shell")]
impl blitz_dom::Document for GloryBlitzDocument {
    fn handle_ui_event(&mut self, event: blitz_traits::events::UiEvent) {
        let events = Rc::new(RefCell::new(Vec::new()));
        let handler = GloryEventBridge {
            blitz_to_glory: self.consumer.blitz_to_glory.clone(),
            listeners: self.consumer.listeners.clone(),
            events: events.clone(),
        };
        {
            let mut driver = blitz_dom::EventDriver::new(self.consumer.doc.mutate(), handler);
            driver.handle_ui_event(event);
        }

        let mut changed = false;
        for event in events.borrow_mut().drain(..) {
            if self.holder.dispatch_event(event) {
                self.consumer.apply_batch(&self.holder.take_batch());
                changed = true;
            }
        }
        if changed {
            self.consumer.doc.shell_provider.request_redraw();
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(feature = "shell")]
struct GloryEventBridge {
    blitz_to_glory: HashMap<usize, u64>,
    listeners: BTreeSet<(u64, String)>,
    events: Rc<RefCell<Vec<EventData>>>,
}

#[cfg(feature = "shell")]
impl blitz_dom::EventHandler for GloryEventBridge {
    fn handle_event(
        &mut self,
        chain: &[usize],
        event: &mut blitz_traits::events::DomEvent,
        _mutr: &mut blitz_dom::DocumentMutator<'_>,
        _event_state: &mut blitz_traits::events::EventState,
    ) {
        let name = event.name();
        let Some(glory_id) = chain.iter().find_map(|blitz_id| {
            let glory_id = self.blitz_to_glory.get(blitz_id).copied()?;
            self.listeners.contains(&(glory_id, name.to_owned())).then_some(glory_id)
        }) else {
            return;
        };

        let mut data = EventData::new(name, glory_id);
        match &event.data {
            blitz_traits::events::DomEventData::MouseMove(mouse)
            | blitz_traits::events::DomEventData::MouseDown(mouse)
            | blitz_traits::events::DomEventData::MouseUp(mouse)
            | blitz_traits::events::DomEventData::Click(mouse) => {
                data.pointer = Some(PointerData {
                    client_x: mouse.x as f64,
                    client_y: mouse.y as f64,
                    button: mouse.button as i16,
                    buttons: mouse.buttons.bits() as u16,
                });
            }
            blitz_traits::events::DomEventData::Input(input) => {
                data.target = Some(TargetData {
                    value: Some(input.value.clone()),
                    checked: None,
                });
            }
            _ => {}
        }
        self.events.borrow_mut().push(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory_core::Holder;
    use glory_core::reflow::Cage;
    use glory_core::web::events;
    use glory_core::web::holders::CommandHolder;
    use glory_core::web::widgets::{button, div, span};
    use glory_core::{Scope, Widget};

    #[derive(Debug)]
    struct Counter {
        value: Cage<i64>,
    }

    impl Widget for Counter {
        fn build(&mut self, ctx: &mut Scope) {
            let value = self.value.clone();
            div()
                .attr("data-app", "counter")
                .fill(button().text("+").on(events::click, move |_| value.revise(|mut v| *v += 1)))
                .fill(span().text(self.value.clone()))
                .show_in(ctx);
        }
    }

    /// The actual spike: a real widget tree drives a real blitz document
    /// through nothing but the command stream, including a reactive patch.
    #[test]
    fn widget_tree_renders_into_blitz_document() {
        let value = Cage::new(0i64);
        let holder = CommandHolder::new().mount(Counter { value: value.clone() });

        let mut consumer = BlitzConsumer::new();
        consumer.apply_batch(&holder.take_batch());

        assert_eq!(consumer.child_tags(0), vec!["div"]);
        // div is glory id 1 (first created); its attribute survived.
        assert_eq!(consumer.attribute(1, "data-app").as_deref(), Some("counter"));
        assert_eq!(consumer.child_tags(1), vec!["button", "span"]);
        assert!(consumer.listeners().iter().any(|(_, name)| name == "click"));

        // Reactive update flows through as an incremental batch.
        holder.update(|| value.revise(|mut v| *v = 42));
        consumer.apply_batch(&holder.take_batch());

        // The span (id 3: div=1, button=2, span=3) now carries "42".
        let span_blitz_id = consumer.blitz_id(3).unwrap();
        let doc = consumer.document();
        let span_node = doc.get_node(span_blitz_id).unwrap();
        assert_eq!(span_node.text_content(), "42");
    }
}
