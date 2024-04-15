# Linting & Testing

I'm probably missing something obvious, but to test various feature combinations you can do some nightmare wall of text like this:

```shell
cargo +nightly clippy --all-features
cargo +nightly clippy --no-default-features
cargo +nightly clippy --no-default-features --features file
cargo +nightly clippy --no-default-features --features memchr
cargo +nightly clippy --no-default-features --features ahash
cargo +nightly clippy --benches --no-default-features --features bench
cargo +nightly test --all-features
cargo +nightly test --no-default-features
cargo +nightly test --no-default-features --features file
cargo +nightly test --no-default-features --features memchr
cargo +nightly test --no-default-features --features ahash
#cargo +nightly test --benches --no-default-features --features bench # runs benchmarks as test without recording results
cargo +nightly clippy --package cuniq --all-features
cargo +nightly clippy --package cuniq --no-default-features
cargo +nightly clippy --package cuniq --no-default-features --features memmap
cargo +nightly clippy --package cuniq --benches --no-default-features --features bench
cargo +nightly test --package cuniq --all-features
cargo +nightly test --package cuniq --no-default-features
cargo +nightly test --package cuniq --no-default-features --features memmap
#cargo +nightly test --package cuniq --benches --no-default-features --features bench # runs benchmarks as test without recording results
cargo +nightly clippy --package line_cardinality --all-features
cargo +nightly clippy --package line_cardinality --no-default-features
cargo +nightly clippy --package line_cardinality --no-default-features --features file
cargo +nightly clippy --package line_cardinality --no-default-features --features memchr
cargo +nightly clippy --package line_cardinality --no-default-features --features ahash
cargo +nightly clippy --package line_cardinality --benches --no-default-features --features bench
cargo +nightly test --package line_cardinality --all-features
cargo +nightly test --package line_cardinality --no-default-features
cargo +nightly test --package line_cardinality --no-default-features --features file
cargo +nightly test --package line_cardinality --no-default-features --features memchr
cargo +nightly test --package line_cardinality --no-default-features --features ahash
#cargo +nightly test --package line_cardinality --benches --no-default-features --features bench # runs benchmarks as test without recording results
```

Also follow the instructions in [PERFORMANCE.md](PERFORMANCE.md) if you're doing anything perf-sensitive to make sure
you haven't blundered.

# Local install

```shell
RUSTFLAGS="-C target-cpu=native" cargo +nightly install -Z build-std=std --path ./cuniq --target=x86_64-pc-windows-msvc
```

# Other Bits and Bobs

Check if I forgot any copyright notices: `rg -g '*.rs' --files-without-match -F 'GNU GPL v3.0'`
