//! Request-bound image decode observations. No application reentry from JS.
use super::{
    backend,
    dom::{Dom, Entry},
};
use futures_util::future::{AbortHandle, Abortable};
use glory_render::{Error, Event, EventKind, ImageStatus, Input, NodeId, memory::Memory};
use std::{collections::BTreeMap, rc::Rc};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlImageElement;

pub(super) struct Observer {
    revision: u64,
    abort: AbortHandle,
}
impl Drop for Observer {
    fn drop(&mut self) {
        self.abort.abort();
    }
}
#[derive(Default)]
pub(super) struct Images {
    pending: BTreeMap<NodeId, u64>,
    ready: BTreeMap<NodeId, (u64, ImageStatus)>,
}
impl Images {
    pub fn retire(&mut self, id: NodeId) {
        self.pending.remove(&id);
        self.ready.remove(&id);
    }
    pub fn clear(&mut self) {
        self.pending.clear();
        self.ready.clear();
    }
}
impl Dom {
    pub(super) fn queue_image(&self, id: NodeId, entry: &Entry, enabled: bool) {
        // Withdraw old acceptance immediately. Starting the next decode belongs
        // to the host read phase, after all source writes for this tick coalesce.
        let previous = entry.image.borrow_mut().take();
        drop(previous);
        let mut images = self.images.borrow_mut();
        images.retire(id);
        if enabled {
            images.pending.insert(id, entry.image_revision());
            self.frame.input();
        }
    }
    pub(super) fn read_images(self: &Rc<Self>, tree: &Memory) -> Result<(), Error> {
        let pending = std::mem::take(&mut self.images.borrow_mut().pending);
        for (id, revision) in pending {
            let Ok(entry) = self.entry(id) else { continue };
            if !entry.interests.borrow().contains(&EventKind::ImageStatus)
                || entry.image_revision() != revision
            {
                continue;
            }
            if !tree.active(id) {
                self.images.borrow_mut().pending.insert(id, revision);
                continue;
            }
            if entry
                .image
                .borrow()
                .as_ref()
                .is_some_and(|observer| observer.revision == revision)
            {
                continue;
            }
            let element = entry
                .element()?
                .dyn_into::<HtmlImageElement>()
                .map_err(|value| backend(value.into()))?;
            // HTML decode() captures its current request after source-selection's
            // microtask and rejects when that request changes, including A/B/A.
            // It observes the actual image, not a second fetch/probe element.
            let promise = element.decode();
            let (abort, registration) = AbortHandle::new_pair();
            *entry.image.borrow_mut() = Some(Box::new(Observer { revision, abort }));
            let weak_dom = Rc::downgrade(self);
            let weak_entry = Rc::downgrade(&entry);
            wasm_bindgen_futures::spawn_local(async move {
                let Ok(result) = Abortable::new(JsFuture::from(promise), registration).await else {
                    return;
                };
                let (Some(dom), Some(entry)) = (weak_dom.upgrade(), weak_entry.upgrade()) else {
                    return;
                };
                if entry.image_revision() != revision
                    || !entry.interests.borrow().contains(&EventKind::ImageStatus)
                {
                    return;
                }
                let status = if result.is_ok() {
                    ImageStatus::Loaded
                } else {
                    ImageStatus::Failed
                };
                dom.images.borrow_mut().ready.insert(id, (revision, status));
                dom.frame.input();
            });
        }
        let ready = std::mem::take(&mut self.images.borrow_mut().ready);
        for (id, (revision, status)) in ready {
            let Ok(entry) = self.entry(id) else { continue };
            if entry.image_revision() != revision
                || !entry.interests.borrow().contains(&EventKind::ImageStatus)
            {
                continue;
            }
            if !tree.active(id) {
                self.images
                    .borrow_mut()
                    .ready
                    .insert(id, (revision, status));
                continue;
            }
            self.inputs.borrow_mut().push(Input::Event {
                node: id,
                event: Event::ImageStatus { revision, status },
            });
        }
        Ok(())
    }
}
