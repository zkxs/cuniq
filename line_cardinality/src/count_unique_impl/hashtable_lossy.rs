// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use hashbrown::HashTable;

use crate::{CountUnique, CountUniqueHash};

/// Calculates the unique count and holds necessary state.
///
/// Internally, a [`HashTable`] is created that contains an entry for each distinct line in the
/// input. This may be expensive to drop if it contains a large amount of processed data, so using
/// [`std::mem::forget`] may be worth considering if your application will terminate immediately
/// after finishing the unique-counting work.
///
/// This implementation also has accepts a customizable `line_mapper` function with
/// [`LossyHashingLineCounter::with_line_mapper`]. If provided, this function will be applied to
/// each line before checking if it is unique or not. Note that this also affects the output that
/// will be seen from functions that enumerate internal state, such as
/// [`EmitLines::for_each_line`](crate::EmitLines::for_each_line).
pub struct LossyHashingLineCounter {
    map: HashTable<u64>,
    count: usize,
}

impl Default for LossyHashingLineCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// Constructors that do not take a custom line mapper
impl LossyHashingLineCounter {
    /// Creates a new [`LossyHashingLineCounter`].
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates a new [`LossyHashingLineCounter`] with a cardinality hint of `capacity`.
    ///
    /// Note that it is best to leave `capacity` unset unless you have a near-perfect idea of your
    /// data's cardinality lower bound, as it is extremely difficult to gain performance by setting
    /// it, but extremely easy to lose performance.
    pub fn with_capacity(capacity: usize) -> Self {
        LossyHashingLineCounter {
            map: HashTable::with_capacity(capacity),
            count: 0,
        }
    }
}

impl CountUnique for LossyHashingLineCounter {
    fn count(&self) -> usize {
        self.count
    }

    fn reset(&mut self) {
        self.count = 0;
        self.map.clear();
    }
}

impl CountUniqueHash for LossyHashingLineCounter {
    fn count_hash(&mut self, hash: u64) {
        let entry = self
            .map
            .entry(hash, |found_hash| *found_hash == hash, |rehash| *rehash);
        entry.or_insert_with(|| {
            self.count += 1;
            hash
        });
    }
}
