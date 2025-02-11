// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

#[cfg(feature = "memmap")]
pub(crate) mod memmap;
pub(crate) mod read;
pub(crate) mod util;
#[cfg(feature = "parallel")]
pub(crate) mod parallel;
