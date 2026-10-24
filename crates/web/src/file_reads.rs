//! Four current non-abortable reads. Cancellation revokes delivery, while
//! physical flights and completed-but-undrained results retain their slots.
#[cfg(test)]
use glory_render::NodeId;
use glory_render::{Error, FileToken, MAX_FILE_READS, QueryToken};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Consumer {
    pub file: FileToken,
    pub query: QueryToken,
    pub applied: u64,
}
struct Flight<T> {
    ticket: u64,
    consumer: Option<Consumer>,
    result: Option<T>,
}
pub(crate) struct Reads<T> {
    next: u64,
    flights: [Option<Flight<T>>; MAX_FILE_READS],
}
impl<T> Default for Reads<T> {
    fn default() -> Self {
        Self {
            next: 0,
            flights: std::array::from_fn(|_| None),
        }
    }
}
impl<T> Reads<T> {
    pub fn begin(&mut self, consumer: Consumer) -> Result<u64, Error> {
        let slot = self
            .flights
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(Error::Unsupported("file read capacity is full"))?;
        let ticket = self
            .next
            .checked_add(1)
            .ok_or(Error::Unsupported("file read ticket exhausted"))?;
        self.next = ticket;
        *slot = Some(Flight {
            ticket,
            consumer: Some(consumer),
            result: None,
        });
        Ok(ticket)
    }
    /// Return unaccepted payload ownership to the caller, to drop outside any
    /// enclosing host-state borrow. Duplicate completions cannot replace Ready.
    pub fn finish(&mut self, ticket: u64, result: T) -> Option<T> {
        let Some(slot) = self
            .flights
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|flight| flight.ticket == ticket))
        else {
            return Some(result);
        };
        let flight = slot.as_mut().expect("current flight");
        if flight.result.is_some() {
            return Some(result);
        }
        if flight.consumer.is_none() {
            *slot = None;
            return Some(result);
        }
        flight.result = Some(result);
        None
    }
    pub fn revoke_invalid(
        &mut self,
        mut valid: impl FnMut(FileToken) -> bool,
    ) -> Vec<(Consumer, Option<T>)> {
        let mut removed = Vec::new();
        for slot in &mut self.flights {
            let Some(flight) = slot.as_mut() else {
                continue;
            };
            if !flight
                .consumer
                .is_some_and(|consumer| !valid(consumer.file))
            {
                continue;
            }
            let consumer = flight.consumer.take().expect("current consumer");
            let result = flight.result.take();
            if result.is_some() {
                *slot = None;
            }
            removed.push((consumer, result));
        }
        removed
    }
    #[cfg(test)]
    pub fn retire_invalid(&mut self, mut valid: impl FnMut(FileToken) -> bool) -> Vec<T> {
        self.retire(|consumer| !valid(consumer.file))
    }
    pub fn retire_all(&mut self) -> Vec<T> {
        self.retire(|_| true)
    }
    #[cfg(test)]
    pub fn retire_node(&mut self, node: NodeId) -> Vec<T> {
        self.retire(|consumer| consumer.file.node == node)
    }
    #[cfg(test)]
    pub fn retire_query(&mut self, query: QueryToken) -> Vec<T> {
        self.retire(|consumer| consumer.query == query)
    }
    fn retire(&mut self, mut matches: impl FnMut(Consumer) -> bool) -> Vec<T> {
        let mut removed = Vec::new();
        for slot in &mut self.flights {
            let Some(flight) = slot.as_mut() else {
                continue;
            };
            if !flight.consumer.is_some_and(&mut matches) {
                continue;
            }
            flight.consumer = None;
            if let Some(result) = flight.result.take() {
                *slot = None;
                removed.push(result);
            }
        }
        removed
    }
    pub fn take_ready(&mut self) -> Vec<(Consumer, T)> {
        let mut ready = Vec::new();
        for slot in &mut self.flights {
            if !slot.as_ref().is_some_and(|flight| flight.result.is_some()) {
                continue;
            }
            let flight = slot.take().expect("ready flight");
            if let (Some(consumer), Some(result)) = (flight.consumer, flight.result) {
                ready.push((consumer, result));
            }
        }
        ready
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use glory_render::{FileDocumentId, NodeIds, RuntimeId};
    fn consumer(query: u64) -> Consumer {
        Consumer {
            file: FileToken {
                document: FileDocumentId::new().unwrap(),
                node: NodeIds::new(RuntimeId::new()).allocate(),
                revision: 0,
                serial: 1,
            },
            query: QueryToken(query),
            applied: 10,
        }
    }
    #[test]
    fn cancellation_keeps_four_os_permits_until_real_completion_and_never_queues_rejections() {
        let mut state = Reads::<Vec<u8>>::default();
        let current: Vec<_> = (1..=4).map(consumer).collect();
        let tickets: Vec<_> = current.iter().map(|c| state.begin(*c).unwrap()).collect();
        for c in &current {
            assert!(state.retire_node(c.file.node).is_empty());
        }
        for index in 5..10005 {
            assert!(state.begin(consumer(index)).is_err());
        }
        assert_eq!(state.flights.iter().flatten().count(), 4);
        assert_eq!(state.finish(tickets[0], vec![7]), Some(vec![7]));
        let next = consumer(10005);
        let ticket = state.begin(next).unwrap();
        assert!(state.begin(consumer(10006)).is_err());
        assert_eq!(state.finish(ticket, vec![8]), None);
        assert!(
            state.begin(consumer(10007)).is_err(),
            "Ready result still owns its capacity"
        );
        assert_eq!(state.finish(ticket, vec![9]), Some(vec![9]));
        assert_eq!(state.take_ready(), vec![(next, vec![8])]);
        assert!(state.begin(consumer(10008)).is_ok());
    }
    #[test]
    fn retired_query_releases_ready_bytes_and_exhaustion_has_no_flight_or_pending_capture() {
        let mut state = Reads::<usize>::default();
        let first = consumer(1);
        let ticket = state.begin(first).unwrap();
        assert!(state.retire_query(QueryToken(2)).is_empty());
        assert_eq!(state.finish(ticket, 123), None);
        assert_eq!(state.retire_query(first.query), vec![123]);
        assert!(state.take_ready().is_empty());
        assert!(state.retire_invalid(|_| false).is_empty());
        assert!(state.retire_all().is_empty());
        assert_eq!(state.finish(ticket, 456), Some(456));
        state.next = u64::MAX;
        assert!(state.begin(consumer(2)).is_err());
        assert!(state.flights.iter().all(Option::is_none));
    }
    #[test]
    fn revoking_rights_completes_each_query_without_refunding_the_unfinished_os_flight() {
        let mut state = Reads::<usize>::default();
        let current = consumer(1);
        let ticket = state.begin(current).unwrap();
        let revoked = state.revoke_invalid(|_| false);
        assert_eq!(revoked.len(), 1);
        assert_eq!(revoked[0].0, current);
        assert!(revoked[0].1.is_none());
        assert_eq!(state.flights.iter().flatten().count(), 1);
        assert!(state.revoke_invalid(|_| false).is_empty());
        assert_eq!(state.finish(ticket, 123), Some(123));
        assert!(state.take_ready().is_empty());
        assert!(state.flights.iter().all(Option::is_none));
    }
}
