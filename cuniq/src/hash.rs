// This file is part of cuniq. Copyright © 2025 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

pub(crate) use ahash::RandomState;

use cfg_if::cfg_if;

/// Handle getting a hasher for various hasher and RNG feature flag settings.
pub(crate) fn init_hasher_state() -> RandomState {
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
}
