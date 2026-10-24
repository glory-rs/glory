//! One non-abortable platform write per document; source/Owner cancellation
//! withdraws acceptance, never pretends it can undo an OS clipboard write.
use super::dom::Dom;
use crate::clipboard_state::Consumer;
pub(super) use crate::clipboard_state::Writes;
use glory_render::{ClipboardOutcome, Error, Event, Input, NodeId, memory::Memory};
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
fn outcome(value: &JsValue) -> ClipboardOutcome {
    let name = js_sys::Reflect::get(value, &"name".into())
        .ok()
        .and_then(|v| v.as_string());
    if matches!(name.as_deref(), Some("NotAllowedError" | "SecurityError")) {
        ClipboardOutcome::Denied
    } else {
        ClipboardOutcome::Failed
    }
}
impl Dom {
    pub(super) fn write_clipboard(
        self: &Rc<Self>,
        node: NodeId,
        revision: u64,
        request: u64,
        text: String,
    ) {
        let ticket = self.clipboards.borrow_mut().begin(Consumer {
            node,
            request,
            revision,
        });
        let Some(ticket) = ticket else {
            self.frame.input();
            return;
        };
        let start = (|| -> Result<js_sys::Promise, ClipboardOutcome> {
            let window = self
                .document
                .default_view()
                .ok_or(ClipboardOutcome::Unavailable)?;
            let navigator = js_sys::Reflect::get(window.as_ref(), &"navigator".into())
                .map_err(|error| outcome(&error))?;
            let clipboard = js_sys::Reflect::get(&navigator, &"clipboard".into())
                .map_err(|error| outcome(&error))?;
            if clipboard.is_null() || clipboard.is_undefined() {
                return Err(ClipboardOutcome::Unavailable);
            }
            let method = js_sys::Reflect::get(&clipboard, &"writeText".into())
                .map_err(|error| outcome(&error))?
                .dyn_into::<js_sys::Function>()
                .map_err(|_| ClipboardOutcome::Unavailable)?;
            method
                .call1(&clipboard, &text.into())
                .map_err(|error| outcome(&error))?
                .dyn_into::<js_sys::Promise>()
                .map_err(|_| ClipboardOutcome::Failed)
        })();
        let promise = match start {
            Ok(promise) => promise,
            Err(outcome) => {
                self.clipboards.borrow_mut().finish(ticket, outcome);
                self.frame.input();
                return;
            }
        };
        // Only ticket + weak document survive. The submitted text belongs to
        // the platform now; there is no per-source Rust future/payload history.
        let weak = Rc::downgrade(self);
        wasm_bindgen_futures::spawn_local(async move {
            let result = JsFuture::from(promise).await;
            let Some(dom) = weak.upgrade() else { return };
            let result = match result {
                Ok(_) => ClipboardOutcome::Written,
                Err(error) => outcome(&error),
            };
            dom.clipboards.borrow_mut().finish(ticket, result);
            dom.frame.input();
        });
    }
    pub(super) fn update_clipboard(&self, tree: &Memory) {
        let consumer = self.clipboards.borrow().consumer();
        let Some(consumer) = consumer else { return };
        if !tree
            .clipboard_pending(consumer.node, consumer.request, consumer.revision)
            .unwrap_or(false)
        {
            self.clipboards.borrow_mut().retire(consumer.node);
            return;
        }
        if !tree.active(consumer.node) || !tree.enabled(consumer.node) || tree.inert(consumer.node)
        {
            let mut writes = self.clipboards.borrow_mut();
            writes.retire(consumer.node);
            writes.ready(
                consumer.node,
                consumer.request,
                consumer.revision,
                ClipboardOutcome::Failed,
            );
            drop(writes);
            self.frame.input();
        }
    }
    pub(super) fn read_clipboard(&self, mapper: &mut glory_html::Mapper) -> Result<(), Error> {
        let ready = self.clipboards.borrow_mut().take_ready();
        for (node, (request, revision, outcome)) in ready {
            if !mapper
                .tree()
                .clipboard_pending(node, request, revision)
                .unwrap_or(false)
            {
                continue;
            }
            if mapper.finish_clipboard(node, request, revision)? {
                self.inputs.borrow_mut().push(Input::Event {
                    node,
                    event: Event::Clipboard {
                        request,
                        revision,
                        outcome,
                    },
                });
            }
        }
        Ok(())
    }
}
