# Changelog

This project follows [semantic versioning](https://semver.org/).

# 2.0.0 - 2024-09-15

## Added

- Iter implementations for `HashingLineCounter`
  - borrowed: `HashingLineCounterIter`. Comes from `HashingLineCounter::iter()` or `(&HashingLineCounter)::into_iter()`
  - owned: `HashingLineCounterIntoIter`. Comes from `HashingLineCounter::into_iter()`
- `HashingLineCounter::get()`, which lets you retrieve a count for a specified line.

## Changed

- Nightly Rust is no longer needed to compile line_cardinality.
- `HashingLineCounter::for_each_report_entry` now calls a `FnMut(&[u8], T)` instead of a `FnMut((&[u8], &T))`
- `Increment` must now be `Copy`

## Removed

- the underling map used is no longer a `std::collections::HashMap`, and converting into one would be quite expensive so
  the following functions were removed. You should switch to using the new `get()` function or iter implementations
  instead.
  - `HashingLineCounter::as_map()`
  - `HashingLineCounter::into_map()`

# 1.0.2 - 2024-08-11

No changes: only documentation improvements.

# 1.0.1 - 2024-08-11

## Fixed

- fix repository link on crates.io

# 1.0.0 - 2024-08-11

## Added

- Initial release.
