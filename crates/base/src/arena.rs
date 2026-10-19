//! Generational arena with typed IDs. A removed ID stays invalid after its slot is reused.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::marker::PhantomData;

/// Typed ID of a value in an `Arena<T>`.
#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Id<T> {
    index: u32,
    generation: u32,
    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// Slot index. Use it for sorting and tie-breaks only.
    pub fn index(self) -> u32 {
        self.index
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}
impl<T> Eq for Id<T> {}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.index, self.generation).cmp(&(other.index, other.generation))
    }
}
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({}v{})", self.index, self.generation)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

/// Arena that stores values in slots. Iteration is in slot order.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    len: u32,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Arena<T> {
    /// Make an empty arena.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
        }
    }

    /// Number of live values.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// True if there are no live values.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Insert a value. A free slot is reused (last freed first).
    pub fn insert(&mut self, value: T) -> Id<T> {
        self.len += 1;
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.value = Some(value);
            return Id {
                index,
                generation: slot.generation,
                marker: PhantomData,
            };
        }
        let index = self.slots.len() as u32;
        self.slots.push(Slot {
            generation: 0,
            value: Some(value),
        });
        Id {
            index,
            generation: 0,
            marker: PhantomData,
        }
    }

    /// Remove a value. Return `None` if the ID is not valid.
    pub fn remove(&mut self, id: Id<T>) -> Option<T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index);
        self.len -= 1;
        Some(value)
    }

    /// Get a value, or `None` if the ID is not valid.
    pub fn get(&self, id: Id<T>) -> Option<&T> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation == id.generation {
            slot.value.as_ref()
        } else {
            None
        }
    }

    /// Get a mutable value, or `None` if the ID is not valid.
    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation == id.generation {
            slot.value.as_mut()
        } else {
            None
        }
    }

    /// True if the ID is valid.
    pub fn contains(&self, id: Id<T>) -> bool {
        self.get(id).is_some()
    }

    /// Iterate over `(id, &value)` in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            s.value.as_ref().map(|v| {
                (
                    Id {
                        index: i as u32,
                        generation: s.generation,
                        marker: PhantomData,
                    },
                    v,
                )
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_remove() {
        let mut a = Arena::new();
        let x = a.insert("x");
        let y = a.insert("y");
        assert_eq!(a.len(), 2);
        assert_eq!(a.get(x), Some(&"x"));
        assert_eq!(a.remove(x), Some("x"));
        assert_eq!(a.get(x), None);
        assert_eq!(a.get(y), Some(&"y"));
        assert_eq!(a.remove(x), None);
    }

    #[test]
    fn removed_id_is_invalid_after_slot_reuse() {
        let mut a = Arena::new();
        let old = a.insert(1);
        a.remove(old);
        let new = a.insert(2);
        assert_eq!(old.index(), new.index(), "slot is reused");
        assert_ne!(old, new);
        assert!(!a.contains(old));
        assert_eq!(a.get(old), None);
        assert_eq!(a.remove(old), None);
        assert_eq!(a.get(new), Some(&2));
    }

    #[test]
    fn iteration_is_in_slot_order_and_skips_free_slots() {
        let mut a = Arena::new();
        let ids: Vec<_> = (0..4).map(|v| a.insert(v)).collect();
        a.remove(ids[1]);
        let v: Vec<_> = a.iter().map(|(_, v)| *v).collect();
        assert_eq!(v, vec![0, 2, 3]);
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn get_mut_changes_value() {
        let mut a = Arena::new();
        let id = a.insert(1);
        *a.get_mut(id).unwrap() = 5;
        assert_eq!(a.get(id), Some(&5));
    }
}
