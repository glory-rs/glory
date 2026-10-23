use std::sync::atomic::{AtomicU64, Ordering};

/// An application instance, independent of the selected backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeId(u64);

impl Default for RuntimeId {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeId {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(
            NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
                .expect("runtime identity space exhausted"),
        )
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

/// A logical presentation identity, never a DOM node or hydration address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId {
    runtime: RuntimeId,
    slot: u32,
    generation: u32,
}

impl NodeId {
    pub fn runtime(self) -> RuntimeId {
        self.runtime
    }
    pub fn slot(self) -> u32 {
        self.slot
    }
    pub fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Debug)]
struct Slot {
    generation: u32,
    live: bool,
}

/// Reusable identities with bounded storage. Exhausted generations are retired,
/// never wrapped, so an old event cannot address a new object in the same slot.
#[derive(Debug)]
pub struct NodeIds {
    runtime: RuntimeId,
    slots: Vec<Slot>,
    free: Vec<u32>,
    live: usize,
}

impl NodeIds {
    pub fn new(runtime: RuntimeId) -> Self {
        Self {
            runtime,
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }

    pub fn allocate(&mut self) -> NodeId {
        let slot = if let Some(slot) = self.free.pop() {
            self.slots[slot as usize].live = true;
            slot
        } else {
            let slot = u32::try_from(self.slots.len()).expect("node identity space exhausted");
            self.slots.push(Slot {
                generation: 0,
                live: true,
            });
            slot
        };
        self.live += 1;
        NodeId {
            runtime: self.runtime,
            slot,
            generation: self.slots[slot as usize].generation,
        }
    }

    pub fn contains(&self, id: NodeId) -> bool {
        id.runtime == self.runtime
            && self
                .slots
                .get(id.slot as usize)
                .is_some_and(|slot| slot.live && slot.generation == id.generation)
    }

    pub fn release(&mut self, id: NodeId) -> bool {
        if !self.contains(id) {
            return false;
        }
        let slot = &mut self.slots[id.slot as usize];
        slot.live = false;
        if let Some(next) = slot.generation.checked_add(1) {
            slot.generation = next;
            self.free.push(id.slot);
        }
        self.live -= 1;
        true
    }

    pub fn len(&self) -> usize {
        self.live
    }
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }
}
