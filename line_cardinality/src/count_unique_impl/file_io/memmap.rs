// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use memmap2::{Mmap, MmapOptions, MmapRaw};
use std::fs::File;

use crate::count_unique_impl::init_hasher_state;
use crate::count_unique_impl::result::Error;
use crate::{CountUnique, Merge};
use crate::{CountUniqueHash, Result};

const DEFAULT_CHUNK_SIZE: usize = 0x1 << 27; // 2^27 == 134217728 bytes == 128 MiB

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
pub trait ParallelChunkedCountUniqueFromMemmapFile: CountUniqueHash {
    /// Count unique lines in some newline-delimited files in parallel.
    fn parallel_chunked_count_unique_in_memmap_files(
        &mut self,
        files: &[File],
        threads: usize,
    ) -> Result;
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct Chunk {
    start_ptr: *const u8,
    len: usize,
}

unsafe impl Send for Chunk {}

impl Chunk {
    #[inline(always)]
    unsafe fn from_ptr_range(start_ptr: *const u8, end_ptr: *const u8) -> Self {
        // should be used in the form `end.offset_from(start)`
        let len = end_ptr.offset_from(start_ptr);
        // `isize as usize` is a no-op, so we'll get garbage data if the isize was negative
        let len = len as usize;
        Self { start_ptr, len }
    }

