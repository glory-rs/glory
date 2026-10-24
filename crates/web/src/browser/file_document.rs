//! Real browser-Document budgets shared by independent semantic consumers.
use super::files::Picker;
use glory_render::{Error, MAX_FILE_READS};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use web_sys::Document;
const MAX_REFERENCES: usize = 128;
const MAX_METADATA: usize = 128 * 1024;
const MAX_SELECTED_BYTES: u64 = 1024 * 1024 * 1024;
thread_local! {static DOCUMENTS:RefCell<Vec<Weak<DocumentFiles>>>=const{RefCell::new(Vec::new())};}
pub(super) struct DocumentFiles {
    pub document: Document,
    pub picker: RefCell<Option<Picker>>,
    pub next_picker: Cell<u64>,
    reads: Cell<usize>,
    raw_drop: Cell<bool>,
    references: Cell<usize>,
    metadata: Cell<usize>,
    bytes: Cell<u64>,
}
impl DocumentFiles {
    pub fn acquire(document: &Document) -> Rc<Self> {
        DOCUMENTS.with(|entries| {
            let mut entries = entries.borrow_mut();
            entries.retain(|entry| entry.strong_count() > 0);
            if let Some(entry) = entries
                .iter()
                .filter_map(Weak::upgrade)
                .find(|entry| entry.document == *document)
            {
                return entry;
            }
            let entry = Rc::new(Self {
                document: document.clone(),
                picker: RefCell::new(None),
                next_picker: Cell::new(0),
                reads: Cell::new(0),
                raw_drop: Cell::new(false),
                references: Cell::new(0),
                metadata: Cell::new(0),
                bytes: Cell::new(0),
            });
            entries.push(Rc::downgrade(&entry));
            entry
        })
    }
    pub fn raw_drop(self: &Rc<Self>) -> Option<DropPermit> {
        if self.raw_drop.replace(true) {
            None
        } else {
            Some(DropPermit(self.clone()))
        }
    }
    pub fn read(self: &Rc<Self>) -> Result<ReadPermit, Error> {
        if self.reads.get() >= MAX_FILE_READS {
            return Err(Error::Unsupported("document file read capacity is full"));
        }
        self.reads.set(self.reads.get() + 1);
        Ok(ReadPermit(self.clone()))
    }
    pub fn select(
        self: &Rc<Self>,
        previous: Option<&mut SelectionLease>,
        cost: Cost,
    ) -> Result<SelectionLease, Error> {
        let old = previous
            .as_ref()
            .map_or(Cost::default(), |lease| lease.cost);
        let references = self.references.get() - old.references + cost.references;
        let metadata = self.metadata.get() - old.metadata + cost.metadata;
        let bytes = self
            .bytes
            .get()
            .checked_sub(old.bytes)
            .and_then(|v| v.checked_add(cost.bytes))
            .ok_or(Error::Unsupported(
                "document selected file byte capacity overflow",
            ))?;
        if references > MAX_REFERENCES || metadata > MAX_METADATA || bytes > MAX_SELECTED_BYTES {
            return Err(Error::Unsupported(
                "document selected file capacity is full",
            ));
        }
        self.references.set(references);
        self.metadata.set(metadata);
        self.bytes.set(bytes);
        if let Some(previous) = previous {
            previous.cost = Cost::default();
        }
        Ok(SelectionLease {
            document: self.clone(),
            cost,
        })
    }
}
pub(super) struct ReadPermit(Rc<DocumentFiles>);
impl Drop for ReadPermit {
    fn drop(&mut self) {
        self.0.reads.set(self.0.reads.get() - 1);
    }
}
#[derive(Clone, Copy, Default)]
pub(super) struct Cost {
    pub references: usize,
    pub metadata: usize,
    pub bytes: u64,
}
pub(super) struct SelectionLease {
    document: Rc<DocumentFiles>,
    cost: Cost,
}
impl Drop for SelectionLease {
    fn drop(&mut self) {
        self.document
            .references
            .set(self.document.references.get() - self.cost.references);
        self.document
            .metadata
            .set(self.document.metadata.get() - self.cost.metadata);
        self.document
            .bytes
            .set(self.document.bytes.get() - self.cost.bytes);
    }
}

pub(super) struct DropPermit(Rc<DocumentFiles>);
impl Drop for DropPermit {
    fn drop(&mut self) {
        self.0.raw_drop.set(false);
    }
}
