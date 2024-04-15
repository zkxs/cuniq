// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use std::f64::consts::E;
#[cfg(not(feature = "ahash"))]
use std::hash::BuildHasher;

use crate::{CountUnique, Error};

use super::{init_hasher_state, RandomState};

type Hash = u64;

const DEFAULT_SIZE: usize = 65536;

pub struct HyperLogLog<M> {
    random_state: RandomState,
    size: usize,
    /// number of bits in the left part == log2(size)
    bits: u32,
    /// number of bits we need to shift the left part to get it alone
    shift_bits: u32,
    /// mask used to isolate the right side
    mask: Hash,
    counters: Vec<u8>,
    string_buffer: Vec<u8>,
    line_mapper: M,
}

fn check_size(size: usize) -> Result<SizeInfo, Error> {
    if !size.is_power_of_two() {
        Err(Error::hyper_log_log(format!("HyperLogLog size must be a power of 2, but was {}", size), size))
    } else if size < 16 {
        Err(Error::hyper_log_log(format!("HyperLogLog size must be at least 16, but was {}", size), size))
    } else {
        let bits = size.ilog2();
        let shift_bits: u32 = Hash::BITS - bits;
        let mask: Hash = 0xFFFFFFFFFFFFFFFF >> bits;
        Ok(SizeInfo {
            bits,
            shift_bits,
            mask,
        })
    }
}

struct SizeInfo {
    bits: u32,
    shift_bits: u32,
    mask: Hash,
}

impl Default for HyperLogLog<()> {
    fn default() -> Self {
        Self::new()
    }
}

/// Constructors that do not take a custom line mapper
impl HyperLogLog<()> {
    /// Creates a new count_unique_impl.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_SIZE).unwrap()
    }

    /// Creates a new count_unique_impl with a cardinality hint of `capacity`.
    ///
    /// Note that it is best to leave `capacity` unset unless you have a near-perfect idea of your
    /// data's cardinality lower bound, as it is extremely difficult to gain performance by setting
    /// it, but extremely easy to lose performance.
    pub fn with_capacity(size: usize) -> Result<Self, Error> {
        let SizeInfo { bits, shift_bits, mask } = check_size(size)?;
        Ok(HyperLogLog {
            random_state: init_hasher_state(),
            size,
            bits,
            shift_bits,
            mask,
            counters: vec![0; size],
            string_buffer: Vec::new(),
            line_mapper: (),
        })
    }
}

/// Constructors that take a custom line mapper
impl<M> HyperLogLog<M>
where
    M: for<'a> FnMut(&'a [u8], &'a mut Vec<u8>) -> &'a [u8],
{
    /// Creates a new count_unique_impl with a custom `line_mapper` function which will be applied to
    /// each read line before counting.
    pub fn with_line_mapper(line_mapper: M) -> Self {
        Self::with_line_mapper_and_capacity(line_mapper, DEFAULT_SIZE).unwrap()
    }

    /// Creates a new count_unique_impl with a cardinality hint of `capacity` and a custom
    /// `line_mapper` function which will be applied to each read line before counting.
    ///
    /// Note that it is best to leave `capacity` unset unless you have a near-perfect idea of your
    /// data's cardinality lower bound, as it is extremely difficult to gain performance by setting
    /// it, but extremely easy to lose performance.
    pub fn with_line_mapper_and_capacity(line_mapper: M, size: usize) -> Result<Self, Error> {
        let SizeInfo { bits, shift_bits, mask } = check_size(size)?;
        Ok(HyperLogLog {
            random_state: init_hasher_state(),
            size,
            bits,
            shift_bits,
            mask,
            counters: vec![0; size],
            string_buffer: Vec::new(),
            line_mapper,
        })
    }
}

impl<M> HyperLogLog<M> {
    /// get the first b bits where b == log2(SIZE) == bits()
    #[inline(always)]
    fn left_bits(&self, hash: Hash) -> usize {
        (hash >> self.shift_bits) as usize
    }

    /// getting the remaining bits, that aren't part of [`Self::left_bits`]
    #[inline(always)]
    fn right_bits(&self, hash: Hash) -> Hash {
        hash & self.mask
    }

