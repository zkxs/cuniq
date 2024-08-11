# line_cardinality

A library that provides high performance line cardinality counts and estimates, including:
- Hashing with collision detection
- Hashing **without** collision detection. Note that collisions are nearly impossible for 64-bit hashes, and this has higher performance due to not having to store lines.
- [HyperLogLog](https://en.wikipedia.org/wiki/HyperLogLog

Full API documentation at [docs.rs](https://docs.rs/line_cardinality/latest/line_cardinality/).
See [PERFORMANCE.md](../PERFORMANCE.md) for performance data and technical details on the benchmarking and
profile-guided optimization that went into creating line_cardinality.

## License

line_cardinality was built primarily for use with the [cuniq](../README.md) CLI tool, and is therefore released under the
same GPL-3.0-or-later license.

line_cardinality is free software: you can redistribute it and/or modify it under the terms of the
[GNU General Public License](../LICENSE) as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

line_cardinality is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the [GNU General Public License](../LICENSE) for more
details.

A full list of dependencies is available in [Cargo.toml](Cargo.toml), or a breakdown of dependencies by license can be
generated with `cargo deny list`.
