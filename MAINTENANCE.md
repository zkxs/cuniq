# Linting & Testing

I'm probably missing something obvious, but to test various feature combinations you can do some nightmare wall of text like this:

```shell
cargo clippy --all-features
cargo clippy --no-default-features
cargo clippy --no-default-features --features memmap
cargo clippy --no-default-features --features parallel
cargo clippy --benches --no-default-features --features bench
cargo test --all-features
cargo test --no-default-features
cargo test --no-default-features --features memmap
cargo test --no-default-features --features parallel
#cargo test --benches --no-default-features --features bench # runs benchmarks as test without recording results
cargo clippy --package cuniq --all-features
cargo clippy --package cuniq --no-default-features
cargo clippy --package cuniq --no-default-features --features memmap
cargo clippy --package cuniq --benches --no-default-features --features bench
cargo test --package cuniq --all-features
cargo test --package cuniq --no-default-features
cargo test --package cuniq --no-default-features --features memmap
#cargo test --package cuniq --benches --no-default-features --features bench # runs benchmarks as test without recording results
cargo clippy --package line_cardinality --all-features
cargo clippy --package line_cardinality --no-default-features
cargo clippy --package line_cardinality --benches --features bench
cargo test --package line_cardinality
cargo test --package line_cardinality --benches --features bench # runs benchmarks as test without recording results
```

Also follow the instructions in [PERFORMANCE.md](PERFORMANCE.md) if you're doing anything perf-sensitive to make sure
you haven't blundered.

# Local install

```shell
RUSTFLAGS="-C target-cpu=native" cargo +nightly install -Z build-std=std --path ./cuniq --target=x86_64-pc-windows-msvc
```

# Other Bits and Bobs

Check if I forgot any copyright notices: `rg -g '*.rs' --files-without-match -F 'GNU GPL v3.0'`

MSRV checks:
```shell
cargo msrv --package cuniq --path cuniq verify
cargo msrv --package line_cardinalty --path line_cardinality verify
```

Show assembly

```shell
RUSTFLAGS="-C target-cpu=native" cargo +nightly asm -Z build-std=std --package=cuniq --bin=cuniq --profile=release-optimized-debug --target-cpu=native --intel --simplify
```
