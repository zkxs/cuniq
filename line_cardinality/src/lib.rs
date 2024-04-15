// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! line_cardinality provides utilities to count unique lines from input data. It can read from a
//! [`BufRead`] (such as stdin) or a file using optimized file reading functions.
//!
//! Note line_cardinality only supports newline (`\n`) delimited input and does not perform any
//! UTF-8 validation: all lines are compared by byte value alone.

#![feature(hash_raw_entry)]

use std::collections::HashMap;
use std::io::BufRead;

use bstr::io::BufReadExt;
use cfg_if::cfg_if;

#[cfg(all(feature = "file", feature = "memmap"))]
pub use count_unique_impl::file_io::memmap::CountUniqueFromMemmapFile;
#[cfg(feature = "file")]
pub use count_unique_impl::file_io::read::CountUniqueFromReadFile;
pub use count_unique_impl::hashing::HashingLineCounter;
#[cfg(feature = "hash-only")]
pub use count_unique_impl::hashing_inexact::InexactHashingLineCounter;
pub use count_unique_impl::hyperloglog::HyperLogLog;
use count_unique_impl::RandomState;
pub use count_unique_impl::result::Cause as ErrorCause;
pub use count_unique_impl::result::Error;
use count_unique_impl::result::Result;

pub(crate) mod count_unique_impl;

/// A [`CountUnique`] that does not track each line's occurrence count, but is still
/// useful for finding the total number of distinct lines in the input data.
pub type LineCounter<M> = HashingLineCounter<(), M>;

/// Functionality to count total unique lines.
///
/// A typical example:
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
/// You may also wish to pre-process your input. For example, to trim whitespace from input:
///
/// ```rust
/// use bstr::ByteSlice;
/// use line_cardinality::{CountUnique, LineCounter};
///
/// let data = b"foo \n foo\nbar\nbar \nfoo\t\nfoo";
/// let mut reader = data.as_slice();
///
/// let mut line_counter = LineCounter::with_line_mapper(|line, buffer| {
///     line.trim()
/// });
///
/// line_counter.count_unique_in_read(&mut reader).unwrap();
///
/// assert_eq!(line_counter.count(), 2);
/// ```
///
/// Or a slightly more complex example, converting input to lowercase:
///
/// ```rust
/// use bstr::ByteSlice;
/// use line_cardinality::{CountUnique, LineCounter};
///
/// let data = b"FOO\nfoo\nBAR\nbar\nFOO\nFOO";
/// let mut reader = data.as_slice();
///
/// let mut line_counter = LineCounter::with_line_mapper(|line, buffer| {
///     buffer.clear();
///     line.to_lowercase_into(buffer);
///     buffer
/// });
///
/// line_counter.count_unique_in_read(&mut reader).unwrap();
///
/// assert_eq!(line_counter.count(), 2);
/// ```
///
/// `buffer` here is simply a reference to a growable buffer which you may optionally use in your processing.
/// This is done to avoid unnecessary allocations.
pub trait CountUnique: Sized {
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
    fn count_unique_in_read<T: BufRead>(&mut self, mut reader: T) -> Result {
        reader.for_byte_line(|line| {
            self.count_line(line);
            Ok(true)
        }).map_err(|e| Error::io_static("failed to read from buffer", e))
    }

    /// Count unique lines in newline-delimited bytes.
    fn count_unique_in_bytes(&mut self, bytes: &[u8]) {
        cfg_if! {
            if #[cfg(feature = "memchr")] {
                let mut start: usize = 0;
                for newline_index in memchr::memchr_iter(b'\n', bytes) {
                    self.count_line(&bytes[start..newline_index]);
                    start = newline_index + 1;
                }
                // handle trailing
                if start < bytes.len() {
                    self.count_line(&bytes[start..]);
                }
            } else {
                self.count_unique_in_read(bytes).unwrap()
            }
        }
    }

    /// Count a single line, incrementing counters if it is the first occurrence of that line.
    fn count_line(&mut self, line: &[u8]);

    /// Returns current cardinality count of the [`CountUnique`].
    fn count(&self) -> usize;

    /// Resets internal state of this [`CountUnique`] for reuse
    fn reset(&mut self);
}

/// Functionality to emit lines from a [`CountUnique`]
pub trait EmitLines {
    /// `f` is called for each map entry.
    fn for_each_line<L>(&self, f: L)
    where
        L: FnMut(&[u8]);

    /// Consume this [`EmitLines`] and convert it into a [`Vec`]
    fn into_vec(self) -> Vec<Vec<u8>>;
}

/// Functionality to count occurrences of each line
///
/// ```rust
/// use line_cardinality::{CountUnique, HashingLineCounter, ReportUnique};
///
/// // grab some test data
/// let data = b"three\ntwo\nthree\ntwo\nthree\none";
///
/// // run the unique line count
/// let mut line_counter = HashingLineCounter::<u64, _>::new();
/// line_counter.count_unique_in_read(data.as_slice()).unwrap();
///
/// // we can get occurrence counts for individual lines
/// assert!(matches!(line_counter.as_map().get(b"one".as_slice()).as_ref(), Some(1)));
/// assert!(matches!(line_counter.as_map().get(b"two".as_slice()).as_ref(), Some(2)));
/// assert!(matches!(line_counter.as_map().get(b"three".as_slice()).as_ref(), Some(3)));
///
/// // we can also get the total number of distinct lines in the file
/// assert_eq!(line_counter.count(), 3);
/// ```
pub trait ReportUnique<T> {
    /// `f` is called for each map entry.
    fn for_each_report_entry<F: FnMut((&[u8], &T))>(&self, f: F);

    /// Consume this [`ReportUnique`] and convert it into a [`Vec`]. This function has overhead, as
    /// it has to allocate a new Vec.
    fn to_report_vec(self) -> Vec<(Vec<u8>, T)>;

    /// Access the underlying map this [`ReportUnique`] is using
    fn as_map(&self) -> &HashMap<Vec<u8>, T, RandomState>;

    /// Extract the underling map from this [`ReportUnique`], consuming it in the process.
    fn into_map(self) -> HashMap<Vec<u8>, T, RandomState>;
}

/// A type that can count occurrences of a line
pub trait Increment {
    /// Increment the current count
    fn increment(&mut self);

    /// Create a new counter with the default starting value for a single entry found
    fn new() -> Self;

    /// Return the current count
    fn count(&self) -> &Self {
        self
    }
}
