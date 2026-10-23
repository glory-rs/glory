//! Paged live-identity storage. Payloads never move when the directory grows.
//! Empty slots retain capacity, never values or a visited business key history.
use crate::NodeId;
use std::{collections::HashMap, ops::Index};

const PAGE: usize = 32;
// A sparse external identity must not allocate a directory up to u32::MAX.
const DIRECT_PAGES: usize = 4096;
type Page<T> = Box<[Option<(NodeId, T)>]>;

/// A single-runtime index with at most one live identity per numeric slot.
/// Callers validate creation/generation watermarks before insertion. Every read
/// and removal compares the complete identity, including runtime/generation.
#[derive(Debug)]
pub struct NodeMap<T> {
    pages: Vec<Option<Page<T>>>,
    sparse: HashMap<usize, Page<T>>,
    len: usize,
}
impl<T> Default for NodeMap<T> {
    fn default() -> Self {
        Self {
            pages: Vec::new(),
            sparse: HashMap::new(),
            len: 0,
        }
    }
}
impl<T> NodeMap<T> {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Allocated payload slots, not bytes or business-data cache entries.
    pub fn capacity(&self) -> usize {
        (self.pages.iter().filter(|page| page.is_some()).count() + self.sparse.len()) * PAGE
    }
    fn page(&self, page: usize) -> Option<&Page<T>> {
        if page < DIRECT_PAGES {
            self.pages.get(page)?.as_ref()
        } else {
            self.sparse.get(&page)
        }
    }
    fn page_mut(&mut self, page: usize) -> Option<&mut Page<T>> {
        if page < DIRECT_PAGES {
            self.pages.get_mut(page)?.as_mut()
        } else {
            self.sparse.get_mut(&page)
        }
    }
    pub fn get(&self, id: &NodeId) -> Option<&T> {
        let (key, value) =
            self.page(id.slot() as usize / PAGE)?[id.slot() as usize % PAGE].as_ref()?;
        (*key == *id).then_some(value)
    }
    pub fn get_mut(&mut self, id: &NodeId) -> Option<&mut T> {
        let (key, value) =
            self.page_mut(id.slot() as usize / PAGE)?[id.slot() as usize % PAGE].as_mut()?;
        (*key == *id).then_some(value)
    }
    pub fn contains_key(&self, id: &NodeId) -> bool {
        self.get(id).is_some()
    }
    /// Inserting a different live identity in an occupied slot is an internal
    /// caller error, not a way to replace a retired generation or foreign node.
    pub fn insert(&mut self, id: NodeId, value: T) -> Option<T> {
        let page = id.slot() as usize / PAGE;
        let empty = || {
            (0..PAGE)
                .map(|_| None)
                .collect::<Vec<_>>()
                .into_boxed_slice()
        };
        let slots = if page < DIRECT_PAGES {
            if self.pages.len() <= page {
                self.pages.resize_with(page + 1, || None);
            }
            self.pages[page].get_or_insert_with(empty)
        } else {
            self.sparse.entry(page).or_insert_with(empty)
        };
        let slot = &mut slots[id.slot() as usize % PAGE];
        assert!(
            slot.as_ref().is_none_or(|(key, _)| *key == id),
            "occupied identity slot"
        );
        let previous = slot.replace((id, value));
        self.len += usize::from(previous.is_none());
        previous.map(|(_, value)| value)
    }
    pub fn remove(&mut self, id: &NodeId) -> Option<T> {
        let slot = &mut self.page_mut(id.slot() as usize / PAGE)?[id.slot() as usize % PAGE];
        if slot.as_ref().is_none_or(|(key, _)| *key != *id) {
            return None;
        }
        let (_, value) = slot.take().expect("live slot");
        self.len -= 1;
        Some(value)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &T)> {
        self.pages
            .iter()
            .filter_map(Option::as_ref)
            .chain(self.sparse.values())
            .flat_map(|page| {
                page.iter()
                    .filter_map(Option::as_ref)
                    .map(|(id, value)| (id, value))
            })
    }
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.iter().map(|(_, value)| value)
    }
    /// Resolve only canonical decimal wire identities against a live slot.
    /// No NodeId is manufactured from untrusted DOM text.
    pub fn id_for_token(&self, token: &str) -> Option<NodeId> {
        fn number(value: &str) -> Option<u64> {
            if value.is_empty()
                || value.len() > 1 && value.starts_with('0')
                || !value.bytes().all(|b| b.is_ascii_digit())
            {
                return None;
            }
            value.parse().ok()
        }
        let mut parts = token.split('.');
        let runtime = number(parts.next()?)?;
        let slot = u32::try_from(number(parts.next()?)?).ok()?;
        let generation = u32::try_from(number(parts.next()?)?).ok()?;
        if parts.next().is_some() {
            return None;
        }
        let (id, _) = self.page(slot as usize / PAGE)?[slot as usize % PAGE].as_ref()?;
        (id.runtime().value() == runtime && id.slot() == slot && id.generation() == generation)
            .then_some(*id)
    }
}
impl<T> Index<&NodeId> for NodeMap<T> {
    type Output = T;
    fn index(&self, id: &NodeId) -> &T {
        self.get(id).expect("live identity")
    }
}
