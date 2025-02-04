// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use hashbrown::HashTable;

use line_cardinality::CountUnique;

#[cfg(not(feature = "ahash"))]
use std::hash::BuildHasher;

use crate::{init_hasher_state, RandomState};

pub struct Processor {
    map: HashTable<Vec<u8>>,
    random_state: RandomState,
    count: usize,
}

impl Default for Processor {
    fn default() -> Self {
        Self {
            map: HashTable::new(),
            random_state: init_hasher_state(),
            count: 0,
        }
    }
}

impl CountUnique for Processor {
    #[inline(always)]
    fn count_line(&mut self, line: &[u8]) {
        let hash = self.random_state.hash_one(line);
        let entry = self.map.entry(
            hash,
            |entry| line == entry.as_slice(),
            |entry| {
                let slice = entry.as_slice();
                self.random_state.hash_one(slice)
            },
        );
        entry.or_insert_with(|| {
            self.count += 1;
            line.to_vec()
        });
    }

    fn count(&self) -> usize {
        self.count
    }

    fn reset(&mut self) {
        unimplemented!("not used in benches")
    }
}
