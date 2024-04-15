// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ahash")] {
        pub(crate) use ahash::RandomState;
    } else {
        pub(crate) use std::hash::RandomState;
    }
}

pub(crate) mod increment;
#[cfg(feature = "file")]
pub(crate) mod file_io;
pub(crate) mod hashing;
#[cfg(feature = "hash-only")]
pub(crate) mod hashing_inexact;
pub(crate) mod hyperloglog;
pub(crate) mod result;

/// Handle getting a hasher for various hasher and RNG feature flag settings.
pub(crate) fn init_hasher_state() -> RandomState {
    cfg_if! {
        if #[cfg(feature = "ahash")] {
            cfg_if! {
                if #[cfg(feature = "compile-time-rng")] {
                    Default::default()
                } else {
                    // Pre-generated random seed that matches the one in the binary benchmarks.
                    // Hopefully this will remove RNG from the benchmarking.
                    RandomState::with_seeds(
                        0xD4D1C62E748C6F9F,
                        0x6AB3CDB8BD6660B5,
                        0x252E7AFD38FC5B30,
                        0xD47C5724DAD72AD1,
                    )
                }
            }
        } else {
            Default::default()
        }
    }
}