    /// Lifted straight from wikipedia
    /// https://en.wikipedia.org/wiki/HyperLogLog#Practical_considerations
    fn magic_bias_constant(&self) -> f64 {
        match self.size {
            16 => 0.673,
            32 => 0.697,
            64 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / (self.size as f64)),
        }
    }

    #[inline(always)]
    fn count(&self) -> usize {
        let sum: f64 = self.counters.iter()
            .map(|value| 2f64.powf(-(*value as f64)))
            .sum();
        let sum = 1.0 / sum;
        let size_float = self.size as f64;
        let count: f64 = self.magic_bias_constant() * size_float * size_float * sum;

        if count < size_float * 5.0 / 2.0 {
            // fall back to linear counting if cardinality estimate is below some threshold
            let zeroed_counters = self.counters.iter()
                .filter(|value| **value == 0)
                .count();
            if zeroed_counters == 0 {
                (count + 0.5) as usize // `as usize` truncates, so by adding 0.5 we achieve round-nearest behavior
            } else {
                let count = size_float * f64::log(size_float / (zeroed_counters as f64), E); // I'll be honest, I don't know why this is log base E
                (count + 0.5) as usize // `as usize` truncates, so by adding 0.5 we achieve round-nearest behavior
            }
        } else {
            (count + 0.5) as usize // `as usize` truncates, so by adding 0.5 we achieve round-nearest behavior
        }
        // TODO big counting for 32 bit registers, see https://en.wikipedia.org/wiki/HyperLogLog#Practical_considerations
        //let count = (-2f64).powf(32f64) * f64::log2(1.0 - (count / 2f64.powf(32f64)));
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.counters.fill(0);
    }
}

impl CountUnique for HyperLogLog<()> {
    fn count_line(&mut self, line: &[u8]) {
        let hash: Hash = self.random_state.hash_one(line);
        let index = self.left_bits(hash);

        // This is actually the position of the leftmost 1, which is why there's a +1 in there.
        // Since we're counting bits in a u64 this is guaranteed to fit in a u8.
        let zero_count = (self.right_bits(hash).leading_zeros() + 1 - self.bits) as u8;

        let counter = &mut self.counters[index];
        *counter = u8::max(*counter, zero_count);
    }

    fn count(&self) -> usize {
        HyperLogLog::count(self)
    }

    fn reset(&mut self) {
        HyperLogLog::reset(self);
    }
}

impl<M> CountUnique for HyperLogLog<M>
where
    M: for<'a> FnMut(&'a [u8], &'a mut Vec<u8>) -> &'a [u8],
{
    fn count_line(&mut self, line: &[u8]) {
        let line = (self.line_mapper)(line, &mut self.string_buffer);

        let hash: Hash = self.random_state.hash_one(line);
        let index = self.left_bits(hash);

        // This is actually the position of the leftmost 1, which is why there's a +1 in there.
        // Since we're counting bits in a u64 this is guaranteed to fit in a u8.
        let zero_count = (self.right_bits(hash).leading_zeros() + 1 - self.bits) as u8;

        let counter = &mut self.counters[index];
        *counter = u8::max(*counter, zero_count);
    }

    fn count(&self) -> usize {
        HyperLogLog::count(self)
    }

    fn reset(&mut self) {
        HyperLogLog::reset(self);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bits() {
        assert_eq!(check_size(16).unwrap().bits, 4);
        assert_eq!(check_size(32).unwrap().bits, 5);
        assert_eq!(check_size(64).unwrap().bits, 6);
        assert_eq!(check_size(128).unwrap().bits, 7);
        assert_eq!(check_size(256).unwrap().bits, 8);
    }

    #[test]
    fn test_mask() {
        assert_eq!(check_size(16).unwrap().mask, 0x0FFFFFFFFFFFFFFF);
        assert_eq!(check_size(32).unwrap().mask, 0x07FFFFFFFFFFFFFF);
        assert_eq!(check_size(64).unwrap().mask, 0x03FFFFFFFFFFFFFF);
        assert_eq!(check_size(128).unwrap().mask, 0x01FFFFFFFFFFFFFF);
        assert_eq!(check_size(256).unwrap().mask, 0x00FFFFFFFFFFFFFF);
    }

    #[test]
    fn test_left_bits() {
        assert_eq!(HyperLogLog::with_capacity(16).unwrap().left_bits(0x5FFFFFFFFFFFFFFF), 0x05);
        assert_eq!(HyperLogLog::with_capacity(256).unwrap().left_bits(0x05FFFFFFFFFFFFFF), 0x05);
    }

    #[test]
    fn test_right_bits() {
        assert_eq!(HyperLogLog::with_capacity(16).unwrap().right_bits(0xF876543210EDCBA9), 0x0876543210EDCBA9);
        assert_eq!(HyperLogLog::with_capacity(256).unwrap().right_bits(0xFF76543210EDCBA9), 0x0076543210EDCBA9);
    }
}
