// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! Internal utilities used for IO. These are useful, but often unsafe, so I'm not interested in
//! exporting these for public use outside of this crate.

#[cfg(feature = "memchr")]
pub(crate) use memchr_features::*;

/// Raw, pointer-based representation of a slice. This completely throws lifetimes out the window
/// and is terribly unsafe, but is necessary as Rust is unable to reason about lifetimes in many
/// multithreaded interactions. For example, I can ensure that a memory-mapped slice will outlive a
/// background thread, but rustc will be unable to prove this making safe Rust impossible without
/// copying large amounts of data.
#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) struct RawSlice {
    start_ptr: *const u8,
    len: usize,
}

unsafe impl Send for RawSlice {}

impl RawSlice {
    #[inline(always)]
    #[allow(dead_code)] // may use this later, don't want to delete
    pub(crate) unsafe fn from_ptr_and_len(start_ptr: *const u8, len: usize) -> Self {
        Self { start_ptr, len }
    }

    #[inline(always)]
    pub(crate) unsafe fn from_ptr_range(start_ptr: *const u8, end_ptr: *const u8) -> Self {
        // should be used in the form `end.offset_from(start)`
        let len = end_ptr.offset_from(start_ptr);
        // `isize as usize` is a no-op, so we'll get garbage data if the isize was negative
        let len = len as usize;
        Self { start_ptr, len }
    }

    #[inline(always)]
    pub(crate) fn as_slice<'a>(self) -> &'a [u8] {
        unsafe { std::slice::from_raw_parts(self.start_ptr, self.len) }
    }
}

#[cfg(feature = "memchr")]
mod memchr_features {
    use super::*;

    /// Extracts approximately `chunk_size`-sized newline-delimited chunks from a `RawSlice`.
    pub(crate) struct ChunkIterator {
        /// desired chunk size
        chunk_size: usize,
        /// exclusive end ptr for entire range
        end_ptr: *const u8,
        /// inclusive start ptr for next chunk
        chunk_start_ptr: *const u8,
        /// desired exclusive end ptr for next chunk
        chunk_end_ptr: *const u8,
    }

    unsafe impl Send for ChunkIterator {}

    impl ChunkIterator {
        #[cfg(feature = "memmap")]
        pub(crate) fn from_memmap(mem_map: &memmap2::MmapRaw, chunk_size: usize) -> Self {
            Self::new(mem_map.as_ptr(), mem_map.len(), chunk_size)
        }

        #[allow(dead_code)] // may use this later, don't want to delete
        pub(crate) fn from_raw_slice(raw_slice: RawSlice, chunk_size: usize) -> Self {
            let RawSlice { start_ptr, len } = raw_slice;
            Self::new(start_ptr, len, chunk_size)
        }

        pub(crate) fn new(start_ptr: *const u8, len: usize, chunk_size: usize) -> Self {
            let chunk_start_ptr = start_ptr;
            let end_ptr = unsafe { start_ptr.add(len) };
            let chunk_end_ptr = unsafe { chunk_start_ptr.add(chunk_size) };
            Self {
                chunk_size,
                end_ptr,
                chunk_start_ptr,
                chunk_end_ptr,
            }
        }
    }

    impl Iterator for ChunkIterator {
        type Item = RawSlice;

        fn next(&mut self) -> Option<Self::Item> {
            if self.chunk_start_ptr >= self.end_ptr {
                // edge case: we have ran out of data and are ready to stop iterating
                None
            } else if self.chunk_end_ptr >= self.end_ptr {
                // edge case: end ptr has passed end of mem_map
                // just use real end and skip the newline search shit
                // equivalent to  `chunk = &mem_map[chunk_start_index_inclusive..]`
                let chunk = unsafe { RawSlice::from_ptr_range(self.chunk_start_ptr, self.end_ptr) };

                // update start ptr so that the next iteration returns None
                self.chunk_start_ptr = self.end_ptr;

                Some(chunk)
            } else {
                // equivalent to `search_range = &mem_map[chunk_end_index_exclusive..]`
                let search_range = unsafe {
                    // should be used in the form `end.offset_from(start)`
                    let search_range_len = self.end_ptr.offset_from(self.chunk_end_ptr);
                    // `isize as usize` is a no-op, so we'll get garbage data if the isize was negative
                    let search_range_len = search_range_len as usize;

                    std::slice::from_raw_parts(self.chunk_end_ptr, search_range_len)
                };
                if let Some(newline_index) = memchr::memchr(b'\n', search_range) {
                    // equivalent to `chunk = &mem_map[chunk_start_index_inclusive..newline_index]`

                    // convert the search result into a direct pointer to the newline byte
                    let newline_ptr = unsafe { self.chunk_end_ptr.add(newline_index) };

                    let chunk =
                        unsafe { RawSlice::from_ptr_range(self.chunk_start_ptr, newline_ptr) };
                    // update start of next chunk to be directly after this newline
                    self.chunk_start_ptr = unsafe { newline_ptr.add(1) };

                    // update next of next chunk to be 1 chunk worth of size after the start
                    self.chunk_end_ptr = unsafe { self.chunk_start_ptr.add(self.chunk_size) };

                    Some(chunk)
                } else {
                    // edge case: we couldn't find a newline so this 1-word chunk will be the last thread
                    // equivalent to  `chunk = &mem_map[chunk_start_index_inclusive..]`
                    let chunk = unsafe {
                        RawSlice::from_ptr_range(self.chunk_start_ptr, self.chunk_end_ptr)
                    };

                    // update start ptr so that the next iteration returns None
                    self.chunk_start_ptr = self.end_ptr;

                    Some(chunk)
                }
            }
        }
    }

    /// Iterator over lines in some slice
    pub(crate) struct LineIterator<'a> {
        bytes: &'a [u8],
        /// Position to start next memchr search at
        start: usize,
        /// Memchr iterator
        memchr: memchr::Memchr<'a>,
    }

    impl<'a> LineIterator<'a> {
        pub(crate) fn new(bytes: &'a [u8]) -> Self {
            let start = 0;
            let memchr = memchr::memchr_iter(b'\n', bytes);
            Self {
                bytes,
                start,
                memchr,
            }
        }
    }

    impl<'a> Iterator for LineIterator<'a> {
        type Item = &'a [u8];

        fn next(&mut self) -> Option<Self::Item> {
            if let Some(newline_index) = self.memchr.next() {
                // handle normal case
                let slice = &self.bytes[self.start..newline_index];
                self.start = newline_index + 1;
                Some(slice)
            } else if self.start < self.bytes.len() {
                // handle trailing
                let slice = &self.bytes[self.start..];
                self.start = self.bytes.len(); // set up for end
                Some(slice)
            } else {
                // handle end
                None
            }
        }
    }
}
