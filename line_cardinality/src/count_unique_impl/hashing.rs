// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use hashbrown::HashMap;

use crate::{CountUnique, EmitLines, Increment, ReportUnique};

use super::{init_hasher_state, RandomState};

/// Calculates the unique count and holds necessary state.
///
/// Internally, a [`HashMap`] is created that contains an entry for each distinct line in the input.
/// This may be expensive to drop if it contains a large amount of processed data, so using
/// [`std::mem::forget`] may be worth considering if your application will terminate immediately
/// after finishing the unique-counting work.
///
/// This implementation also has accepts a customizable `line_mapper` function with
/// [`HashingLineCounter::with_line_mapper`]. If provided, this function will be applied to each
/// line before checking if it is unique or not. Note that this also affects the output that will be
/// seen from functions that enumerate internal state, such as [`EmitLines::for_each_line`].
pub struct HashingLineCounter<T, M> {
    map: HashMap<Vec<u8>, T, RandomState>,
    string_buffer: Vec<u8>,
    count: usize,
    line_mapper: M,
}

impl<T> Default for HashingLineCounter<T, ()> {
    fn default() -> Self {
        Self::new()
    }
}

/// Constructors that do not take a custom line mapper
impl<T> HashingLineCounter<T, ()> {
    /// Creates a new [`HashingLineCounter`].
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates a new [`HashingLineCounter`] with a cardinality hint of `capacity`.
    ///
    /// Note that it is best to leave `capacity` unset unless you have a near-perfect idea of your
    /// data's cardinality lower bound, as it is extremely difficult to gain performance by setting
    /// it, but extremely easy to lose performance.
    pub fn with_capacity(capacity: usize) -> Self {
        HashingLineCounter {
            map: HashMap::with_capacity_and_hasher(capacity, init_hasher_state()),
            string_buffer: Vec::new(),
            count: 0,
            line_mapper: (),
        }
    }
}

/// Constructors that take a custom line mapper
impl<T, M> HashingLineCounter<T, M>
where
    M: for<'a> FnMut(&'a [u8], &'a mut Vec<u8>) -> &'a [u8],
{
    /// Creates a new [`HashingLineCounter`] with a custom `line_mapper` function which will be applied to
    /// each read line before counting.
    pub fn with_line_mapper(line_mapper: M) -> Self {
        Self::with_line_mapper_and_capacity(line_mapper, 0)
    }

    /// Creates a new [`HashingLineCounter`] with a cardinality hint of `capacity` and a custom
    /// `line_mapper` function which will be applied to each read line before counting.
    ///
    /// Note that it is best to leave `capacity` unset unless you have a near-perfect idea of your
    /// data's cardinality lower bound, as it is extremely difficult to gain performance by setting
    /// it, but extremely easy to lose performance.
    pub fn with_line_mapper_and_capacity(line_mapper: M, capacity: usize) -> Self {
        HashingLineCounter {
            map: HashMap::with_capacity_and_hasher(capacity, init_hasher_state()),
            string_buffer: Vec::new(),
            count: 0,
            line_mapper,
        }
    }
}

impl<T, M> HashingLineCounter<T, M> {
    fn count(&self) -> usize {
        self.count
    }

    fn reset(&mut self) {
        self.count = 0;
        self.map.clear();
    }
}

impl CountUnique for HashingLineCounter<(), ()> {
    #[inline(always)]
    fn count_line(&mut self, line: &[u8]) {
        self.map.raw_entry_mut()
            .from_key(line)
            .or_insert_with(|| {
                self.count += 1;
                (line.to_vec(), ())
            });
    }

    fn count(&self) -> usize {
        HashingLineCounter::count(self)
    }

    fn reset(&mut self) {
        HashingLineCounter::reset(self)
    }
}

/// Non-reporting [`CountUnique`] implementation that doesn't tabulate report counts: only total count
impl<M> CountUnique for HashingLineCounter<(), M>
where
    M: for<'a> FnMut(&'a [u8], &'a mut Vec<u8>) -> &'a [u8],
{
    #[inline(always)]
    fn count_line(&mut self, line: &[u8]) {
        let line = (self.line_mapper)(line, &mut self.string_buffer);
        self.map.raw_entry_mut()
            .from_key(line)
            .or_insert_with(|| {
                self.count += 1;
                (line.to_vec(), ())
            });
    }

    fn count(&self) -> usize {
        HashingLineCounter::count(self)
    }

    fn reset(&mut self) {
        HashingLineCounter::reset(self)
    }
}

