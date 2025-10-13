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
