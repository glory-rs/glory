use std::sync::atomic::{AtomicU64, Ordering};

/// An application instance, independent of the selected backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeId(u64);

impl Default for RuntimeId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod map_tests {
    use super::*;
    use crate::NodeMap;
    use std::{cell::Cell, collections::HashMap, rc::Rc};

    fn id(runtime: RuntimeId, slot: u32, generation: u32) -> NodeId {
        NodeId {
            runtime,
            slot,
            generation,
        }
    }
    #[test]
    fn pages_keep_payload_addresses_and_sparse_external_ids_do_not_allocate_to_the_maximum() {
        let runtime = RuntimeId::new();
        let mut map = NodeMap::default();
        let first = id(runtime, 0, 0);
        map.insert(first, "first".to_owned());
        let address = std::ptr::from_ref(map.get(&first).unwrap());
        for slot in [31, 32, 8191, 131071, 131072, u32::MAX] {
            map.insert(id(runtime, slot, 0), "other".to_owned());
        }
        assert!(std::ptr::eq(address, map.get(&first).unwrap()));
        assert_eq!(map.len(), 7);
        assert_eq!(map.capacity(), 6 * 32);
        assert!(map.get(&id(RuntimeId::new(), 0, 0)).is_none());
        map.remove(&first);
        let next = id(runtime, 0, 1);
        map.insert(next, "next".to_owned());
        assert!(map.get(&first).is_none());
        assert!(map.remove(&first).is_none());
        assert_eq!(map.get(&next).unwrap(), "next");
    }
    #[test]
    fn canonical_tokens_preserve_all_bits_and_reject_foreign_retired_and_aliased_text() {
        let runtime = RuntimeId(u64::MAX);
        let full = id(runtime, u32::MAX, u32::MAX);
        let mut map = NodeMap::default();
        map.insert(full, ());
        let token = format!("{}.{}.{}", u64::MAX, u32::MAX, u32::MAX);
        assert_eq!(map.id_for_token(&token), Some(full));
        for bad in [
            format!("0{token}"),
            format!("+{token}"),
            format!(" {token}"),
            format!("{token}.0"),
            "18446744073709551616.0.0".into(),
            "1.4294967296.0".into(),
            "1.0.4294967296".into(),
            "1..0".into(),
            "1.0.-1".into(),
            "1.0.00".into(),
            "1.0.0".into(),
        ] {
            assert_eq!(map.id_for_token(&bad), None, "{bad}");
        }
        map.remove(&full);
        assert_eq!(map.id_for_token(&token), None);
        assert_eq!(map.capacity(), 32);
    }
    #[test]
    fn random_recycled_slots_match_a_full_identity_hash_map_and_release_values() {
        struct Value(Rc<Cell<usize>>);
        impl Drop for Value {
            fn drop(&mut self) {
                self.0.set(self.0.get() - 1);
            }
        }
        let count = Rc::new(Cell::new(0));
        let runtime = RuntimeId::new();
        let mut map = NodeMap::default();
        let mut oracle = HashMap::new();
        let mut generations = [0; 128];
        let mut random = 1u64;
        for _ in 0..10000 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let slot = (random >> 32) as usize % generations.len();
            let key = id(runtime, slot as u32, generations[slot]);
            if oracle.remove(&key).is_some() {
                drop(map.remove(&key).unwrap());
                generations[slot] += 1;
                assert!(map.get(&key).is_none());
            } else {
                count.set(count.get() + 1);
                map.insert(key, Value(count.clone()));
                oracle.insert(key, ());
            }
            assert_eq!(map.len(), oracle.len());
            assert_eq!(count.get(), oracle.len());
            assert!(oracle.keys().all(|key| map.get(key).is_some()));
        }
        drop(map);
        assert_eq!(count.get(), 0);
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
