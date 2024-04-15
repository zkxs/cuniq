// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

#[cfg(not(feature = "ahash"))]
use std::hash::BuildHasher;

use hashbrown::HashTable;

use crate::count_unique_impl::init_hasher_state;
use crate::CountUnique;

use super::RandomState;

pub struct InexactHashingLineCounter<M>
where
{
    map: HashTable<u64>,
    random_state: RandomState,
    string_buffer: Vec<u8>,
    count: usize,
    line_mapper: M,
}

impl Default for InexactHashingLineCounter<()> {
    fn default() -> Self {
        Self::new()
    }
}

/// Constructors that do not take a custom line mapper
impl InexactHashingLineCounter<()> {
    /// Creates a new count_unique_impl.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates a new count_unique_impl with a cardinality hint of `capacity`.
    ///
    /// Note that it is best to leave `capacity` unset unless you have a near-perfect idea of your
    /// data's cardinality lower bound, as it is extremely difficult to gain performance by setting
    /// it, but extremely easy to lose performance.
    pub fn with_capacity(capacity: usize) -> Self {
        InexactHashingLineCounter {
            map: HashTable::with_capacity(capacity),
            random_state: init_hasher_state(),
            string_buffer: Vec::new(),
            count: 0,
            line_mapper: (),
        }
    }
}

/// Constructors that take a custom line mapper
impl<M> InexactHashingLineCounter<M>
where
    M: for<'a> FnMut(&'a [u8], &'a mut Vec<u8>) -> &'a [u8],
{
    /// Creates a new count_unique_impl with a custom `line_mapper` function which will be applied to
    /// each read line before counting.
    pub fn with_line_mapper(line_mapper: M) -> Self {
        Self::with_line_mapper_and_capacity(line_mapper, 0)
    }

    /// Creates a new count_unique_impl with a cardinality hint of `capacity` and a custom
    /// `line_mapper` function which will be applied to each read line before counting.
    ///
    /// Note that it is best to leave `capacity` unset unless you have a near-perfect idea of your
    /// data's cardinality lower bound, as it is extremely difficult to gain performance by setting
    /// it, but extremely easy to lose performance.
    pub fn with_line_mapper_and_capacity(line_mapper: M, capacity: usize) -> Self {
        InexactHashingLineCounter {
            map: HashTable::with_capacity(capacity),
            random_state: init_hasher_state(),
            string_buffer: Vec::new(),
            count: 0,
            line_mapper,
        }
    }
}

impl<M> InexactHashingLineCounter<M> {
    #[inline(always)]
    fn count(&self) -> usize {
        self.count
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.count = 0;
        self.map.clear();
    }
}

impl CountUnique for InexactHashingLineCounter<()> {
    fn count_line(&mut self, line: &[u8]) {
        let hash = self.random_state.hash_one(line);
        let entry = self.map.entry(hash, |found_hash| *found_hash == hash, |rehash| *rehash);
        entry.or_insert_with(|| {
            self.count += 1;
            hash
        });
    }

    fn count(&self) -> usize {
        InexactHashingLineCounter::count(self)
    }

    fn reset(&mut self) {
        InexactHashingLineCounter::reset(self)
    }
}


impl<M> CountUnique for InexactHashingLineCounter<M>
where
    M: for<'a> FnMut(&'a [u8], &'a mut Vec<u8>) -> &'a [u8],
{
    fn count_line(&mut self, line: &[u8]) {
        let line = (self.line_mapper)(line, &mut self.string_buffer);
        let hash = self.random_state.hash_one(line);
        let entry = self.map.entry(hash, |found_hash| *found_hash == hash, |rehash| *rehash);
        entry.or_insert_with(|| {
            self.count += 1;
            hash
        });
    }

    fn count(&self) -> usize {
        InexactHashingLineCounter::count(self)
    }

    fn reset(&mut self) {
        InexactHashingLineCounter::reset(self)
    }
}
