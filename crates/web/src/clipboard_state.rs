//! Numeric ownership of the document's single non-abortable platform write.
use glory_render::{ClipboardOutcome, NodeId};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Consumer {
    pub node: NodeId,
    pub request: u64,
    pub revision: u64,
}
struct Flight {
    ticket: u64,
    consumer: Option<Consumer>,
}
#[derive(Default)]
pub(crate) struct Writes {
    next: u64,
    flight: Option<Flight>,
    ready: BTreeMap<NodeId, (u64, u64, ClipboardOutcome)>,
}
impl Writes {
    pub fn consumer(&self) -> Option<Consumer> {
        self.flight.as_ref().and_then(|flight| flight.consumer)
    }
    pub fn ready(&mut self, node: NodeId, request: u64, revision: u64, outcome: ClipboardOutcome) {
        self.ready.insert(node, (request, revision, outcome));
    }
    pub fn take_ready(&mut self) -> BTreeMap<NodeId, (u64, u64, ClipboardOutcome)> {
        std::mem::take(&mut self.ready)
    }
    pub fn retire(&mut self, node: NodeId) {
        self.ready.remove(&node);
        if let Some(flight) = &mut self.flight
            && flight.consumer.is_some_and(|c| c.node == node)
        {
            flight.consumer = None
        }
    }
    pub fn clear(&mut self) {
        self.ready.clear();
        if let Some(flight) = &mut self.flight {
            flight.consumer = None
        }
    }
    pub fn begin(&mut self, consumer: Consumer) -> Option<u64> {
        self.retire(consumer.node);
        if self.flight.is_some() {
            self.ready(
                consumer.node,
                consumer.request,
                consumer.revision,
                ClipboardOutcome::Failed,
            );
            return None;
        }
        let Some(ticket) = self.next.checked_add(1) else {
            self.ready(
                consumer.node,
                consumer.request,
                consumer.revision,
                ClipboardOutcome::Failed,
            );
            return None;
        };
        self.next = ticket;
        self.flight = Some(Flight {
            ticket,
            consumer: Some(consumer),
        });
        Some(ticket)
    }
    pub fn finish(&mut self, ticket: u64, outcome: ClipboardOutcome) {
        if !self
            .flight
            .as_ref()
            .is_some_and(|flight| flight.ticket == ticket)
        {
            return;
        }
        if let Some(consumer) = self.flight.take().and_then(|flight| flight.consumer) {
            self.ready(consumer.node, consumer.request, consumer.revision, outcome)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use glory_render::{NodeIds, RuntimeId};
    fn consumer() -> Consumer {
        Consumer {
            node: NodeIds::new(RuntimeId::new()).allocate(),
            request: 1,
            revision: 0,
        }
    }
    #[test]
    fn one_platform_write_cancels_acceptance_without_accumulating_nonabortable_promises() {
        let mut state = Writes::default();
        let first = consumer();
        let ticket = state.begin(first).unwrap();
        assert_eq!(state.consumer(), Some(first));
        for request in 2..10000 {
            assert_eq!(state.begin(Consumer { request, ..first }), None);
            assert!(state.consumer().is_none());
            assert_eq!(state.ready.len(), 1)
        }
        let latest = state.take_ready();
        assert_eq!(latest[&first.node], (9999, 0, ClipboardOutcome::Failed));
        state.finish(ticket, ClipboardOutcome::Written);
        assert!(state.take_ready().is_empty());
        let next = Consumer {
            request: 10000,
            ..first
        };
        let ticket = state.begin(next).unwrap();
        state.finish(ticket, ClipboardOutcome::Written);
        state.finish(ticket, ClipboardOutcome::Written);
        assert_eq!(state.take_ready().len(), 1);
    }
    #[test]
    fn other_consumers_do_not_revoke_the_inflight_owner_and_retirement_releases_results() {
        let mut state = Writes::default();
        let first = consumer();
        let other = consumer();
        let ticket = state.begin(first).unwrap();
        assert_eq!(state.begin(other), None);
        assert_eq!(state.consumer(), Some(first));
        state.retire(other.node);
        assert!(state.take_ready().is_empty());
        state.retire(first.node);
        state.finish(ticket, ClipboardOutcome::Written);
        assert!(state.take_ready().is_empty());
        let ticket = state.begin(first).unwrap();
        state.clear();
        state.finish(ticket, ClipboardOutcome::Written);
        assert!(state.take_ready().is_empty());
    }
    #[test]
    fn exhausted_ticket_produces_a_terminal_failure_without_platform_admission() {
        let mut state = Writes {
            next: u64::MAX,
            ..Default::default()
        };
        let c = consumer();
        assert_eq!(state.begin(c), None);
        assert!(state.consumer().is_none());
        assert_eq!(
            state.take_ready()[&c.node],
            (c.request, c.revision, ClipboardOutcome::Failed)
        );
    }
}
