// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! line_cardinality provides utilities to count or estimate unique lines from input data. It can read from a
//! [`BufRead`] (such as stdin) or a file using optimized file reading functions.
//!
//! Note line_cardinality only supports newline (`\n`) delimited input and does not perform any
//! UTF-8 validation: all lines are compared by byte value alone.
//!
//! Examples of counting total distinct lines can be found in [`CountUnique`].
//!
//! Examples of reporting occurrences of each distinct line can be found in [`ReportUniqueLineHash`].

// export things nested deeper within our module structure at the top-level of this crate
pub use count_unique_impl::hashtable_lossless::{
    HashingLineCounterIntoIter, HashingLineCounterIter, LosslessHashingLineCounter,
};
pub use count_unique_impl::hashtable_lossy::LossyHashingLineCounter;
pub use count_unique_impl::hyperloglog::HyperLogLog;
pub use count_unique_impl::increment::Increment;
pub use count_unique_impl::result::Cause as ErrorCause;
pub use count_unique_impl::result::Error;

pub(crate) mod count_unique_impl;

/// Functionality to count total unique lines.
///
/// A typical example:
///
/// ```rust
/// use std::hash::{BuildHasher, RandomState};
/// use line_cardinality::{CountUnique, CountUniqueLineHash, LosslessHashingLineCounter};
///
/// // some setup
/// let hasher = RandomState::new();
///
/// // grab some test data
/// let data = b"three\ntwo\nthree\ntwo\nthree\none";
///
/// // run the unique line count
/// let mut line_counter = LosslessHashingLineCounter::<()>::new();
/// for line in data.split(|byte| *byte == b'\n') {
///     let hash = hasher.hash_one(line);
///     line_counter.count_line(line, hash, |line| hasher.hash_one(line));
/// }
///
/// // we expect there to be 3 distinct lines in this file
/// assert_eq!(line_counter.count(), 3);
/// ```
pub trait CountUnique {
    /// Returns current cardinality count of the [`CountUnique`].
    fn count(&self) -> usize;

    /// Resets internal state of this [`CountUnique`] for reuse
    fn reset(&mut self);
}

/// A [`CountUnique`] that stores line and hash information. This enables lossless handling of hash
/// collisions and reporting of counts per-line, but incurs an extra memory cost.
pub trait CountUniqueLineHash: CountUnique {
    /// Count a single line, incrementing counters if it is the first occurrence of that line.
    ///
    /// `hasher` is called if entries need to be moved or copied to a new table.
    /// This must return the same hash value that each entry was inserted with.
    fn count_line(&mut self, line: &[u8], hash: u64, hasher: impl Fn(&[u8]) -> u64);
}

/// A [`CountUnique`] that only stores hash and not line information. This enables algorithms
/// that have increasing memory-efficiency in exchange for decreasing precision.
pub trait CountUniqueHash: CountUnique {
    fn count_hash(&mut self, hash: u64);
}

/// A [`CountUnique`] that can be cheaply merged with another `CountUnique` of the same type. Notably, this
/// allows simple parallel implementations as the states can be merged at the end of the counting phase.
pub trait Merge: CountUnique {
    fn merge(&mut self, other: &Self);
}

/// Functionality to emit lines from a [`CountUnique`]
pub trait EmitLines: CountUnique {
    /// `f` is called for each map entry.
    fn for_each_line<L>(&self, f: L)
    where
        L: FnMut(&[u8]);

    /// Consume this [`EmitLines`] and convert it into a [`Vec`]
    fn into_vec(self) -> Vec<Vec<u8>>;
}

/// Functionality to count occurrences of each line. `T` is the counter type used.
///
/// ```rust
/// use std::hash::{BuildHasher, RandomState};
/// use line_cardinality::{CountUnique, CountUniqueLineHash, LosslessHashingLineCounter, ReportUniqueLineHash};
///
/// // some setup
/// let hasher = RandomState::new();
///
/// // grab some test data
/// let data = b"three\ntwo\nthree\ntwo\nthree\none";
///
/// // run the unique line count
/// let mut line_counter = LosslessHashingLineCounter::<u64>::new();
/// for line in data.split(|byte| *byte == b'\n') {
///     let hash = hasher.hash_one(line);
///     line_counter.count_line(line, hash, |line| hasher.hash_one(line));
/// }
///
/// // we can get occurrence counts for individual lines
/// let line = b"one".as_slice();
/// assert!(matches!(line_counter.get(line, hasher.hash_one(line)), Some(1)));
/// let line = b"two".as_slice();
/// assert!(matches!(line_counter.get(line, hasher.hash_one(line)), Some(2)));
/// let line = b"three".as_slice();
/// assert!(matches!(line_counter.get(line, hasher.hash_one(line)), Some(3)));
///
/// // we can also get the total number of distinct lines in the file
/// assert_eq!(line_counter.count(), 3);
/// ```
pub trait ReportUniqueLineHash<C>: CountUniqueLineHash
where
    C: Increment,
{
    /// `f` is called for each map entry.
    fn for_each_report_entry<F: FnMut(&[u8], C)>(&self, f: F);

    /// Consume this [`ReportUniqueLineHash`] and convert it into a [`Vec`]. This function has overhead, as
    /// it has to allocate a new Vec.
    fn to_report_vec(self) -> Vec<(Vec<u8>, C)>;

    /// Get the occurrence count for a specific line
    fn get(&self, line: &[u8], hash: u64) -> Option<C>;

    /// Convert this [`ReportUniqueLineHash`] into a borrowed iter over each entry
    fn iter(&self) -> HashingLineCounterIter<C>;

    /// Convert this [`ReportUniqueLineHash`] into an owned iter over each entry
    fn into_iter(self) -> HashingLineCounterIntoIter<C>;
}
