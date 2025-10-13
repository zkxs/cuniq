// This file is part of cuniq. Copyright © 2025 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use line_cardinality::{CountUniqueHash, CountUniqueLineHash, Error, Merge};

pub(crate) mod buf;
#[cfg(feature = "memmap")]
pub(crate) mod memmap;
#[cfg(feature = "parallel")]
pub(crate) mod parallel;
pub(crate) mod read;
pub(crate) mod util;

type Result<T> = std::result::Result<T, Error>;

/// Line & !Hash & !Merge
///
/// Used for normal hashtable implementation
pub(crate) struct ByLine<T: CountUniqueLineHash>(pub(crate) T);

/// !Line & Hash & !Merge
///
/// Used for hash-only hashtable implementation
pub(crate) struct ByHash<T: CountUniqueHash>(pub(crate) T);

/// !Line & Hash & Merge
///
/// Used for HyperLogLog
#[derive(Clone)]
pub(crate) struct ByMerge<T: CountUniqueHash + Merge + Clone>(pub(crate) T);
