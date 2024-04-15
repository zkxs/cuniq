// This file is part of cuniq. Copyright © 2024 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! Benchmarks for the built binary

use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use criterion::{black_box, criterion_group, criterion_main, Criterion};

// require certain features for this benchmark
#[cfg(not(all(feature = "memmap")))]
compile_error!("missing required features");

// Require that we don't have compile-time-rng, which would cause the binary and the bench to use different RNG seeds.
#[cfg(feature = "compile-time-rng")]
compile_error!("compile-time-rng feature should be disabled for benchmarking");

criterion_group!(benches, bench_cuniq_count_vs_shell, bench_cuniq_report_vs_shell);
criterion_main!(benches);

/// primary test condition for comparing high cardinality
const TEST_FILE_ENGLISH_WORDS: TestFile = TestFile::new("hamlet_words.txt", "hamlet", 5414, 20);

const TEST_FILE_SMALL: TestFile = TestFile::new("small.txt", "small", 3, 100);

const TEST_FILE_LARGE: TestFile = TestFile::new("large.txt", "large", 100000, 10);

/// Various different test conditions for comparing against different programs.
/// Some of these are disabled as they don't provide much insight but they slow down the benchmarks.
static TEST_FILES: &[TestFile] = &[
    // TestFile::new("empty.txt", "c1 empty", 1, 20),
    // TestFile::new("same_line.txt", "c1", 1, 20),
    // TestFile::new("needle1.txt", "needle@start", 2, 20),
    // TestFile::new("needle2.txt", "needle@end", 2, 20),
    // TestFile::new("cardinality_10.txt", "c10", 10, 20),
    // TestFile::new("cardinality_100.txt", "c100", 100, 20),
    TestFile::new("cardinality_1000.txt", "c1e3", 1000, 20),
    TestFile::new("shuffled_numbers.txt", "c1e6", 1000000, 10),
    TEST_FILE_SMALL,
    TEST_FILE_ENGLISH_WORDS,
    TEST_FILE_LARGE,
];

struct TestFile {
    filename: &'static str,
    description: &'static str,
    expected: usize,
    sample_size: usize,
}

impl TestFile {
    const fn new(filename: &'static str, description: &'static str, expected: usize, sample_size: usize) -> Self {
        Self {
            filename,
            description,
            expected,
            sample_size,
        }
    }

    fn relative_path(&self) -> PathBuf {
        let mut path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.push("test_files");
        path.push(self.filename);
        path
    }
}

