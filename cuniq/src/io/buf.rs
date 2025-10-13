// This file is part of cuniq. Copyright © 2025 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! Single-threaded buffer-based file processing

use crate::io::util::LineIterator;
use crate::io::{ByHash, ByLine, ByMerge};
use bstr::io::BufReadExt;
use line_cardinality::{CountUniqueHash, CountUniqueLineHash, Error, Merge};
use std::hash::BuildHasher;
use std::io::BufRead;

pub(crate) trait CountBuf {
    /// Count unique lines in a newline-delimited [`BufRead`].
    ///
    /// ```rust
    /// use line_cardinality::{CountUnique, LineCounter};
    ///
    /// // grab some test data
    /// let data = b"three\ntwo\nthree\ntwo\nthree\none";
    /// let mut reader = data.as_slice();
    ///
    /// // run the unique line count
    /// let mut line_counter = LineCounter::new();
    /// line_counter.count_unique_in_read(&mut reader).unwrap();
    ///
    /// // we expect there to be 3 distinct lines in this file
    /// assert_eq!(line_counter.count(), 3);
    /// ```
    ///
    /// Note that this can also be used to read [`Stdin`](std::io::Stdin):
    ///
    /// ```rust
    /// use line_cardinality::{CountUnique, LineCounter};
    ///
    /// let mut reader = std::io::stdin().lock();
    ///
    /// // run the unique line count
    /// let mut line_counter = LineCounter::new();
    /// line_counter.count_unique_in_read(&mut reader).unwrap();
    ///
    /// // we didn't send anything over stdin
    /// assert_eq!(line_counter.count(), 0);
    /// ```
    fn count_unique_in_read(&mut self, random_state: &impl BuildHasher, reader: impl BufRead) -> crate::io::Result<()>;

    /// Count unique lines in newline-delimited bytes.
    fn count_unique_in_bytes(&mut self, random_state: &impl BuildHasher, bytes: &[u8]);
}

impl<C> CountBuf for ByLine<C>
where
    C: CountUniqueLineHash,
{
    fn count_unique_in_read(
        &mut self,
        random_state: &impl BuildHasher,
        mut reader: impl BufRead,
    ) -> crate::io::Result<()> {
        reader
            .for_byte_line(|line| {
                let hash = random_state.hash_one(line);
                self.0.count_line(line, hash, |line| random_state.hash_one(line));
                Ok(true)
            })
            .map_err(|e| Error::io_static("failed to read from buffer", e))
    }

    fn count_unique_in_bytes(&mut self, random_state: &impl BuildHasher, bytes: &[u8]) {
        for line in LineIterator::new(bytes) {
            let hash = random_state.hash_one(line);
            self.0.count_line(line, hash, |line| random_state.hash_one(line));
        }
    }
}

impl<C> CountBuf for ByHash<C>
where
    C: CountUniqueHash,
{
    fn count_unique_in_read(&mut self, random_state: &impl BuildHasher, reader: impl BufRead) -> crate::io::Result<()> {
        hash_count_unique_in_read(&mut self.0, random_state, reader)
    }

    fn count_unique_in_bytes(&mut self, random_state: &impl BuildHasher, bytes: &[u8]) {
        hash_count_unique_in_bytes(&mut self.0, random_state, bytes)
    }
}

impl<C> CountBuf for ByMerge<C>
where
    C: CountUniqueHash + Merge + Clone,
{
    fn count_unique_in_read(&mut self, random_state: &impl BuildHasher, reader: impl BufRead) -> crate::io::Result<()> {
        hash_count_unique_in_read(&mut self.0, random_state, reader)
    }

    fn count_unique_in_bytes(&mut self, random_state: &impl BuildHasher, bytes: &[u8]) {
        hash_count_unique_in_bytes(&mut self.0, random_state, bytes)
    }
}

fn hash_count_unique_in_read<C: CountUniqueHash, T: BufRead>(
    processor: &mut C,
    random_state: &impl BuildHasher,
    mut reader: T,
) -> crate::io::Result<()> {
    reader
        .for_byte_line(|line| {
            let hash = random_state.hash_one(line);
            processor.count_hash(hash);
            Ok(true)
        })
        .map_err(|e| Error::io_static("failed to read from buffer", e))
}

fn hash_count_unique_in_bytes<C: CountUniqueHash>(processor: &mut C, random_state: &impl BuildHasher, bytes: &[u8]) {
    for line in LineIterator::new(bytes) {
        let hash = random_state.hash_one(line);
        processor.count_hash(hash);
    }
}
