// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use std::fs::File;

use memmap2::{Mmap, MmapOptions};

use crate::count_unique_impl::result::Error;
use crate::Result;
use crate::{CountUnique, Merge};

/// Provides capability to read data from newline-delimited memory-mapped files
pub trait CountUniqueFromMemmapFile: CountUnique {
    /// Count unique lines in some newline-delimited files.
    fn count_unique_in_memmap_files(&mut self, files: &[File]) -> Result;

    /// Count unique lines in a newline-delimited file.
    fn count_unique_in_memmap_file(&mut self, file: &File) -> Result;
}

impl<T> CountUniqueFromMemmapFile for T
where
    T: CountUnique,
{
    fn count_unique_in_memmap_files(&mut self, files: &[File]) -> Result {
        for file in files {
            self.count_unique_in_memmap_file(file)?;
        }
        Ok(())
    }

    fn count_unique_in_memmap_file(&mut self, file: &File) -> Result {
        let mem_map =
            unsafe { Mmap::map(file) }.map_err(|e| Error::io_static("failed to memmap file", e))?;

        //TODO: we need to *not* Advice::Sequential if we ever get a parallel counting implementation
        #[cfg(unix)]
        {
            use memmap2::Advice;
            mem_map
                .advise(Advice::WillNeed)
                .map_err(|e| Error::io_static("failed to set memmap file to WillNeed mode", e))?;
            mem_map
                .advise(Advice::Sequential)
                .map_err(|e| Error::io_static("failed to set memmap file to Sequential mode", e))?;
        }

        self.count_unique_in_bytes(&mem_map);
        Ok(())
    }
}

/// Provides capability to read data from newline-delimited memory-mapped files in parallel
pub trait ParallelCountUniqueFromMemmapFile: Merge {
    /// Count unique lines in some newline-delimited files in parallel.
    fn parallel_count_unique_in_memmap_files(&mut self, files: &[File], threads: usize) -> Result;

    /// Count unique lines in a newline-delimited file in parallel.
    fn parallel_count_unique_in_memmap_file(&mut self, file: &File, threads: usize) -> Result;
}

impl<T> ParallelCountUniqueFromMemmapFile for T
where
    T: Merge + Send + Sync + 'static,
{
    fn parallel_count_unique_in_memmap_files(&mut self, files: &[File], threads: usize) -> Result {
        //TODO: this isn't great
        for file in files {
            self.parallel_count_unique_in_memmap_file(file, threads)?;
        }
        Ok(())
    }

    fn parallel_count_unique_in_memmap_file(&mut self, file: &File, threads: usize) -> Result {
        let mem_map = MmapOptions::new()
            .map_raw_read_only(file)
            .map_err(|e| Error::io_static("failed to memmap file", e))?;

        #[cfg(unix)]
        {
            use memmap2::Advice;
            mem_map
                .advise(Advice::WillNeed)
                .map_err(|e| Error::io_static("failed to set memmap file to WillNeed mode", e))?;
        }

        let len = mem_map.len();
        let chunk_size = len / threads;
        let start_ptr = mem_map.as_ptr();
        let mut chunk_start_ptr_inclusive = start_ptr;
        let end_ptr_exclusive = unsafe { start_ptr.add(len) };
        let mut chunk_end_ptr_exclusive = unsafe { chunk_start_ptr_inclusive.add(chunk_size) };
        let join_handles = (0..threads)
            .filter_map(|_thread_index| {
                if chunk_start_ptr_inclusive >= end_ptr_exclusive {
                    // edge case: we already ran out of data so we will not create this thread
                    None
                } else if chunk_end_ptr_exclusive >= end_ptr_exclusive {
                    // edge case: end ptr has passed end of mem_map
                    // just use real end and skip the newline search shit
                    // equivalent to  `chunk = &mem_map[chunk_start_index_inclusive..]`
                    let chunk = unsafe {
                        // should be used in the form `end.offset_from(start)`
                        let chunk_len = end_ptr_exclusive.offset_from(chunk_start_ptr_inclusive);
                        // `isize as usize` is a no-op, so we'll get garbage data if the isize was negative
                        let chunk_len = chunk_len as usize;
                        std::slice::from_raw_parts(chunk_start_ptr_inclusive, chunk_len)
                    };
                    Some(chunk)
                } else {
                    // equivalent to `search_range = &mem_map[chunk_end_index_exclusive..]`
                    let search_range = unsafe {
                        // should be used in the form `end.offset_from(start)`
                        let search_range_len =
                            end_ptr_exclusive.offset_from(chunk_end_ptr_exclusive);
                        // `isize as usize` is a no-op, so we'll get garbage data if the isize was negative
                        let search_range_len = search_range_len as usize;

                        std::slice::from_raw_parts(chunk_end_ptr_exclusive, search_range_len)
                    };
                    if let Some(newline_index) = memchr::memchr(b'\n', search_range) {
                        // equivalent to `chunk = &mem_map[chunk_start_index_inclusive..newline_index]`

                        // convert the search result into a direct pointer to the newline byte
                        let newline_ptr = unsafe { chunk_end_ptr_exclusive.add(newline_index) };

                        let chunk = unsafe {
                            // should be used in the form `end.offset_from(start)`
                            let chunk_len = newline_ptr.offset_from(chunk_start_ptr_inclusive);
                            // `isize as usize` is a no-op, so we'll get garbage data if the isize was negative
                            let chunk_len = chunk_len as usize;
                            std::slice::from_raw_parts(chunk_start_ptr_inclusive, chunk_len)
                        };
                        // update start of next chunk to be directly after this newline
                        chunk_start_ptr_inclusive = unsafe { newline_ptr.add(1) };

                        // update next of next chunk to be 1 chunk worth of size after the start
                        chunk_end_ptr_exclusive =
                            unsafe { chunk_start_ptr_inclusive.add(chunk_size) };

                        Some(chunk)
                    } else {
                        // edge case: we couldn't find a newline so this 1-word chunk will be the last thread
                        // equivalent to  `chunk = &mem_map[chunk_start_index_inclusive..]`
                        let chunk = unsafe {
                            // should be used in the form `end.offset_from(start)`
                            let chunk_len =
                                end_ptr_exclusive.offset_from(chunk_start_ptr_inclusive);
                            // `isize as usize` is a no-op, so we'll get garbage data if the isize was negative
                            let chunk_len = chunk_len as usize;
                            std::slice::from_raw_parts(chunk_start_ptr_inclusive, chunk_len)
                        };
                        Some(chunk)
                    }
                }
                .map(|chunk| {
                    // spawn the thread
                    let mut counter = self.clone();
                    std::thread::spawn(move || {
                        counter.count_unique_in_bytes(chunk);
                        counter
                    })
                })
            })
            .collect::<Vec<_>>();

        // We must collect into a vec here, because if I do it all in one iter chain `self` is still
        // immutably borrowed for the final iteration. We can't have that, as I need it to be mutable borrowed.

        for join_handle in join_handles {
            let counter = join_handle
                .join()
                .map_err(|e| Error::message(format!("thread join error: {e:?}")))?;
            let _ = counter;
            self.merge(&counter);
        }
        Ok(())
    }
}
