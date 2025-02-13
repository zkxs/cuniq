// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

/// A type that can count occurrences of a line
pub trait Increment: Copy {
    /// Increment the current count
    fn increment(&mut self);

    /// Create a new counter with the default starting value for a single entry found
    fn new() -> Self;

    /// Return the current count
    fn count(&self) -> &Self {
        self
    }
}

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
