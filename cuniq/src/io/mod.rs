// This file is part of cuniq. Copyright © 2025 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use line_cardinality::Error;

#[cfg(feature = "memmap")]
pub mod memmap;
#[cfg(feature = "parallel")]
pub mod parallel;
pub mod read;
pub mod util;

type Result<T> = std::result::Result<T, Error>;

pub(crate) trait LineStream {}

/// Handles streaming line information to a line counter.
pub(crate) trait SingleFileLineStream {}

pub(crate) trait MultiFileLineStream {}

/// Handles the single-threaded case where we do the following in series:
///
/// 1. read a line
/// 2. hash the line
/// 3. count the hash (optionally passing the line as context)
pub(crate) trait SequentialLineStream {}

/// **Handles cases where the line counter is monolithic:**
///
/// data -> chunker -> mpmc -> n*hasher -> mpmc -> counter
pub(crate) trait ParallelLineStream {}

/// **Cases where the line counter is Merge, and we do want to load balance: (e.g. many files)**
/// data -> chunker -> mpmc -> n*hasher,counter -> merge
pub(crate) trait ParallelMergeLineStream {}

/// **Cases where the line counter is Merge, and we don't want to load balance: (e.g. 1 file)**
/// data -> n-split -> n*hasher,counter -> merge
///
/// This has the benefit of being a very simple implementation with less machinery to wire pieces
/// together, but the drawback of not being able to handle all types of input: it requires the input
/// to be Seek, so either a file-based BufReader or a Mmap. Notably NOT Stdin. It's also wouldn't
/// be trivial to figure out the n-split across multiple files, so I simply won't. This mechanism
/// will be useful to benchmark against just because it's simple.
pub(crate) trait ParallelSimpleLineStream {}