/// Benches cuniq counts vs other shell commands.
/// TODO fix hardcoded paths
fn bench_cuniq_count_vs_shell(c: &mut Criterion) {
    // get cuniq exe path
    let cuniq_path = env!("CARGO_BIN_EXE_cuniq");
    println!("running benchmarks against \"{cuniq_path}\"");

    for test_file in TEST_FILES {
        let path_buf = test_file.relative_path();
        let mut group = c.benchmark_group(format!("count/{}", test_file.description));
        group.sample_size(test_file.sample_size);
        let expected = format!("{}\n", test_file.expected);

        // sort input.txt | uniq | wc -l
        group.bench_function("uniq", |bencher| {
            bencher.iter(|| {
                let sort = Command::new(r"C:\Program Files\Git\usr\bin\sort.exe")
                    .arg(path_buf.as_os_str())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let uniq = Command::new(r"C:\Program Files\Git\usr\bin\uniq.exe")
                    .stdin(Stdio::from(sort.stdout.unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let wc = Command::new(r"C:\Program Files\Git\usr\bin\wc.exe")
                    .arg("-l")
                    .stdin(Stdio::from(uniq.stdout.unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = wc.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                assert_eq!(result, &expected);
            });
        });

        // sort -u input.txt | wc -l
        group.bench_function("sort", |bencher| {
            bencher.iter(|| {
                let sort = Command::new(r"C:\Program Files\Git\usr\bin\sort.exe")
                    .arg("-u")
                    .arg(path_buf.as_os_str())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let wc = Command::new(r"C:\Program Files\Git\usr\bin\wc.exe")
                    .arg("-l")
                    .stdin(Stdio::from(sort.stdout.unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = wc.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                assert_eq!(result, &expected);
            });
        });

        // cuniq input.txt
        group.bench_function("cuniq", |bencher| {
            bencher.iter(|| {
                let cuniq = Command::new(cuniq_path)
                    .arg("--no-stdin")
                    .arg("--memmap")
                    .arg(path_buf.as_os_str())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = cuniq.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                assert_eq!(result, &expected);
            });
        });

        // cuniq --mode=near-exact input.txt
        group.bench_function("cuniq-hash", |bencher| {
            bencher.iter(|| {
                let cuniq = Command::new(cuniq_path)
                    .arg("--no-stdin")
                    .arg("--memmap")
                    .arg("--mode=near-exact")
                    .arg(path_buf.as_os_str())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = cuniq.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                assert_eq!(result, &expected);
            });
        });

        // sortuniq < input.txt
        // uses normal file-based stdin
        // note that sortuniq only supports stdin
        group.bench_function("sortuniq", |bencher| {
            bencher.iter(|| {
                let sortuniq = Command::new("sortuniq.exe")
                    .stdin(Stdio::from(File::open(&path_buf).unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let wc = Command::new(r"C:\Program Files\Git\usr\bin\wc.exe")
                    .arg("-l")
                    .stdin(Stdio::from(sortuniq.stdout.unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = wc.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                assert_eq!(result, &expected);
            });
        });

        // runiq --filter=simple input.txt
        group.bench_function("runiq", |bencher| {
            bencher.iter(|| {
                let runiq = Command::new("runiq.exe")
                    .arg("--filter=simple")
                    .arg(path_buf.as_os_str())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let wc = Command::new(r"C:\Program Files\Git\usr\bin\wc.exe")
                    .arg("-l")
                    .stdin(Stdio::from(runiq.stdout.unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = wc.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                assert_eq!(result, &expected);
            });
        });

        // runiq --filter=simple input.txt
        // note that runiq's default filter "quick" is theoretically vulnerable to hash collisions.
        group.bench_function("runiq-hash", |bencher| {
            bencher.iter(|| {
                let runiq = Command::new("runiq.exe")
                    .arg("--filter=quick")
                    .arg(path_buf.as_os_str())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let wc = Command::new(r"C:\Program Files\Git\usr\bin\wc.exe")
                    .arg("-l")
                    .stdin(Stdio::from(runiq.stdout.unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = wc.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                assert_eq!(result, &expected);
            });
        });

        // huniq < input.txt
        // uses normal file-based stdin
        // note that huniq only supports stdin
        // note that this is an unfair benchmark, as huniq only stores the hash
        group.bench_function("huniq", |bencher| {
            bencher.iter(|| {
                let huniq = Command::new("huniq.exe")
                    .stdin(Stdio::from(File::open(&path_buf).unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let wc = Command::new(r"C:\Program Files\Git\usr\bin\wc.exe")
                    .arg("-l")
                    .stdin(Stdio::from(huniq.stdout.unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = wc.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                assert_eq!(result, &expected);
            });
        });

        group.finish();
    }
}

/// Benches cuniq reports vs other shell commands.
/// TODO fix hardcoded paths
fn bench_cuniq_report_vs_shell(c: &mut Criterion) {
    // get cuniq exe path
    let cuniq_path = env!("CARGO_BIN_EXE_cuniq");
    println!("running benchmarks against \"{cuniq_path}\"");

    for test_file in TEST_FILES {
        let path_buf = test_file.relative_path();
        let mut group = c.benchmark_group(format!("report/{}", test_file.description));
        group.sample_size(test_file.sample_size);

        // sort input.txt | uniq -c
        group.bench_function("uniq", |bencher| {
            bencher.iter(|| {
                let sort = Command::new(r"C:\Program Files\Git\usr\bin\sort.exe")
                    .arg(path_buf.as_os_str())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let uniq = Command::new(r"C:\Program Files\Git\usr\bin\uniq.exe")
                    .arg("-c")
                    .stdin(Stdio::from(sort.stdout.unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = uniq.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                black_box(result);
            });
        });

        // cuniq -c input.txt
        group.bench_function("cuniq", |bencher| {
            bencher.iter(|| {
                let cuniq = Command::new(cuniq_path)
                    .arg("--no-stdin")
                    .arg("--memmap")
                    .arg("--report")
                    .arg(path_buf.as_os_str())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = cuniq.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                black_box(result);
            });
        });

        // sortuniq -c < input.txt
        // uses normal file-based stdin
        // note that sortuniq only supports stdin
        group.bench_function("sortuniq", |bencher| {
            bencher.iter(|| {
                let sortuniq = Command::new("sortuniq.exe")
                    .arg("-c")
                    .stdin(Stdio::from(File::open(&path_buf).unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = sortuniq.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                black_box(result);
            });
        });

        // huniq -c < input.txt
        // uses normal file-based stdin
        // note that huniq only supports stdin
        group.bench_function("huniq", |bencher| {
            bencher.iter(|| {
                let huniq = Command::new("huniq.exe")
                    .arg("-c")
                    .stdin(Stdio::from(File::open(&path_buf).unwrap()))
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                let output = huniq.wait_with_output().unwrap();
                let result = std::str::from_utf8(&output.stdout).unwrap();
                black_box(result);
            });
        });

        group.finish();
    }
}
