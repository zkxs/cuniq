// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

#[cfg(feature = "memmap")]
pub mod memmap;
pub mod read;
pub mod util;
#[cfg(feature = "parallel")]
pub mod parallel;


use bstr::io::BufReadExt;
use cfg_if::cfg_if;
use line_cardinality::{CountUnique, Error};
use std::io::BufRead;

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
fn count_unique_in_read<C: CountUnique, T: BufRead>(counter: &C, mut reader: T) -> Result {
    reader
        .for_byte_line(|line| {
            counter.count_line(line);
            Ok(true)
        })
        .map_err(|e| Error::io_static("failed to read from buffer", e))
}

/// Count unique lines in newline-delimited bytes.
fn count_unique_in_bytes<C: CountUnique>(counter: &C, bytes: &[u8]) {
    cfg_if! {
            if #[cfg(feature = "memchr")] {
                for line in LineIterator::new(bytes) {
                    counter.count_line(line);
                }
            } else {
                counter
                    .count_unique_in_read(bytes)
                    .expect("somehow failed to BufRead bytes from memory!?")
            }
        }
}