impl<C> CountUnique for HashingLineCounter<C, ()>
where
    C: Increment,
{
    #[inline(always)]
    fn count_line(&mut self, line: &[u8]) {
        self.map.raw_entry_mut()
            .from_key(line)
            .and_modify(|_line, count| count.increment())
            .or_insert_with(|| {
                self.count += 1;
                (line.to_vec(), C::new())
            });
    }

    fn count(&self) -> usize {
        HashingLineCounter::count(self)
    }

    fn reset(&mut self) {
        HashingLineCounter::reset(self)
    }
}

/// Reporting [`CountUnique`] implementation that tabulates report counts as well as total count
impl<C, M> CountUnique for HashingLineCounter<C, M>
where
    C: Increment,
    M: for<'a> FnMut(&'a [u8], &'a mut Vec<u8>) -> &'a [u8],
{
    #[inline(always)]
    fn count_line(&mut self, line: &[u8]) {
        let line = (self.line_mapper)(line, &mut self.string_buffer);
        self.map.raw_entry_mut()
            .from_key(line)
            .and_modify(|_line, count| count.increment())
            .or_insert_with(|| {
                self.count += 1;
                (line.to_vec(), C::new())
            });
    }

    fn count(&self) -> usize {
        HashingLineCounter::count(self)
    }

    fn reset(&mut self) {
        HashingLineCounter::reset(self)
    }
}

impl<T, M> EmitLines for HashingLineCounter<T, M>
where
    HashingLineCounter<T, M>: CountUnique,
{
    fn for_each_line<F>(&self, f: F)
    where
        F: FnMut(&[u8]),
    {
        self.map.keys()
            .map(|line| line.as_slice())
            .for_each(f);
    }

    fn into_vec(self) -> Vec<Vec<u8>> {
        self.map.into_keys().collect()
    }
}

impl<C, M> ReportUnique<C> for HashingLineCounter<C, M>
where
    C: Increment,
{
    fn for_each_report_entry<F: FnMut(&[u8], C)>(&self, mut f: F) {
        self.map.iter()
            .for_each(|(line, count)| f(line, *count));
    }

    fn to_report_vec(self) -> Vec<(Vec<u8>, C)> {
        self.map.into_iter().collect()
    }

    fn get(&self, line: &[u8]) -> Option<C> {
        self.map.get(line).copied()
    }

    fn iter(&self) -> HashingLineCounterIter<C> {
        HashingLineCounterIter { inner: self.map.iter() }
    }

    fn into_iter(self) -> HashingLineCounterIntoIter<C> {
        HashingLineCounterIntoIter { inner: self.map.into_iter() }
    }
}

impl<'a, C, M> IntoIterator for &'a HashingLineCounter<C, M>
where
    C: Increment,
{
    type Item = (&'a [u8], &'a C);
    type IntoIter = HashingLineCounterIter<'a, C>;

    fn into_iter(self) -> Self::IntoIter {
        HashingLineCounterIter { inner: self.map.iter() }
    }
}

/// A borrowing iter over report entries.
///
/// Currently implemented as a wrapper around [`hashbrown::hash_map::Iter`]. This is done to
/// avoid breaking changes if the internal map implementation changes.
pub struct HashingLineCounterIter<'a, C> {
    inner: hashbrown::hash_map::Iter<'a, Vec<u8>, C>,
}

/// wrapper around [`hashbrown::hash_map::Iter`]'s Iterator impl
impl<'a, C> Iterator for HashingLineCounterIter<'a, C>
where
    C: Increment,
{
    type Item = (&'a [u8], &'a C);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(key, value)| (key.as_slice(), value))
    }
}

/// An owned iter over report entries.
///
/// Currently implemented as a wrapper around [`hashbrown::hash_map::IntoIter`]. This is done to
/// avoid breaking changes if the internal map implementation changes.
pub struct HashingLineCounterIntoIter<C> {
    inner: hashbrown::hash_map::IntoIter<Vec<u8>, C>,
}

/// wrapper around [`hashbrown::hash_map::IntoIter`]'s Iterator impl
impl<C> Iterator for HashingLineCounterIntoIter<C>
where
    C: Increment,
{
    type Item = (Vec<u8>, C);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
