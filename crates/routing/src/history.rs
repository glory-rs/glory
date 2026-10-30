use crate::{Error, Location, Result};
use std::{cell::RefCell, collections::VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EntryKey(pub String);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub location: Location,
    pub key: Option<EntryKey>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Traversal {
    Known(Entry),
    Boundary,
    Host,
}

/// A platform service, independent of rendering. Host traversal is confirmed
/// later with RouterHandle::observe; requesting it does not prove success.
pub trait History: 'static {
    fn current(&self) -> Result<Entry>;
    fn push(&self, location: &Location) -> Result<Entry>;
    fn replace(&self, location: &Location) -> Result<Entry>;
    fn preview(&self, delta: i32) -> Result<Traversal>;
    fn traverse(&self, delta: i32) -> Result<Option<Entry>>;
    fn observe(&self, _entry: &Entry) -> Result {
        Ok(())
    }
}

struct Memory {
    entries: VecDeque<Entry>,
    cursor: usize,
    next: u64,
}
pub struct MemoryHistory {
    state: RefCell<Memory>,
    capacity: usize,
}
impl MemoryHistory {
    pub fn new(initial: Location) -> Self {
        Self::with_capacity(initial, 256)
    }
    pub fn with_capacity(initial: Location, capacity: usize) -> Self {
        Self {
            state: RefCell::new(Memory {
                entries: VecDeque::from([Entry {
                    location: initial,
                    key: Some(EntryKey("0".into())),
                }]),
                cursor: 0,
                next: 1,
            }),
            capacity: capacity.max(1),
        }
    }
    pub fn len(&self) -> usize {
        self.state.borrow().entries.len()
    }
    pub fn is_empty(&self) -> bool {
        false
    }
    fn index(state: &Memory, delta: i32) -> Option<usize> {
        state
            .cursor
            .checked_add_signed(delta as isize)
            .filter(|index| *index < state.entries.len())
    }
}
impl History for MemoryHistory {
    fn current(&self) -> Result<Entry> {
        let state = self.state.borrow();
        Ok(state.entries[state.cursor].clone())
    }
    fn push(&self, location: &Location) -> Result<Entry> {
        let mut state = self.state.borrow_mut();
        let next = state
            .next
            .checked_add(1)
            .ok_or(Error::OverLimit("entry identity"))?;
        let entry = Entry {
            location: location.clone(),
            key: Some(EntryKey(state.next.to_string())),
        };
        let length = state.cursor + 1;
        state.entries.truncate(length);
        state.entries.push_back(entry.clone());
        state.next = next;
        if state.entries.len() > self.capacity {
            state.entries.pop_front();
        }
        state.cursor = state.entries.len() - 1;
        Ok(entry)
    }
    fn replace(&self, location: &Location) -> Result<Entry> {
        let mut state = self.state.borrow_mut();
        let cursor = state.cursor;
        state.entries[cursor].location = location.clone();
        Ok(state.entries[cursor].clone())
    }
    fn preview(&self, delta: i32) -> Result<Traversal> {
        let state = self.state.borrow();
        Ok(Self::index(&state, delta)
            .map(|index| Traversal::Known(state.entries[index].clone()))
            .unwrap_or(Traversal::Boundary))
    }
    fn traverse(&self, delta: i32) -> Result<Option<Entry>> {
        let mut state = self.state.borrow_mut();
        let Some(index) = Self::index(&state, delta) else {
            return Ok(None);
        };
        state.cursor = index;
        Ok(Some(state.entries[index].clone()))
    }
}

/// Bounded, renderer-independent restoration data, keyed by history entry.
pub struct ScrollMemory {
    entries: VecDeque<(EntryKey, String, (f64, f64))>,
    capacity: usize,
}
impl ScrollMemory {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            capacity,
        }
    }
    pub fn record(
        &mut self,
        key: EntryKey,
        container: impl Into<String>,
        x: f64,
        y: f64,
    ) -> Result {
        if !x.is_finite() || !y.is_finite() {
            return Err(Error::BadParams("non-finite scroll position".into()));
        }
        let container = container.into();
        self.entries
            .retain(|(old_key, old_container, _)| old_key != &key || old_container != &container);
        if self.capacity != 0 {
            while self.entries.len() >= self.capacity {
                self.entries.pop_front();
            }
            self.entries.push_back((key, container, (x, y)));
        }
        Ok(())
    }
    pub fn restore(&mut self, key: &EntryKey, container: &str) -> Option<(f64, f64)> {
        let index = self
            .entries
            .iter()
            .position(|(entry, name, _)| entry == key && name == container)?;
        let entry = self.entries.remove(index)?;
        let position = entry.2;
        self.entries.push_back(entry);
        Some(position)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
impl Default for ScrollMemory {
    fn default() -> Self {
        Self::new(64)
    }
}
