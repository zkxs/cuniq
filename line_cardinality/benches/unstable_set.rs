// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use std::collections::HashSet;

use cfg_if::cfg_if;

use line_cardinality::CountUnique;

cfg_if! {
    if #[cfg(feature = "ahash")] {
        use ahash::RandomState;
    } else {
        use std::hash::RandomState;
    }
}

pub struct Processor {
    set: HashSet<Vec<u8>, RandomState>,
    count: usize,
}

impl Default for Processor {
    fn default() -> Self {
        Self {
            set: HashSet::with_hasher(super::init_hasher_state()),
            count: 0,
        }
    }
}

impl CountUnique for Processor {
    #[inline(always)]
    fn count_line(&mut self, line: &[u8]) {
        self.set.get_or_insert_with(line, |line| {
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
