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

- [HashTable](https://docs.rs/hashbrown/0.14.5/hashbrown/struct.HashTable.html)
  is used for deferring cloning keys until a new key is known to be required. This shows significant performance improvements
  over unconditionally cloning every key. HashTable is also used to further reduce HashMap overhead in the mode
  (`--mode=near-exact`) where only hashes are stored.
- [memmap](https://crates.io/crates/memmap2) is used to reduce IO cost of reading large files. This slightly hurts
  performance for small files due to setup overhead, but has scaling performance improvements for larger and larger files.
  [memchr](https://crates.io/crates/memchr) is used for performant newline searching when using memory-mapped IO.
- All non-memory-mapped IO is buffered.
- [bstr](https://crates.io/crates/bstr) is used to skip performing UTF-8 validation on input.
- [ahash](https://crates.io/crates/ahash) is used to reduce cost of hashing, as we do not need the cryptographic
  security of the standard hash.
- Large data structures (e.g. the HashTable) are intentionally leaked to have the OS perform cleanup instead of letting
  Rust call destructors.
- [HyperLogLog](https://en.wikipedia.org/wiki/HyperLogLog) is used in the statistical estimate mode (`--mode=estimate`).
  HyperLogLog tends to be extremely fast, as the bounded memory it uses is small enough to live *entirely* within CPU
  cache on modern CPUs meaning not only does it not have expensive allocations, but often it doesn't even need to read
  main memory.
- Multithreading was implemented for HyperLogLog, which further improves performance on systems where IO is not the bottleneck.

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

Tests were ran on an AMD Ryzen 7 7800X3D (16 threads) with 5200 MT/s memory and 6950 MB/s sequential read from disk. Host OS is Windows 11 24H2.

| Command                                                                              | Version   |           Time | Peak Memory | Operation        | Implementation Notes                 | Threads | Performance Notes                                                                                           |
| ------------------------------------------------------------------------------------ | --------- | -------------: | ----------: | ---------------- | ------------------------------------ | ------: | ----------------------------------------------------------------------------------------------------------- |
| `wc -l huge.txt`                                                                     | GNU 8.32  |     0m 18.991s |             | N/A              | N/A                                  |       1 | Decent baseline for how quickly the file can be traversed.                                                  |
| `sort -u huge.txt \| wc -l`                                                          | GNU 8.32  |    13m 26.379s |             | Count            | sorting                              |       8 | Loses to dedicated tooling due to sorting the entire input.                                                 |
| `sort huge.txt \| uniq -c > /dev/null`                                               | GNU 8.32  |    27m 10.739s |             | Report (sorted)  | sorting                              |       1 | Very bad. Better to use `awk` for reports if you are constrained to GNU coreutils.                          |
| `awk '{ a[$0]++ }; END { for (x in a) { print x ": " a[x] } }' huge.txt > /dev/null` | GNU 5.0.0 |    16m 43.238s |             | Report           | hashtable                            |       1 | Fastest report option in GNU coreutils, but unsurprisingly loses to dedicated tooling.                      |
| `cuniq --memmap huge.txt`                                                            | 1.1.0     |     3m 16.722s |             | Count            | hashtable                            |       1 | Not sorting is cheaper than sorting, so this beats GNU sort easily.                                         |
| `cuniq --no-memmap huge.txt`                                                         | 1.1.0     |     4m 04.366s |             | Count            | hashtable                            |       1 | `--no-memmap` is slower here, but it tends to be faster for small files.                                    |
| `cuniq < huge.txt`                                                                   | 1.1.0     |     3m 49.026s |             | Count            | hashtable                            |       1 | stdin cannot not use memmap, so this is expected to be close to the above benchmark.                        |
| `cuniq --memmap -c huge.txt > /dev/null`                                             | 1.1.0     | **3m 33.067s** |             | Report           | hashtable                            |       1 | Not sorting is cheaper than sorting, so this beats GNU sort easily.                                         |
| `cuniq --memmap -cs huge.txt > /dev/null`                                            | 1.1.0     | **3m 56.435s** |             | Report (sorted)  | hashtable                            |       1 | Sorting after dedupe is cheaper than sorting before dedupe, so this beats GNU sort easily.                  |
| `cuniq --memmap --mode=near-exact --threads=1 huge.txt`                              | 1.1.0     |     2m 03.316s |      1.2 GB | Count            | hashtable, but only stores hash      |       1 | Speedup is from not having to copy any strings.                                                             |
| `cuniq --memmap --mode=near-exact huge.txt`                                          | 1.1.0     | **0m 53.213s** |      4.5 GB | Count            | hashtable, but only stores hash      |      16 | Speedup is from hashing in background theads and thereby moving the bottleneck to hashtable inserts.        |
| `cuniq --memmap --mode=estimate --threads=1 huge.txt`                                | 1.1.0     |     1m 26.148s |      0.5 MB | Count (estimate) | HyperLogLog estimate w/ 0.61% error  |       1 | Speedup is from constant-sized 64 KiB memory usage.                                                         |
| `cuniq --memmap --mode=estimate huge.txt`                                            | 1.1.0     |     0m 08.964s |      1.8 MB | Count (estimate) | HyperLogLog estimate w/ 0.61% error  |      16 | Speedup is from maximizing CPU use while not being IO-bound due to very fast disk.                          |
| `sortuniq < huge.txt \| wc -l`                                                       | 0.2.0     |    14m 28.887s |             | Count            | hashtable                            |       1 | Struggles to outperform even `sort -u` due to several missed optimizations.                                 |
| `sortuniq -c < huge.txt > /dev/null`                                                 | 0.2.0     |    13m 09.038s |             | Report           | hashtable                            |       1 | Outperforms `awk`, but fails to outperform all other dedicated tooling due to several missed optimizations. |
| `runiq --filter=simple huge.txt \| wc -l`                                            | 2.0.0     |    12m 30.652s |             | Count            | hashtable                            |       1 | Struggles to outperform even `sort -u` due to several missed optimizations.                                 |
| `runiq --filter=quick huge.txt \| wc -l`                                             | 2.0.0     |     6m 39.118s |             | Count            | hashtable, but only stores hash      |       1 | Outperforms `sort -u`, but leaves performance on the table from missed optimizations.                       |
| `runiq --filter=compact huge.txt \| wc -l`                                           | 2.0.0     |    13m 01.483s |      701 MB | Count (estimate) | Bloom filter estimate w/ 0.00% error |       1 | Below average time performance but above average memory performance with a remarkably low error rate.       |
| `huniq < huge.txt \| wc -l`                                                          | 2.7.0     |     5m 09.133s |             | Count            | hashtable, but only stores hash      |       1 | Fairly competitive performance due to minimal missed optimizations.                                         |
| `huniq -c < huge.txt > /dev/null`                                                    | 2.7.0     |    10m 37.392s |             | Report           | hashtable                            |       1 | Outperforms `awk`, but leaves performance on the table from missed optimizations.                           |
| `huniq -cs < huge.txt > /dev/null`                                                   | 2.7.0     |    10m 49.869s |             | Report (sorted)  | hashtable                            |       1 | Outperforms `awk`, but leaves performance on the table from missed optimizations.                           |

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
