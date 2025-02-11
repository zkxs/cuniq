// This file is part of line_cardinality. Copyright © 2025 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! Optional feature if `parallel` is enabled, which also guarantees `memmap2`, `memchr`, and `crossbeam-channel`

use crate::count_unique_impl::file_io::util::{ChunkIterator, RawSlice};
use crate::count_unique_impl::init_hasher_state;
use crate::{CountUniqueHash, Error, Merge};
use memmap2::MmapOptions;
use std::fs::File;

const DEFAULT_CHUNK_SIZE: usize = 0x1 << 27; // 2^27 == 134217728 bytes == 128 MiB

/// Provides capability to read data from newline-delimited memory-mapped files in parallel
pub trait ParallelChunkedCountUniqueFromMemmapFile: CountUniqueHash {
    /// Count unique lines in some newline-delimited files in parallel.
    fn parallel_chunked_count_unique_in_memmap_files(
        &mut self,
        files: &[File],
        threads: usize,
    ) -> crate::count_unique_impl::result::Result;
}

impl<T> ParallelChunkedCountUniqueFromMemmapFile for T
where
    T: CountUniqueHash,
{
    fn parallel_chunked_count_unique_in_memmap_files(
        &mut self,
        files: &[File],
        threads: usize,
    ) -> crate::count_unique_impl::result::Result {
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

        let random_state = init_hasher_state();
        let (hash_sender, hash_receiver) = crossbeam_channel::bounded::<Vec<u64>>(0);

        // create worker threads and a channel to read the chunks
        let chunk_sender = {
            // move this into the block so the extra copy gets dropped at the end
            let hash_sender = hash_sender;
            let (chunk_sender, chunk_receiver) = crossbeam_channel::bounded::<RawSlice>(0);
            (0..threads).for_each(|_| {
                let chunk_receiver = chunk_receiver.clone();
                let hash_sender = hash_sender.clone();
                let random_state = random_state.clone();
                std::thread::spawn(move || {
                    while let Ok(chunk) = chunk_receiver.recv() {
                        let mut hashes = Vec::with_capacity(DEFAULT_CHUNK_SIZE);
                        let mut start: usize = 0;
                        let bytes = chunk.as_slice();
                        for newline_index in memchr::memchr_iter(b'\n', bytes) {
                            let hash = random_state.hash_one(&bytes[start..newline_index]);
                            hashes.push(hash);
                            start = newline_index + 1;
                        }
                        // handle trailing
                        if start < bytes.len() {
                            let hash = random_state.hash_one(&bytes[start..]);
                            hashes.push(hash);
                        }
                        hash_sender
                            .send(hashes)
                            .expect("hash sender channel was unexpectedly closed");
                    }
                });
            });
            chunk_sender
        };

        {
            // make sure the sender gets dropped at the end of this block
            let chunk_sender = chunk_sender;
            let chunk_iters = mem_maps
                .iter()
                .map(|mem_map| ChunkIterator::from_memmap(mem_map, DEFAULT_CHUNK_SIZE))
                .collect::<Vec<_>>();
            std::thread::spawn(move || {
                for chunk in chunk_iters.into_iter().flatten() {
                    chunk_sender
                        .send(chunk)
                        .expect("chunk sender channel was unexpectedly closed");
                }
            })
        };

        // aggregate the hashes
        {
            // I want this to get dropped at a specific time, so I move it into this block
            let hash_receiver = hash_receiver;

            while let Ok(hashes) = hash_receiver.recv() {
                for hash in hashes {
                    self.count_hash(hash);
                }
            }
            // this cannot end until all hash senders are done, so no need to join on them
        }

        // ensure mem_maps still exists here, as if it gets dropped earlier we hit UB
        drop(mem_maps);

        Ok(())
    }
}

/// Provides capability to read data from newline-delimited memory-mapped files in parallel
pub trait ParallelCountUniqueFromMemmapFile: Merge {
    /// Count unique lines in some newline-delimited files in parallel.
    fn parallel_count_unique_in_memmap_files(
        &mut self,
        files: &[File],
        threads: usize,
    ) -> crate::count_unique_impl::result::Result;

    /// Count unique lines in a newline-delimited file in parallel.
    fn parallel_count_unique_in_memmap_file(
        &mut self,
        file: &File,
        threads: usize,
    ) -> crate::count_unique_impl::result::Result;
}

impl<T> ParallelCountUniqueFromMemmapFile for T
where
    T: Merge + Send + Sync + 'static,
{
    fn parallel_count_unique_in_memmap_files(
        &mut self,
        files: &[File],
        threads: usize,
    ) -> crate::count_unique_impl::result::Result {
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
                let (chunk_sender, chunk_receiver) = crossbeam_channel::bounded::<RawSlice>(0);

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

            {
                // ensure chunk_sender is dropped early
                let chunk_sender = chunk_sender;
                mem_maps
                    .iter()
                    .flat_map(|mem_map| ChunkIterator::from_memmap(mem_map, DEFAULT_CHUNK_SIZE))
                    .for_each(|chunk| {
                        chunk_sender
                            .send(chunk)
                            .expect("chunk sender channel was unexpectedly closed")
                    });
            }

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

    fn parallel_count_unique_in_memmap_file(
        &mut self,
        file: &File,
        threads: usize,
    ) -> crate::count_unique_impl::result::Result {
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
        let chunk_iter = ChunkIterator::from_memmap(&mem_map, chunk_size);
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
