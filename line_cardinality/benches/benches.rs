// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! Benchmarks for various functions

#![feature(hash_raw_entry)]
#![feature(hash_set_entry)]

use std::fs::File;
use std::path::PathBuf;

use ahash::RandomState;
use bstr::ByteSlice;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use line_cardinality::{CountUnique, CountUniqueFromMemmapFile, CountUniqueFromReadFile, LineCounter};

// require certain features for this benchmark
#[cfg(not(all(feature = "ahash", feature = "memmap", feature = "memchr", feature = "file")))]
compile_error!("missing required features");

criterion_group!(benches, bench_small, bench_large, bench_tweaks);
criterion_main!(benches);

mod no_fn;
mod stable_map;
mod stable_set;
mod siphash;
mod string;
mod unstable_set;

/// primary test condition for comparing high cardinality
const TEST_FILE_ENGLISH_WORDS: TestFile = TestFile::new("hamlet_words.txt", 5414);

/// one-off count of the lowercase distinct words for a certain benchmark
const ENGLISH_WORDS_LOWERCASE_COUNT: usize = 4900;

const TEST_FILE_SMALL: TestFile = TestFile::new("small.txt", 3);

const TEST_FILE_LARGE: TestFile = TestFile::new("large.txt", 100000);

const FILE_HANDLE_BATCH_SIZE: BatchSize = BatchSize::SmallInput;

struct TestFile {
    filename: &'static str,
    expected: usize,
}

impl TestFile {
    const fn new(filename: &'static str, expected: usize) -> Self {
        Self {
            filename,
            expected,
        }
    }

    fn relative_path(&self) -> PathBuf {
        let mut path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.push("test_files");
        path.push(self.filename);
        path
    }

    fn open(&self) -> Vec<File> {
        vec![File::open(self.relative_path()).unwrap()]
    }
}

/// hasher with pre-generated random seed
pub fn init_hasher_state() -> RandomState {
    RandomState::with_seeds(
        0xD4D1C62E748C6F9F,
        0x6AB3CDB8BD6660B5,
        0x252E7AFD38FC5B30,
        0xD47C5724DAD72AD1,
    )
}

/// Test 1-off implementation tweaks from the stock lib implementation that may be overly affect
/// for small filesizes
fn bench_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("tweaks.small");

    group.bench_function("read", |bencher| {
        bencher.iter_batched(|| TEST_FILE_SMALL.open(), |files| {
            let mut processor = LineCounter::default();
            processor.count_unique_in_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_SMALL.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    group.bench_function("memmap", |bencher| {
        bencher.iter_batched(|| TEST_FILE_SMALL.open(), |files| {
            let mut processor = LineCounter::default();
            processor.count_unique_in_memmap_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_SMALL.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });
}

/// Test 1-off implementation tweaks from the stock lib implementation that may be overly affect
/// for small filesizes
fn bench_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("tweaks.large");

    group.bench_function("read", |bencher| {
        bencher.iter_batched(|| TEST_FILE_LARGE.open(), |files| {
            let mut processor = LineCounter::default();
            processor.count_unique_in_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_LARGE.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    group.bench_function("memmap", |bencher| {
        bencher.iter_batched(|| TEST_FILE_LARGE.open(), |files| {
            let mut processor = LineCounter::default();
            processor.count_unique_in_memmap_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_LARGE.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });
}

/// Test 1-off implementation tweaks from the stock lib implementation
fn bench_tweaks(c: &mut Criterion) {
    let mut group = c.benchmark_group("tweaks");

    // uses a map with () values
    group.bench_function("baseline", |bencher| {
        bencher.iter_batched(|| TEST_FILE_ENGLISH_WORDS.open(), |files| {
            let mut processor = LineCounter::default();
            processor.count_unique_in_memmap_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_ENGLISH_WORDS.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    // same as baseline, but there's no FnMut floating around
    group.bench_function("no-fn", |bencher| {
        bencher.iter_batched(|| TEST_FILE_ENGLISH_WORDS.open(), |files| {
            let mut processor = no_fn::Processor::default();
            processor.count_unique_in_memmap_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_ENGLISH_WORDS.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    // use BufRead instead of Mmap
    group.bench_function("read", |bencher| {
        bencher.iter_batched(|| TEST_FILE_ENGLISH_WORDS.open(), |files| {
            let mut processor = LineCounter::default();
            processor.count_unique_in_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_ENGLISH_WORDS.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    // set impl, but doesn't use unstable set APIs
    group.bench_function("stable_set", |bencher| {
        bencher.iter_batched(|| TEST_FILE_ENGLISH_WORDS.open(), |files| {
            let mut processor = stable_set::Processor::default();
            processor.count_unique_in_memmap_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_ENGLISH_WORDS.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    // set impl, but does use unstable set APIs
    group.bench_function("unstable_set", |bencher| {
        bencher.iter_batched(|| TEST_FILE_ENGLISH_WORDS.open(), |files| {
            let mut processor = unstable_set::Processor::default();
            processor.count_unique_in_memmap_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_ENGLISH_WORDS.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    // map<()> impl, but doesn't use unstable set APIs
    group.bench_function("stable_map", |bencher| {
        bencher.iter_batched(|| TEST_FILE_ENGLISH_WORDS.open(), |files| {
            let mut processor = stable_map::Processor::default();
            processor.count_unique_in_memmap_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_ENGLISH_WORDS.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    // uses str instead of bstr
    group.bench_function("str", |bencher| {
        bencher.iter_batched(|| TEST_FILE_ENGLISH_WORDS.open(), |files| {
            let mut processor = string::Processor::default();
            processor.count_unique_in_memmap_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_ENGLISH_WORDS.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    // uses built in hasher
    group.bench_function("siphash", |bencher| {
        bencher.iter_batched(|| TEST_FILE_ENGLISH_WORDS.open(), |files| {
            let mut processor = siphash::Processor::default();
            processor.count_unique_in_memmap_files(&files).unwrap();
            assert_eq!(processor.count(), TEST_FILE_ENGLISH_WORDS.expected);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    // test lowercase performance
    group.bench_function("baseline.lower", |bencher| {
        bencher.iter_batched(|| TEST_FILE_ENGLISH_WORDS.open(), |files| {
            let mut processor = LineCounter::with_line_mapper(|line, buffer| {
                buffer.clear();
                line.to_lowercase_into(buffer);
                buffer
            });
            processor.count_unique_in_memmap_files(&files).unwrap();
            assert_eq!(processor.count(), ENGLISH_WORDS_LOWERCASE_COUNT);
        }, FILE_HANDLE_BATCH_SIZE);
    });

    group.finish();
}
