// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use crate::Increment;

impl Increment for usize {
    fn increment(&mut self) {
        *self += 1;
    }

    fn new() -> Self {
        1
    }
}

/// Maybe don't use this unless you know your inputs are very small...
impl Increment for u8 {
    fn increment(&mut self) {
        *self += 1;
    }

    fn new() -> Self {
        1
    }
}

/// Maybe don't use this unless you know your inputs are very small...
impl Increment for u16 {
    fn increment(&mut self) {
        *self += 1;
    }

    fn new() -> Self {
        1
    }
}

impl Increment for u32 {
    fn increment(&mut self) {
        *self += 1;
    }

    fn new() -> Self {
        1
    }
}

impl Increment for u64 {
    fn increment(&mut self) {
        *self += 1;
    }

    fn new() -> Self {
        1
    }
}

impl Increment for u128 {
    fn increment(&mut self) {
        *self += 1;
    }

    fn new() -> Self {
        1
    }
}
