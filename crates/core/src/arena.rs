#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Key {
    pub slot: u32,
    pub generation: u32,
}
impl Key {
    pub fn packed(self) -> u64 {
        (u64::from(self.generation) << 32) | u64::from(self.slot)
    }
    pub fn unpack(value: u64) -> Self {
        Self {
            slot: value as u32,
            generation: (value >> 32) as u32,
        }
    }
}

struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

pub(crate) struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    len: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
        }
    }
}

impl<T> Arena<T> {
    pub fn insert(&mut self, value: T) -> Key {
        let slot = if let Some(slot) = self.free.pop() {
            self.slots[slot as usize].value = Some(value);
            slot
        } else {
            let slot = u32::try_from(self.slots.len()).expect("runtime identity space exhausted");
            self.slots.push(Slot {
                generation: 0,
                value: Some(value),
            });
            slot
        };
        self.len += 1;
        Key {
            slot,
            generation: self.slots[slot as usize].generation,
        }
    }

    pub fn get(&self, key: Key) -> Option<&T> {
        let slot = self.slots.get(key.slot as usize)?;
        (slot.generation == key.generation)
            .then_some(slot.value.as_ref())
            .flatten()
    }

    pub fn get_mut(&mut self, key: Key) -> Option<&mut T> {
        let slot = self.slots.get_mut(key.slot as usize)?;
        if slot.generation != key.generation {
            return None;
        }
        slot.value.as_mut()
    }

    pub fn remove(&mut self, key: Key) -> Option<T> {
        let slot = self.slots.get_mut(key.slot as usize)?;
        if slot.generation != key.generation {
            return None;
        }
        let value = slot.value.take()?;
        if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            self.free.push(key.slot);
        }
        self.len -= 1;
        Some(value)
    }

    pub fn len(&self) -> usize {
        self.len
    }
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().filter_map(|slot| slot.value.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_generations_are_retired_instead_of_recycled() {
        let mut arena = Arena::default();
        let key = arena.insert(1);
        arena.slots[0].generation = u32::MAX;
        let last = Key {
            generation: u32::MAX,
            ..key
        };
        assert_eq!(arena.remove(last), Some(1));
        let next = arena.insert(2);
        assert_ne!(next.slot, last.slot);
        assert!(arena.get(last).is_none());
    }
}
