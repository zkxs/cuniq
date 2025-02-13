// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use crate::init_hasher_state;
use ahash::RandomState;
use hashbrown::HashTable;
use std::ops::Deref;

pub struct BakedInHashLineCounter {
    random_state: RandomState,
    map: HashTable<Box<[u8]>>,
    count: usize,
}

impl BakedInHashLineCounter {
    pub fn new() -> Self {
        BakedInHashLineCounter {
            random_state: init_hasher_state(),
            map: HashTable::with_capacity(0),
            count: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn count_line(&mut self, line: &[u8]) {
        let hash = self.random_state.hash_one(line);
        let entry = self.map.entry(
            hash,
            |entry| line == entry.deref(),
            |entry| self.random_state.hash_one(entry.deref()),
        );
        entry.or_insert_with(|| {
            self.count += 1;
            line.to_vec().into_boxed_slice()
        });
    }
}
