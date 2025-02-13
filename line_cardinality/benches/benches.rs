// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! Benchmarks for various functions

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use ahash::RandomState;
use bstr::io::BufReadExt;
use bstr::ByteSlice;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

use line_cardinality::{
    CountUnique, CountUniqueHash, CountUniqueLineHash, LosslessHashingLineCounter,
    LossyHashingLineCounter,
};

criterion_group!(benches, bench_tweaks);
criterion_main!(benches);

/// primary test condition for comparing high cardinality
const TEST_FILE_ENGLISH_WORDS: TestFile = TestFile::new("hamlet_words.txt", 5414);

/// one-off count of the lowercase distinct words for a certain benchmark
const ENGLISH_WORDS_LOWERCASE_COUNT: usize = 4900;

const FILE_HANDLE_BATCH_SIZE: BatchSize = BatchSize::SmallInput;

struct TestFile {
    filename: &'static str,
    expected: usize,
}

impl TestFile {
    const fn new(filename: &'static str, expected: usize) -> Self {
        Self { filename, expected }
    }

    fn relative_path(&self) -> PathBuf {
        let mut path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.push("test_files");
        path.push(self.filename);
        path
    }

    fn open(&self) -> File {
        File::open(self.relative_path()).unwrap()
    }
}

/// hasher with pre-generated random seed
fn init_hasher_state() -> RandomState {
    RandomState::with_seeds(
        0xD4D1C62E748C6F9F,
        0x6AB3CDB8BD6660B5,
        0x252E7AFD38FC5B30,
        0xD47C5724DAD72AD1,
    )
}

/// Test 1-off implementation tweaks from the stock lib implementation
fn bench_tweaks(c: &mut Criterion) {
    let mut group = c.benchmark_group("tweaks");

    // baseline "normal" implementation
    group.bench_function("baseline", |bencher| {
        bencher.iter_batched(
            || TEST_FILE_ENGLISH_WORDS.open(),
            |file| {
                let mut reader = BufReader::new(file);
                let hasher = init_hasher_state();
                let mut processor = LosslessHashingLineCounter::<()>::default();
                reader
                    .for_byte_line(|line| {
                        processor
                            .count_line(line, hasher.hash_one(line), |line| hasher.hash_one(line));
                        Ok(true)
                    })
                    .unwrap();
                assert_eq!(processor.count(), TEST_FILE_ENGLISH_WORDS.expected);
            },
            FILE_HANDLE_BATCH_SIZE,
        );
    });

    // test lowercase performance on baseline "normal" implementation
    group.bench_function("baseline.lower", |bencher| {
        bencher.iter_batched(
            || TEST_FILE_ENGLISH_WORDS.open(),
            |file| {
                let mut reader = BufReader::new(file);
                let hasher = init_hasher_state();
                let mut buffer = Vec::new();
                let mut processor = LosslessHashingLineCounter::<()>::default();
                reader
                    .for_byte_line(|line| {
                        buffer.clear();
                        line.to_lowercase_into(&mut buffer);
                        processor.count_line(&buffer, hasher.hash_one(&buffer), |line| {
                            hasher.hash_one(line)
                        });
                        Ok(true)
                    })
                    .unwrap();
                assert_eq!(processor.count(), ENGLISH_WORDS_LOWERCASE_COUNT);
            },
            FILE_HANDLE_BATCH_SIZE,
        );
    });

    // lossy implementation
    group.bench_function("lossy", |bencher| {
        bencher.iter_batched(
            || TEST_FILE_ENGLISH_WORDS.open(),
            |file| {
                let mut reader = BufReader::new(file);
                let hasher = init_hasher_state();
                let mut processor = LossyHashingLineCounter::default();
                reader
                    .for_byte_line(|line| {
                        processor.count_hash(hasher.hash_one(line));
                        Ok(true)
                    })
                    .unwrap();
                assert_eq!(processor.count(), TEST_FILE_ENGLISH_WORDS.expected);
            },
            FILE_HANDLE_BATCH_SIZE,
        );
    });

    // test lowercase performance on lossy implementation
    group.bench_function("lossy.lower", |bencher| {
        bencher.iter_batched(
            || TEST_FILE_ENGLISH_WORDS.open(),
            |file| {
                let mut reader = BufReader::new(file);
                let hasher = init_hasher_state();
                let mut buffer = Vec::new();
                let mut processor = LossyHashingLineCounter::default();
                reader
                    .for_byte_line(|line| {
                        buffer.clear();
                        line.to_lowercase_into(&mut buffer);
                        processor.count_hash(hasher.hash_one(&buffer));
                        Ok(true)
                    })
                    .unwrap();
                assert_eq!(processor.count(), ENGLISH_WORDS_LOWERCASE_COUNT);
            },
            FILE_HANDLE_BATCH_SIZE,
        );
    });

    group.finish();
}
