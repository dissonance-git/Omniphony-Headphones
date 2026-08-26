//! Bounded stable source-ID to physical-lane assignment.
//!
//! The allocator owns identity continuity only. It knows nothing about Windows,
//! ADM, PCM, or rendering. Surviving nonzero IDs keep their slots, ended IDs
//! release theirs, and new IDs take the lowest free slot. Capacity is fixed at
//! construction, so reconciliation performs no allocation.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableSourceSlotError {
    ReservedIdZero,
    DuplicateId(u64),
    CapacityExceeded { capacity: usize, active: usize },
}

#[derive(Debug, Clone)]
pub struct StableSourceSlots {
    slots: Vec<Option<u64>>,
}

impl StableSourceSlots {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
        }
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    pub fn slots(&self) -> &[Option<u64>] {
        &self.slots
    }

    pub fn slot_for(&self, id: u64) -> Option<usize> {
        self.slots.iter().position(|slot| *slot == Some(id))
    }

    /// Number of slots needed to include every currently active assignment.
    /// Trailing free capacity is intentionally omitted so sparse realtime
    /// consumers do not have to render negotiated-but-unused lanes.
    pub fn active_span_len(&self) -> usize {
        self.slots
            .iter()
            .rposition(Option::is_some)
            .map(|index| index + 1)
            .unwrap_or(0)
    }

    pub fn reconcile(&mut self, active_ids: &[u64]) -> Result<(), StableSourceSlotError> {
        if active_ids.len() > self.slots.len() {
            return Err(StableSourceSlotError::CapacityExceeded {
                capacity: self.slots.len(),
                active: active_ids.len(),
            });
        }

        for (index, &id) in active_ids.iter().enumerate() {
            if id == 0 {
                return Err(StableSourceSlotError::ReservedIdZero);
            }
            if active_ids[..index].contains(&id) {
                return Err(StableSourceSlotError::DuplicateId(id));
            }
        }

        // Release only identities that actually ended.
        for slot in &mut self.slots {
            if let Some(id) = *slot {
                if !active_ids.contains(&id) {
                    *slot = None;
                }
            }
        }

        // Existing identities keep their lanes. Newly admitted identities take
        // the lowest free slot, making allocation deterministic and testable.
        for &id in active_ids {
            if self.slot_for(id).is_some() {
                continue;
            }
            let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) else {
                return Err(StableSourceSlotError::CapacityExceeded {
                    capacity: self.slots.len(),
                    active: active_ids.len(),
                });
            };
            *slot = Some(id);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surviving_ids_keep_slots_across_reorder_spawn_and_despawn() {
        let mut slots = StableSourceSlots::new(3);
        slots.reconcile(&[10, 20]).unwrap();
        assert_eq!(slots.slots(), &[Some(10), Some(20), None]);

        slots.reconcile(&[20, 10]).unwrap();
        assert_eq!(slots.slots(), &[Some(10), Some(20), None]);

        slots.reconcile(&[20, 30]).unwrap();
        assert_eq!(slots.slots(), &[Some(30), Some(20), None]);
        assert_eq!(slots.slot_for(20), Some(1));
        assert_eq!(slots.active_span_len(), 2);
    }

    #[test]
    fn active_span_omits_unused_trailing_capacity_but_preserves_holes() {
        let mut slots = StableSourceSlots::new(4);
        slots.reconcile(&[1, 2, 3]).unwrap();
        slots.reconcile(&[1, 3]).unwrap();
        assert_eq!(slots.slots(), &[Some(1), None, Some(3), None]);
        assert_eq!(slots.active_span_len(), 3);
    }

    #[test]
    fn zero_duplicates_and_overflow_are_explicit_errors() {
        let mut slots = StableSourceSlots::new(1);
        assert_eq!(
            slots.reconcile(&[0]).unwrap_err(),
            StableSourceSlotError::ReservedIdZero
        );
        assert_eq!(
            slots.reconcile(&[7, 7]).unwrap_err(),
            StableSourceSlotError::CapacityExceeded {
                capacity: 1,
                active: 2,
            }
        );

        let mut two = StableSourceSlots::new(2);
        assert_eq!(
            two.reconcile(&[7, 7]).unwrap_err(),
            StableSourceSlotError::DuplicateId(7)
        );
        assert_eq!(
            two.reconcile(&[7, 8, 9]).unwrap_err(),
            StableSourceSlotError::CapacityExceeded {
                capacity: 2,
                active: 3,
            }
        );
    }
}
