//! Image request-to-semantic revision mapping, captured at installation time.
use super::*;
use apace_dom::{ImageLoadStatus, ImageRequestId};
use glory_render::ImageStatus;

#[derive(Clone, Copy)]
pub(super) struct ImageObserver {
    revision: u64,
    request: Option<ImageRequestId>,
    captured: bool,
    status: Option<ImageStatus>,
    delivered: bool,
    pub(super) ready: Option<u128>,
}
impl ImageObserver {
    pub(super) fn new(revision: u64) -> Self {
        Self {
            revision,
            request: None,
            captured: false,
            status: None,
            delivered: false,
            ready: None,
        }
    }
    fn install(&mut self, revision: u64, request: Option<ImageRequestId>) -> bool {
        if self.captured && self.revision == revision && self.request == request {
            return false;
        }
        let delivered = self.revision == revision && self.delivered;
        self.revision = revision;
        self.request = request;
        self.captured = true;
        // No engine consumer means admission was denied (including exhausted
        // request identities) or the selected source is empty: terminal Failed.
        self.status = request.is_none().then_some(ImageStatus::Failed);
        self.delivered = delivered;
        true
    }
}
impl Dom {
    pub(super) fn queue_image_result(&mut self, id: NodeId) {
        let Some(image) = self
            .ranges
            .get_mut(&id)
            .and_then(|range| range.image.as_mut())
        else {
            return;
        };
        if image.ready.is_some() || image.delivered || image.status.is_none() {
            return;
        }
        self.image_sequence = self
            .image_sequence
            .checked_add(1)
            .expect("image observation sequence exhausted");
        image.ready = Some(self.image_sequence);
        self.image_ready.insert(self.image_sequence, id);
    }
    pub(super) fn sync_image(&mut self, id: NodeId) -> Result<(), Error> {
        let range = self.range(id)?;
        if range.image.is_none() {
            return Ok(());
        }
        let node = range.part(Part::Main)?;
        let revision = range.editing.requested;
        let has_source = self
            .doc
            .get_node(node)
            .and_then(|node| node.attr(apace_dom::local_name!("src")))
            .is_some_and(|source| !source.is_empty());
        let request = has_source.then(|| self.doc.image_request(node)).flatten();
        let image = self.ranges.get_mut(&id).unwrap().image.as_mut().unwrap();
        if image.install(revision, request) {
            if let Some(ready) = image.ready.take() {
                self.image_ready.remove(&ready);
            }
            self.queue_image_result(id);
        }
        Ok(())
    }
    fn harvest_images(&mut self) {
        for event in self.doc.take_image_events() {
            let Some(id) = self.owners.get(&event.node).copied() else {
                continue;
            };
            let Some(image) = self
                .ranges
                .get_mut(&id)
                .and_then(|range| range.image.as_mut())
            else {
                continue;
            };
            if !image.captured || image.request != Some(event.request) || image.delivered {
                continue;
            }
            image.status = Some(match event.status {
                ImageLoadStatus::Loaded => ImageStatus::Loaded,
                ImageLoadStatus::Failed => ImageStatus::Failed,
            });
            self.queue_image_result(id);
        }
    }
}
impl State {
    pub(super) fn collect_image_inputs(&mut self) {
        self.dom.harvest_images();
        let ready = std::mem::take(&mut self.dom.image_ready);
        for (sequence, id) in ready {
            let Some(image) = self
                .dom
                .ranges
                .get_mut(&id)
                .and_then(|range| range.image.as_mut())
            else {
                continue;
            };
            if image.ready != Some(sequence) {
                continue;
            }
            if !self.mapper.tree().active(id) {
                self.dom.image_ready.insert(sequence, id);
                continue;
            }
            image.ready = None;
            if image.delivered || !self.subscriptions.contains(&(id, EventKind::ImageStatus)) {
                continue;
            }
            let Some(status) = image.status else { continue };
            image.delivered = true;
            self.inputs.push(Input::Event {
                node: id,
                event: Event::ImageStatus {
                    revision: image.revision,
                    status,
                },
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_engine_consumer_maps_to_the_captured_revision_as_failed_and_can_recover() {
        let mut image = ImageObserver::new(7);
        assert!(image.install(7, None));
        assert_eq!(
            (image.revision, image.status),
            (7, Some(ImageStatus::Failed))
        );
        image.delivered = true;
        assert!(!image.install(7, None));
        assert!(image.install(8, Some(ImageRequestId(40))));
        assert_eq!(
            (image.revision, image.status, image.delivered),
            (8, None, false)
        );
        assert!(image.install(9, None));
        assert_eq!(
            (image.revision, image.status, image.delivered),
            (9, Some(ImageStatus::Failed), false)
        );
    }
}
