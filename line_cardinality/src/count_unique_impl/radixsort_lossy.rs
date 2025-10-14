// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use crate::{CountUnique, CountUniqueHash};
use voracious_radix_sort::RadixSort;

pub struct LossySortingLineCounter {
    line_hashes: Vec<u64>,
}

impl Default for LossySortingLineCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl LossySortingLineCounter {
    /// Creates a new [`LossySortingLineCounter`].
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates a new [`LossySortingLineCounter`] with a cardinality hint of `capacity`.
    ///
    /// Note that it is best to leave `capacity` unset unless you have a near-perfect idea of your
    /// data's cardinality lower bound, as it is extremely difficult to gain performance by setting
    /// it, but extremely easy to lose performance.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            line_hashes: Vec::with_capacity(capacity),
        }
    }

    /// Counts unique in sorted line hashes.
    ///
    /// # Preconditions
    /// self.line_hashes MUST contain at least two elements
    #[inline(always)]
    fn count_sorted(&self) -> usize {
        let mut count = 1;
        for i in 0..self.line_hashes.len() - 1 {
            let a = &self.line_hashes[i];
            let b = &self.line_hashes[i + 1];
            count += (*a != *b) as usize;
        }
        count
    }
}

impl CountUnique for LossySortingLineCounter {
    fn count(&mut self) -> usize {
        if self.line_hashes.is_empty() {
            0
        } else if self.line_hashes.len() == 1 {
            1
        } else {
            // guaranteed 2+ entries
            self.line_hashes.voracious_sort();
            self.count_sorted()
        }
    }

    fn count_multithreaded(&mut self, threads: usize) -> usize {
        if threads > 1 {
            if self.line_hashes.is_empty() {
                0
            } else if self.line_hashes.len() == 1 {
                1
            } else {
                // guaranteed 2+ entries
                self.line_hashes.voracious_mt_sort(threads);
                self.count_sorted()
            }
        } else {
            self.count()
        }
    }

    fn reset(&mut self) {
        self.line_hashes.clear();
    }
}

impl CountUniqueHash for LossySortingLineCounter {
    fn count_hash(&mut self, hash: u64) {
        self.line_hashes.push(hash);
    }
}
