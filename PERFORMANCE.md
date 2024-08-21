# Performance

## Implementation Details 

cuniq works by storing each unique line in a hash map, meaning it runs in O(*n*) time and uses O(*m*) memory, where *m*
is the number of distinct lines, or in other words the cardinality of the dataset. This means cuniq will significantly
outperform sorting-based approaches when the cardinality is low. Where cardinality is very high (with the worst case
being every line in the dataset being unique) the cost of inserting every item into a hash map starts to outweigh the
benefit of not sorting.

For datasets with very large cardinality exact approaches becomes infeasible as it becomes impossible to process the
dataset in main memory. You will instead need to use a statistical estimate such as HyperLogLog. You can do this with
`cuniq --mode=estimate`. 

## Optimizations

Various tweaks to cuniq were implemented and benchmarked. Tweaks that improved performance were retained:

- [`HashMap::raw_entry_mut`](https://doc.rust-lang.org/std/collections/hash_map/struct.HashMap.html#method.raw_entry_mut)
  is used for deferring cloning keys until a new key is know to be required. This shows significant performance improvements
  over unconditionally cloning every key, but unfortunately requires nightly Rust to compile (pending
  [#56167](https://github.com/rust-lang/rust/issues/56167)).
- [memmap](https://crates.io/crates/memmap2) is used to reduce IO cost of reading large files. This slightly hurts
  performance for small files due to setup overhead, but has scaling performance improvements for larger and larger files.
  [memchr](https://crates.io/crates/memchr) is used for performant newline searching when using memory-mapped IO.
- All non-memory-mapped IO is buffered.
- [bstr](https://crates.io/crates/bstr) is used to skip performing UTF-8 validation on input.
- [ahash](https://crates.io/crates/ahash) is used to reduce cost of hashing, as we do not need the cryptographic
  security of the standard hash.
- [const generics](https://doc.rust-lang.org/reference/items/generics.html#const-generics) were intentionally not used,
  as they were unable to reliably improve performance in benchmarks.
- HashMap with `()` values was found to have equal performance to a HashSet, so HashSet was dropped to slightly simplify
  the implementation.
- Large data structures (e.g. the HashMap) are intentionally leaked to have the OS perform cleanup instead of letting
  Rust call destructors.
- [HashTable](https://docs.rs/hashbrown/0.14.5/hashbrown/struct.HashTable.html) is used to further reduce HashMap
  overhead in the mode (`--mode=near-exact`) where only hashes are stored.
- [HyperLogLog](https://en.wikipedia.org/wiki/HyperLogLog) is used in the statistical estimate mode (`--mode=estimate`).
  HyperLogLog tends to be extremely fast, as the bounded memory it uses is small enough to live *entirely* within CPU
  cache on modern CPUs meaning not only does it not have expensive allocations, but often it doesn't even need to read
  main memory.

# Benchmarking

To run the benchmarks yourself:

1. Be on Windows (sorry, I haven't set the benchmarks up to find GNU coreutils in a cross platform way)
2. Install Git Bash
3. Run the following in Git Bash:

```shell
RUSTFLAGS="-C target-cpu=native" cargo +nightly bench -Z build-std=std --no-default-features --features bench --target=x86_64-pc-windows-msvc
```

You may also want to only bench the library, as the binary benchmarks can be somewhat slow:

```shell
RUSTFLAGS="-C target-cpu=native" cargo +nightly bench -Z build-std=std --no-default-features --features bench --target=x86_64-pc-windows-msvc --package line_cardinality
```

## Tests against a large file

The test file is 4,000,000 lines (~22 MiB) of uniformly distributed random numeric strings. 100,000 of the strings are
unique (2.5% cardinality).

### Counting

This test just gets a count of unique lines. Note that out of these commands only cuniq supports counting, meaning the
rest must be piped into `wc` which has overhead due to all the I/O being performed.

The following plot shows time spent to count the unique lines. `cuniq-hash` and `runiq-hash` use a technique where only
the string's hash is retained, which in theory is vulnerable to hash collisions, but in practice with the 64-bit hashes
they're using it would be extraordinarily rare to see incorrect results.

[![violin plot of count timing](docs/criterion/count_large/report/violin.svg)](docs/criterion/count_large/report/index.html)

### Reporting

This test gets a report of the number of times each distinct line occurred. Of the 6 counting commands tested only 4
have this feature, which is why there are fewer rows in the plot.

[![violin plot of report timing](docs/criterion/report_large/report/violin.svg)](docs/criterion/report_large/report/index.html)

## Tests against a huge file

The test file is 32 GiB dump of slightly preprocessed Wikipedia text. The file has 6,028,206,370 lines (no trailing
newline), of which 78,035,032 are unique (1.3% cardinality). Times were recorded using bash's `time` builtin. The best
times for the "Count", "Report" and "Report (Sorted)" categories are bolded.

| Command                                   | Version  | Real Time     | User Time  | Sys Time  | Operation        | Notes                                                       |
|-------------------------------------------|----------|---------------|------------|-----------|------------------|-------------------------------------------------------------|
| `wc -l huge.txt`                          | GNU 8.32 | 0m23.612s     | 0m8.625s   | 0m4.796s  | N/A              | a decent baseline for how quickly the file can be traversed |
| `sort -u huge.txt \| wc -l`               | GNU 8.32 | 13m29.613s    | 31m58.686s | 0m16.108s | Count            |                                                             |
| `sort huge.txt \| uniq -c > /dev/null`    | GNU 8.32 | 28m13.754s    | 36m58.639s | 0m38.624s | Report (sorted)  |                                                             |
| `cuniq huge.txt`                          | 1.0.0    | 4m09.794s     | 0m0.000s   | 0m0.015s  | Count            |                                                             |
| `cuniq --no-memmap huge.txt`              | 1.0.0    | 3m20.319s     | 0m0.000s   | 0m0.016s  | Count            |                                                             |
| `cuniq < huge.txt`                        | 1.0.0    | 3m19.071s     | 0m0.000s   | 0m0.015s  | Count            |                                                             |
| `cuniq -c huge.txt > /dev/null`           | 1.0.0    | **4m36.895s** | 0m0.000s   | 0m0.030s  | Report           |                                                             |
| `cuniq -cs huge.txt > /dev/null`          | 1.0.0    | **5m05.738s** | 0m0.000s   | 0m0.000s  | Report (sorted)  |                                                             |
| `cuniq --mode=near-exact huge.txt`        | 1.0.0    | **2m3.940s**  | 0m0.000s   | 0m0.000s  | Count            | only stores hash                                            |
| `cuniq --mode=estimate huge.txt`          | 1.0.0    | 1m30.028s     | 0m0.000s   | 0m0.000s  | Count (estimate) | HyperLogLog estimate w/ 0.16% error                         |
| `sortuniq < huge.txt \| wc -l`            | 0.2.0    | 12m27.609s    | 0m3.859s   | 0m15.843s | Count            |                                                             |
| `sortuniq -c < huge.txt > /dev/null`      | 0.2.0    | 12m1.088s     | 0m0.000s   | 0m0.000s  | Report           |                                                             |
| `runiq --filter=simple huge.txt \| wc -l` | 2.0.0    | 11m59.739s    | 0m3.875s   | 0m17.421s | Count            |                                                             |
| `runiq huge.txt \| wc -l`                 | 2.0.0    | 6m9.880s      | 0m46.984s  | 3m39.093s | Count            | only stores hash                                            |
| `huniq < huge.txt \| wc -l`               | 2.7.0    | 4m30.499s     | 0m31.500s  | 2m23.250s | Count            | only stores hash                                            |
| `huniq -c < huge.txt > /dev/null`         | 2.7.0    | 10m21.352s    | 0m0.000s   | 0m0.015s  | Report           |                                                             |
| `huniq -cs < huge.txt > /dev/null`        | 2.7.0    | 10m25.526s    | 0m0.000s   | 0m0.000s  | Report (sorted)  |                                                             |

The commands that are noted as "only stores hash" are in theory vulnerable to hash collisions, but in practice with the
64-bit hashes they're using it would be extraordinarily rare to see incorrect results.

# Profiling on Windows

For some reason on Windows `cargo flamegraph` isn't picking up the debug symbols when used from the project root.
Instead, I have to CD to the build directory.

Run the following in an administrator Powershell:

```powershell
cd F:\git\cuniq
$env:RUSTFLAGS='-C target-cpu=native'; cargo +nightly build -Z build-std=std --profile=release-optimized-debug --target=x86_64-pc-windows-msvc
cd target\x86_64-pc-windows-msvc\release-optimized-debug
flamegraph -- .\cuniq.exe --no-stdin ..\..\..\test_files\huge.txt

# alternatively use blondie instead of dtrace if the above does not work
flamegraph --cmd blondie_dtrace.exe --output flamegraph-blondie.svg -- .\cuniq.exe --no-stdin ..\..\..\test_files\huge.txt
```
