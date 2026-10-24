//! One current selection and monotonic request/batch watermarks per live node.
//! No historical files, bytes, platform handles, callbacks or retired tokens.
use super::{FileDocumentId, FileEvent, FileInfo, FileOrigin, FileOutcome, FileProps, FileToken};
use crate::{Error, NodeId};

#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    request: u64,
    pending: Option<(u64, u64)>,
    batch: u64,
    current: Vec<FileInfo>,
}
impl State {
    pub fn next_request(&self) -> Result<u64, Error> {
        self.request
            .checked_add(1)
            .ok_or(Error::InvalidProps("file request identity exhausted"))
    }
    pub fn admit(&mut self, request: u64, revision: u64) -> Result<(), Error> {
        if request <= self.request {
            return Err(Error::InvalidProps(
                "file request must strictly advance its live node watermark",
            ));
        }
        self.request = request;
        self.pending = Some((request, revision));
        Ok(())
    }
    pub fn pending(&self, request: u64, revision: u64) -> bool {
        self.pending == Some((request, revision))
    }
    /// Source/configuration reset revokes read rights and acceptance, retaining
    /// only numeric watermarks to reject reuse within this NodeId lifetime.
    pub fn clear(&mut self) {
        self.pending = None;
        self.current.clear();
    }
    pub fn file(&self, token: FileToken) -> Result<&FileInfo, Error> {
        self.current
            .iter()
            .find(|file| file.token == token)
            .ok_or(Error::Unsupported("file token is no longer selected"))
    }
    pub fn remove(&mut self, token: FileToken) -> Result<(), Error> {
        let index = self
            .current
            .iter()
            .position(|file| file.token == token)
            .ok_or(Error::Unsupported("file token is no longer selected"))?;
        self.current.remove(index);
        Ok(())
    }
    pub fn validate_event(
        &self,
        node: NodeId,
        props: &FileProps,
        document: FileDocumentId,
        event: &FileEvent,
    ) -> Result<bool, Error> {
        event.validate_node(node, props)?;
        if event.document != document {
            return Err(Error::InvalidEvent(
                "file event belongs to another document",
            ));
        }
        if event.origin == FileOrigin::Adopted {
            if event.batch <= self.batch {
                return Ok(false);
            }
            if self.batch != 0 || self.request != 0 {
                return Err(Error::InvalidEvent(
                    "file adoption requires a fresh control lifetime",
                ));
            }
        }
        if let Some(request) = event.request {
            if request == 0 || request > self.request {
                return Err(Error::InvalidEvent(
                    "file completion names an unadmitted picker request",
                ));
            }
            if !self.pending(request, event.revision) {
                return Ok(false);
            }
        }
        Ok(event.revision == props.revision
            && (event.request.is_some() && !matches!(event.outcome, FileOutcome::Selected(_))
                || event.batch > self.batch))
    }
    pub fn observe(
        &mut self,
        node: NodeId,
        props: &FileProps,
        document: FileDocumentId,
        event: &FileEvent,
    ) -> Result<bool, Error> {
        if !self.validate_event(node, props, document, event)? {
            return Ok(false);
        }
        if event.request.is_none() || matches!(event.outcome, FileOutcome::Selected(_)) {
            self.batch = event.batch;
        }
        // Only a successful drop supersedes a pending chooser. Rejected drops
        // cannot withdraw its terminal observation or strand its pending state.
        if event.request.is_some() || matches!(event.outcome, FileOutcome::Selected(_)) {
            self.pending = None;
        }
        if let FileOutcome::Selected(files) = &event.outcome {
            self.current = files.clone();
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileConfig, FileOrigin, NodeIds, RuntimeId};
    fn event(
        node: NodeId,
        document: FileDocumentId,
        request: Option<u64>,
        batch: u64,
        revision: u64,
        serial: u64,
    ) -> FileEvent {
        FileEvent {
            document,
            revision,
            request,
            batch,
            origin: if request.is_some() {
                FileOrigin::Picker
            } else {
                FileOrigin::Drop
            },
            outcome: FileOutcome::Selected(vec![FileInfo {
                token: FileToken {
                    document,
                    node,
                    revision,
                    serial,
                },
                name: "file.txt".into(),
                media_type: "text/plain".into(),
                size: 3,
            }]),
        }
    }
    #[test]
    fn current_selection_picker_cancel_drop_clear_and_no_request_aba() {
        let node = NodeIds::new(RuntimeId::new()).allocate();
        let document = FileDocumentId::new().unwrap();
        let props = FileProps {
            configuration: FileConfig {
                drop: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut state = State::default();
        state.admit(1, 0).unwrap();
        let first = event(node, document, Some(1), 1, 0, 1);
        assert!(state.observe(node, &props, document, &first).unwrap());
        assert!(!state.observe(node, &props, document, &first).unwrap());
        let FileOutcome::Selected(files) = &first.outcome else {
            panic!()
        };
        assert_eq!(state.file(files[0].token).unwrap(), &files[0]);
        state.admit(2, 0).unwrap();
        let cancel = FileEvent {
            request: Some(2),
            batch: 2,
            outcome: FileOutcome::Cancelled,
            ..first.clone()
        };
        assert!(state.observe(node, &props, document, &cancel).unwrap());
        assert!(state.file(files[0].token).is_ok());
        state.admit(3, 0).unwrap();
        let drop = event(node, document, None, 3, 0, 2);
        assert!(state.observe(node, &props, document, &drop).unwrap());
        assert!(state.file(files[0].token).is_err());
        assert!(!state.pending(3, 0));
        let late = event(node, document, Some(3), 4, 0, 3);
        assert!(!state.observe(node, &props, document, &late).unwrap());
        state.clear();
        assert!(
            state
                .file(match &drop.outcome {
                    FileOutcome::Selected(f) => f[0].token,
                    _ => panic!(),
                })
                .is_err()
        );
        assert!(state.admit(3, 0).is_err());
        state.admit(u64::MAX, 0).unwrap();
        assert!(state.next_request().is_err());
    }
    #[test]
    fn foreign_document_future_request_and_revision_reject_without_changing_selection() {
        let node = NodeIds::new(RuntimeId::new()).allocate();
        let document = FileDocumentId::new().unwrap();
        let props = FileProps::default();
        let mut state = State::default();
        state.admit(1, 0).unwrap();
        let first = event(node, document, Some(1), 1, 0, 1);
        assert!(
            state
                .observe(node, &props, FileDocumentId::new().unwrap(), &first)
                .is_err()
        );
        let future = event(node, document, Some(2), 1, 0, 1);
        assert!(state.observe(node, &props, document, &future).is_err());
        assert!(
            state
                .observe(
                    node,
                    &props,
                    document,
                    &event(node, document, Some(1), 1, 1, 1)
                )
                .is_err()
        );
        assert!(state.pending(1, 0));
        assert!(state.observe(node, &props, document, &first).unwrap());
        let token = match &first.outcome {
            FileOutcome::Selected(f) => f[0].token,
            _ => panic!(),
        };
        state.remove(token).unwrap();
        assert!(state.file(token).is_err());
        assert!(state.remove(token).is_err());
    }
}
