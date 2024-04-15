// This file is part of cuniq. Copyright © 2024 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::constants::CLAP_VERSION;

/// Counts unique lines from newline (\n) delimited input. Input can be provided via stdin and/or
/// file arguments.
#[derive(Parser)]
#[command(version = CLAP_VERSION, about, long_about, author)]
pub struct CliArgs {
    /// Files to process.
    pub files: Vec<PathBuf>,

    /// Instead of printing total unique lines, print a report showing occurrence count of each line.
    /// This is only compatible with "exact" mode (the default).
    #[arg(short = 'c', long)]
    pub report: bool,

    /// Sort report output alphabetically by line. Has no effect unless used with `--report`.
    #[arg(short = 's', long)]
    pub sort: bool,

    /// Remove leading and trailing whitespace from input
    #[arg(short, long)]
    pub trim: bool,

    /// Convert input to lowercase
    #[arg(short, long = "lower")]
    pub lowercase: bool,

    /// Sets the algorithm used to count (or estimate) cardinality.
    #[arg(value_enum, short = 'm', long, default_value_t)]
    pub mode: Mode,

    /// Set the size used by the selected counting mode. See the `--mode` documentation for how this
    /// affects each counting mode.
    #[arg(short = 'n', long)]
    pub size: Option<usize>,

    /// Set the number of threads used to perform the count. By default, the number of logical cores
    /// is used.
    /// Not all counting modes support parallelism: see `--mode` for details.
    #[arg(long)]
    pub threads: Option<usize>,

    /// Disable checking stdin for input. May yield a small performance improvement when only
    /// reading input from files.
    #[arg(long)]
    pub no_stdin: bool,

    /// Force reading files via memmap. This may yield improved performance for large files. If the
    /// binary was built without memmap support, using this flag will result in an error.
    #[arg(long)]
    pub memmap: bool,

    /// Disable reading files via memmap, instead falling back to normal reads. By default, cuniq
    /// will try to use memmap if it thinks it will be faster. Disabling memmap may yield improved
    /// performance for small files.
    #[arg(long)]
    pub no_memmap: bool,
}

/// Mode used to calculate cardinality
#[derive(ValueEnum, Clone, Default)]
pub enum Mode {
    /// Uses a hash table to exactly count cardinality.
    /// The size of the hash table is proportional to the cardinality of the input.
    /// You may use the `--size` flag to set the initial capacity of the internal hash table. For
    /// very large inputs `--size` may help reduce expensive hash table reallocations. Avoid setting
    /// `--size` for small datasets.
    #[default]
    Exact,
    /// Uses a hash table to exactly count cardinality, but does not store the original line.
    /// This mode is faster than "exact" mode, but hash collision will result in under-counting the
    /// cardinality by one. However, hash collisions for a 64-bit hash are exceedingly unlikely.
    /// The size of the hash table is proportional to the cardinality of the input.
    /// You may use the `--size` flag to set the initial capacity of the internal hash table. For
    /// very large inputs `--size` may help reduce expensive hash table reallocations. Avoid setting
    /// `--size` for small datasets. This mode is not compatible with `--report`.
    NearExact,
    /// Uses the HyperLogLog algorithm to estimate cardinality with fixed memory.
    /// Use the `--size` flag to specify the number of 1-byte registers to use. More registers will
    /// increase estimate accuracy. By default, 65536 is used. This mode is not compatible with
    /// `--report`.
    Estimate,
}

impl Display for Mode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Mode::Exact => "exact",
            Mode::NearExact => "near-exact",
            Mode::Estimate => "estimate",
        };
        f.write_str(str)
    }
}
