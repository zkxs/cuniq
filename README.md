# cuniq

**The pitch**: cuniq is a dedicated command line tool for counting unique lines in text input. If you find yourself
frequently running commands like `sort -u | wc -l` or `sort | uniq -c` you will find improved performance by using cuniq
instead.

**The anti-pitch**: For small inputs you're fine using sort and uniq, as we're talking millisecond-savings by switching
to cuniq. However, if you've been using `sort | uniq | wc -l` you should switch to `sort -u | wc -l`, as it's free
performance gain without having to go outside standard POSIX commands.

## Performance

cuniq has been benchmarked against various combinations of GNU coreutils (sort, uniq, and wc) as well as other
hashing-based Rust utilities [runiq](https://crates.io/crates/runiq), [sortuniq](https://crates.io/crates/sortuniq),
and [huniq](https://crates.io/crates/huniq).
As of this writing, you should not use runiq 2.0.0 or sortuniq 0.2.0 for counting unique lines: they underperform cuniq
in all cases, and in many cases their performance is on par with or even worse than `sort -u | wc -l`.

For **counting** cuniq reliably outperforms GNU sort in all cases.

For **reporting line occurrence counts** cuniq reliably outperforms GNU uniq in all cases except one:

> [!NOTE]
> If your input has extremely few duplicates and you want a sorted report, than you're better off using `sort | uniq c`.
> This is because with extremely few duplicates both approaches must sort nearly all of the input, but cuniq also wastes
> time building a hash table.

More data and technical details on the benchmarking and profile-guided optimization that went into creating cuniq are
available in [PERFORMANCE.md](PERFORMANCE.md).

## Compatibility

cuniq has compatible output with corresponding GNU coreutils commands:

| GNU coreutils command   | cuniq equivalent | Effect                                | Notes                                                        |
|-------------------------|------------------|---------------------------------------|--------------------------------------------------------------|
| `sort \| uniq \| wc -l` | `cuniq`          | Count of unique lines                 |                                                              |
| `sort -u \| wc -l`      | `cuniq`          | Count of unique lines                 | this GNU coreutils command is more performant than the above |
| `sort \| uniq -c`       | `cuniq -c`       | Unsorted report of unique line counts | output order differs between the two commands                |
| `sort \| uniq -c`       | `cuniq -cs`      | Sorted report of unique line counts   |                                                              |

## Install

### Installing from Source

1. [Install Rust](https://www.rust-lang.org/tools/install). Nightly toolchain is required pending stabilization of the `hash_raw_entry` feature.
2. `RUSTFLAGS="-C target-cpu=native" cargo +nightly install cuniq`

### Manual Installation

1. Download cuniq from the [latest release](https://github.com/zkxs/cuniq/releases/latest), and save it to a location of your choice

## Usage

cuniq can accept lines from stdin or from a list of files.

```
Usage: cuniq [OPTIONS] [FILES]...

Arguments:
  [FILES]...
          Files to process

Options:
  -c, --report
          Instead of printing total unique lines, print a report showing occurrence count of each
          line. This is only compatible with "exact" mode (the default)

  -s, --sort
          Sort report output alphabetically by line. Has no effect unless used with `--report`

  -t, --trim
          Remove leading and trailing whitespace from input

  -l, --lower
          Convert input to lowercase

  -m, --mode <MODE>
          Sets the algorithm used to count (or estimate) cardinality

          [default: exact]

          Possible values:
          - exact:      Uses a hash table to exactly count cardinality. The size of the hash table
            is proportional to the cardinality of the input. You may use the `--size` flag to set
            the initial capacity of the internal hash table. For very large inputs `--size` may help
            reduce expensive hash table reallocations. Avoid setting `--size` for small datasets
          - near-exact: Uses a hash table to exactly count cardinality, but does not store the
            original line. This mode is faster than "exact" mode, but hash collision will result in
            under-counting the cardinality by one. However, hash collisions for a 64-bit hash are
            exceedingly unlikely. The size of the hash table is proportional to the cardinality of
            the input. You may use the `--size` flag to set the initial capacity of the internal
            hash table. For very large inputs `--size` may help reduce expensive hash table
            reallocations. Avoid setting `--size` for small datasets. This mode is not compatible
            with `--report`
          - estimate:   Uses the HyperLogLog algorithm to estimate cardinality with fixed memory.
            Use the `--size` flag to specify the number of 1-byte registers to use. More registers
            will increase estimate accuracy. By default, 65536 is used. This mode is not compatible
            with `--report`

  -n, --size <SIZE>
          Set the size used by the selected counting mode. See the `--mode` documentation for how
          this affects each counting mode

      --threads <THREADS>
          Set the number of threads used to perform the count. By default, the number of logical
          cores is used. Not all counting modes support parallelism: see `--mode` for details

      --no-stdin
          Disable checking stdin for input. May yield a small performance improvement when only
          reading input from files

      --memmap
          Force reading files via memmap. This may yield improved performance for large files. If
          the binary was built without memmap support, using this flag will result in an error

      --no-memmap
          Disable reading files via memmap, instead falling back to normal reads. By default, cuniq
          will try to use memmap if it thinks it will be faster. Disabling memmap may yield improved
          performance for small files

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## License

cuniq is free software: you can redistribute it and/or modify it under the terms of the
[GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

cuniq is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the [GNU General Public License](LICENSE) for more
details.

A full list of dependencies is available in [Cargo.toml](Cargo.toml), or a breakdown of dependencies by license can be
generated with `cargo deny list`.
