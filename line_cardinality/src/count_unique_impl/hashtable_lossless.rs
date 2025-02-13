// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use hashbrown::HashTable;
use std::ops::Deref;

use crate::{CountUnique, CountUniqueLineHash, EmitLines, Increment, ReportUniqueLineHash};

struct Entry<T> {
    line: Box<[u8]>,
    counter: T,
}

/// Calculates the unique count and holds necessary state.
///
/// Internally, a [`HashMap`] is created that contains an entry for each distinct line in the input.
/// This may be expensive to drop if it contains a large amount of processed data, so using
/// [`std::mem::forget`] may be worth considering if your application will terminate immediately
/// after finishing the unique-counting work.
///
/// This implementation also has accepts a customizable `line_mapper` function with
/// [`LosslessHashingLineCounter::with_line_mapper`]. If provided, this function will be applied to each
/// line before checking if it is unique or not. Note that this also affects the output that will be
/// seen from functions that enumerate internal state, such as [`EmitLines::for_each_line`].
pub struct LosslessHashingLineCounter<T> {
    map: HashTable<Entry<T>>,
    count: usize,
}

impl<C> Default for LosslessHashingLineCounter<C> {
    fn default() -> Self {
        Self::new()
    }
}

/// Constructors that do not take a custom line mapper
impl<C> LosslessHashingLineCounter<C> {
    /// Creates a new [`LosslessHashingLineCounter`].
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates a new [`LosslessHashingLineCounter`] with a cardinality hint of `capacity`.
    ///
    /// Note that it is best to leave `capacity` unset unless you have a near-perfect idea of your
    /// data's cardinality lower bound, as it is extremely difficult to gain performance by setting
    /// it, but extremely easy to lose performance.
    pub fn with_capacity(capacity: usize) -> Self {
        LosslessHashingLineCounter {
            map: HashTable::with_capacity(capacity),
            count: 0,
        }
    }
}

impl<C> CountUnique for LosslessHashingLineCounter<C> {
    fn count(&self) -> usize {
        self.count
    }

    fn reset(&mut self) {
        self.count = 0;
        self.map.clear();
    }
}

impl CountUniqueLineHash for LosslessHashingLineCounter<()> {
    fn count_line(&mut self, line: &[u8], hash: u64, hasher: impl Fn(&[u8]) -> u64) {
        let entry = self.map.entry(
            hash,
            |entry| line == entry.line.deref(),
            |entry| {
                let slice: &[u8] = &entry.line;
                hasher(slice)
            },
        );
        entry.or_insert_with(|| {
            self.count += 1;
            Entry {
                line: line.to_vec().into_boxed_slice(),
                counter: (),
            }
        });
    }
}

impl<C> CountUniqueLineHash for LosslessHashingLineCounter<C>
where
    C: Increment,
{
    fn count_line(&mut self, line: &[u8], hash: u64, hasher: impl Fn(&[u8]) -> u64) {
        let entry = self.map.entry(
            hash,
            |entry| line == entry.line.deref(),
            |entry| {
                let slice: &[u8] = &entry.line;
                hasher(slice)
            },
        );
        entry
            .and_modify(|entry| entry.counter.increment())
            .or_insert_with(|| {
                self.count += 1;
                Entry {
                    line: line.to_vec().into_boxed_slice(),
                    counter: C::new(),
                }
            });
    }
}

impl<C> EmitLines for LosslessHashingLineCounter<C> {
    fn for_each_line<F>(&self, f: F)
    where
        F: FnMut(&[u8]),
    {
        self.map.iter().map(|entry| entry.line.deref()).for_each(f);
    }

    fn into_vec(self) -> Vec<Vec<u8>> {
        self.map
            .into_iter()
            .map(|entry| entry.line)
            .map(|line| line.into_vec())
            .collect()
    }
}

impl<C> ReportUniqueLineHash<C> for LosslessHashingLineCounter<C>
where
    C: Increment,
{
    fn for_each_report_entry<F: FnMut(&[u8], C)>(&self, mut f: F) {
        self.map
            .iter()
            .for_each(|entry| f(&entry.line, entry.counter));
    }

    fn to_report_vec(self) -> Vec<(Vec<u8>, C)> {
        self.map
            .into_iter()
            .map(|entry| (entry.line.into_vec(), entry.counter))
            .collect()
    }

    fn get(&self, line: &[u8], hash: u64) -> Option<C> {
        self.map
            .find(hash, |entry| line == entry.line.deref())
            .map(|entry| entry.counter)
    }

    fn iter(&self) -> HashingLineCounterIter<C> {
        HashingLineCounterIter {
            inner: self.map.iter(),
        }
    }

    fn into_iter(self) -> HashingLineCounterIntoIter<C> {
        HashingLineCounterIntoIter {
            inner: self.map.into_iter(),
        }
    }
}

impl<'a, C> IntoIterator for &'a LosslessHashingLineCounter<C> {
    type Item = (&'a [u8], &'a C);
    type IntoIter = HashingLineCounterIter<'a, C>;

    fn into_iter(self) -> Self::IntoIter {
        HashingLineCounterIter {
            inner: self.map.iter(),
        }
    }
}

/// A borrowing iter over report entries.
///
/// Currently implemented as a wrapper around [`hashbrown::hash_table::Iter`]. This is done to
/// avoid breaking changes if the internal map implementation changes.
pub struct HashingLineCounterIter<'a, C> {
    inner: hashbrown::hash_table::Iter<'a, Entry<C>>,
}

/// wrapper around [`hashbrown::hash_table::Iter`]'s Iterator impl
impl<'a, C> Iterator for HashingLineCounterIter<'a, C> {
    type Item = (&'a [u8], &'a C);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|entry| (entry.line.deref(), &entry.counter))
    }
}

/// An owned iter over report entries.
///
/// Currently implemented as a wrapper around [`hashbrown::hash_table::IntoIter`]. This is done to
/// avoid breaking changes if the internal map implementation changes.
pub struct HashingLineCounterIntoIter<C> {
    inner: hashbrown::hash_table::IntoIter<Entry<C>>,
}

/// wrapper around [`hashbrown::hash_table::IntoIter`]'s Iterator impl
impl<C> Iterator for HashingLineCounterIntoIter<C> {
    type Item = (Vec<u8>, C);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|entry| (entry.line.into_vec(), entry.counter))
    }
}