    #[inline(always)]
    fn as_slice<'a>(self) -> &'a [u8] {
        unsafe { std::slice::from_raw_parts(self.start_ptr, self.len) }
    }
}

struct ChunkIterator {
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
    fn new(chunk_size: usize, mem_map: &MmapRaw) -> Self {
        let len = mem_map.len();
        let start_ptr = mem_map.as_ptr();
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
    type Item = Chunk;

    fn next(&mut self) -> Option<Self::Item> {
        if self.chunk_start_ptr >= self.end_ptr {
            // edge case: we have ran out of data and are ready to stop iterating
            None
        } else if self.chunk_end_ptr >= self.end_ptr {
            // edge case: end ptr has passed end of mem_map
            // just use real end and skip the newline search shit
            // equivalent to  `chunk = &mem_map[chunk_start_index_inclusive..]`
            let chunk = unsafe { Chunk::from_ptr_range(self.chunk_start_ptr, self.end_ptr) };

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

                let chunk = unsafe { Chunk::from_ptr_range(self.chunk_start_ptr, newline_ptr) };
                // update start of next chunk to be directly after this newline
                self.chunk_start_ptr = unsafe { newline_ptr.add(1) };

                // update next of next chunk to be 1 chunk worth of size after the start
                self.chunk_end_ptr = unsafe { self.chunk_start_ptr.add(self.chunk_size) };

                Some(chunk)
            } else {
                // edge case: we couldn't find a newline so this 1-word chunk will be the last thread
                // equivalent to  `chunk = &mem_map[chunk_start_index_inclusive..]`
                let chunk =
                    unsafe { Chunk::from_ptr_range(self.chunk_start_ptr, self.chunk_end_ptr) };
                
                // update start ptr so that the next iteration returns None
                self.chunk_start_ptr = self.end_ptr;
                
                Some(chunk)
            }
        }
    }
}

impl<T> ParallelChunkedCountUniqueFromMemmapFile for T
where
    T: CountUniqueHash,
{
    fn parallel_chunked_count_unique_in_memmap_files(
        &mut self,
        files: &[File],
        threads: usize,
    ) -> Result {
        let random_state = init_hasher_state();
        let (hash_sender, hash_receiver) = crossbeam_channel::bounded::<u64>(1024);

        // create worker threads and a channel to read the chunks
        let (chunk_sender, mut join_handles) = {
            let (chunk_sender, chunk_receiver) = crossbeam_channel::bounded::<Chunk>(1024);
            let join_handles = (0..threads)
                .map(|_| {
                    let chunk_receiver = chunk_receiver.clone();
                    let hash_sender = hash_sender.clone();
                    let random_state = random_state.clone();
                    std::thread::spawn(move || {
                        while let Ok(chunk) = chunk_receiver.recv() {
                            let hash = random_state.hash_one(chunk.as_slice());
                            hash_sender
                                .send(hash)
                                .expect("hash sender channel was unexpectedly closed");
                        }
                    })
                })
                .collect::<Vec<_>>();
            (chunk_sender, join_handles)
        };

        let mut mem_maps = Vec::with_capacity(files.len());
        for file in files {
            let mem_map = MmapOptions::new()
                .map_raw_read_only(file)
                .map_err(|e| Error::io_static("failed to memmap file", e))?;

            #[cfg(unix)]
            {
                use memmap2::Advice;
                mem_map.advise(Advice::WillNeed).map_err(|e| {
                    Error::io_static("failed to set memmap file to WillNeed mode", e)
                })?;
            }

            mem_maps.push(mem_map);
        }

        let chunk_sender_join_handle = {
            let chunk_iters = mem_maps
                .iter()
                .map(|mem_map| ChunkIterator::new(DEFAULT_CHUNK_SIZE, mem_map))
                .collect::<Vec<_>>();
            std::thread::spawn(move || {
                for chunk in chunk_iters.into_iter().flatten() {
                    chunk_sender.send(chunk).unwrap()
                }
            })
        };
        join_handles.push(chunk_sender_join_handle);

        // aggregate the hashes
        {
            // I want this to get dropped at a specific time, so I move it into this block
            let hash_receiver = hash_receiver;

            while let Ok(hash) = hash_receiver.recv() {
                self.count_hash(hash);
            }
        }

        // wait for the workers to exit
        for join_handle in join_handles {
            join_handle
                .join()
                .map_err(|e| Error::message(format!("thread join error: {e:?}")))?;
        }

        // ensure mem_maps still exists here, as if it gets dropped earlier we hit UB
        drop(mem_maps);

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
        if files.len() == 1 {
            self.parallel_count_unique_in_memmap_file(&files[0], threads)
        } else {
            let mut mem_maps = Vec::with_capacity(files.len());
            for file in files {
                let mem_map = MmapOptions::new()
                    .map_raw_read_only(file)
                    .map_err(|e| Error::io_static("failed to memmap file", e))?;

                #[cfg(unix)]
                {
                    use memmap2::Advice;
                    mem_map.advise(Advice::WillNeed).map_err(|e| {
                        Error::io_static("failed to set memmap file to WillNeed mode", e)
                    })?;
                }

                mem_maps.push(mem_map);
            }

            let (chunk_sender, join_handles) = {
                let (chunk_sender, chunk_receiver) = crossbeam_channel::bounded::<Chunk>(1024);

                let join_handles = (0..threads)
                    .map(|_| {
                        // spawn the thread
                        let mut counter = self.clone();
                        let chunk_receiver = chunk_receiver.clone();
                        std::thread::spawn(move || {
                            while let Ok(chunk) = chunk_receiver.recv() {
                                counter.count_unique_in_bytes(chunk.as_slice());
                            }
                            counter
                        })
                    })
                    .collect::<Vec<_>>();
                (chunk_sender, join_handles)
            };

            mem_maps
                .iter()
                .flat_map(|mem_map| ChunkIterator::new(DEFAULT_CHUNK_SIZE, mem_map))
                .for_each(|chunk| {
                    chunk_sender
                        .send(chunk)
                        .expect("chunk sender channel was unexpectedly closed")
                });

            for join_handle in join_handles {
                let counter = join_handle
                    .join()
                    .map_err(|e| Error::message(format!("thread join error: {e:?}")))?;
                let _ = counter;
                self.merge(&counter);
            }

            // ensure mem_maps still exists here, as if it gets dropped earlier we hit UB
            drop(mem_maps);

            Ok(())
        }
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

        let chunk_size = mem_map.len() / threads;
        let chunk_iter = ChunkIterator::new(chunk_size, &mem_map);
        let join_handles = chunk_iter
            .map(|chunk| {
                // spawn the thread
                let mut counter = self.clone();
                std::thread::spawn(move || {
                    counter.count_unique_in_bytes(chunk.as_slice());
                    counter
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

        // ensure mem_map still exists here, as if it gets dropped earlier we hit UB
        drop(mem_map);

        Ok(())
    }
}
