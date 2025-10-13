// This file is part of cuniq. Copyright © 2025 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! Multithreaded
//!
//! Optional feature if `parallel` is enabled, which also guarantees `memmap2` and `crossbeam-channel`

use super::util::ChunkIterator;
use super::{ByHash, ByMerge, Result};
use crate::io::buf::CountBuf;
use line_cardinality::{CountUniqueHash, Error, Merge};
use memmap2::MmapOptions;
use std::fs::File;
use std::hash::BuildHasher;
use std::{slice, thread};

const DEFAULT_CHUNK_SIZE: usize = 0x1 << 27; // 2^27 == 134217728 bytes == 128 MiB

pub(crate) trait CountParallel: CountBuf {
    /// Count unique lines in some newline-delimited files.
    fn count_unique_parallel_files<H>(&mut self, random_state: &H, files: &[File], threads: usize) -> Result<()>
    where
        H: BuildHasher + Clone + Send;

    /// Count unique lines in a newline-delimited file.
    fn count_unique_parallel_file<H>(&mut self, random_state: &H, file: &File, threads: usize) -> Result<()>
    where
        H: BuildHasher + Clone + Send;
}

impl<C> CountParallel for ByHash<C>
where
    C: CountUniqueHash,
{
    /// This approach does not require [`Merge`] but does require [`CountUniqueHash`], which means the worker threads can
    /// only hash and counting must be done from a single thread. n worker threads are created and sent chunks from the
    /// file. The workers hash lines in these chunks and then send them on to the counting thread.
    fn count_unique_parallel_files<H>(&mut self, random_state: &H, files: &[File], threads: usize) -> Result<()>
    where
        H: BuildHasher + Clone + Send,
    {
        let mut mem_maps = Vec::with_capacity(files.len());
        for file in files {
            // SAFETY: dealing with external file modification is out of scope
            let mem_map = unsafe {
                MmapOptions::new()
                    .map(file)
                    .map_err(|e| Error::io_static("failed to memmap file", e))?
            };

            #[cfg(unix)]
            {
                use memmap2::Advice;
                mem_map
                    .advise(Advice::WillNeed)
                    .map_err(|e| Error::io_static("failed to set memmap file to WillNeed mode", e))?;
            }

            mem_maps.push(mem_map);
        }

        let (hash_sender, hash_receiver) = crossbeam_channel::bounded::<Vec<u64>>(0);

        thread::scope(|scope| {
            // create worker threads and a channel to read the chunks
            let chunk_sender = {
                // move this into the block so the extra copy gets dropped at the end
                let hash_sender = hash_sender;
                let (chunk_sender, chunk_receiver) = crossbeam_channel::bounded::<&[u8]>(0);
                (0..threads).for_each(|_| {
                    let chunk_receiver = chunk_receiver.clone();
                    let hash_sender = hash_sender.clone();
                    let random_state = random_state.clone();
                    scope.spawn(move || {
                        while let Ok(bytes) = chunk_receiver.recv() {
                            let mut hashes = Vec::with_capacity(DEFAULT_CHUNK_SIZE);
                            let mut start: usize = 0;
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
                scope.spawn(move || {
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
                        self.0.count_hash(hash);
                    }
                }
                // this cannot end until all hash senders are done, so no need to join on them
            }

            Ok(())
        })
    }

    fn count_unique_parallel_file<H>(&mut self, random_state: &H, file: &File, threads: usize) -> Result<()>
    where
        H: BuildHasher + Clone + Send,
    {
        self.count_unique_parallel_files(random_state, slice::from_ref(file), threads)
    }
}

impl<C> CountParallel for ByMerge<C>
where
    C: CountUniqueHash + Merge + Clone + Send,
{
    /// This approach works by spawning n worker threads which are sent chunks from the files. Once
    /// all chunks are processed, the results are merged.
    fn count_unique_parallel_files<H>(&mut self, random_state: &H, files: &[File], threads: usize) -> Result<()>
    where
        H: BuildHasher + Clone + Send,
    {
        if files.len() == 1 {
            self.count_unique_parallel_file(random_state, &files[0], threads)
        } else {
            let mut mem_maps = Vec::with_capacity(files.len());
            for file in files {
                // SAFETY: dealing with external file modification is out of scope
                let mem_map = unsafe {
                    MmapOptions::new()
                        .map(file)
                        .map_err(|e| Error::io_static("failed to memmap file", e))?
                };

                #[cfg(unix)]
                {
                    use memmap2::Advice;
                    mem_map
                        .advise(Advice::WillNeed)
                        .map_err(|e| Error::io_static("failed to set memmap file to WillNeed mode", e))?;
                }

                mem_maps.push(mem_map);
            }

            thread::scope(|scope| {
                let (chunk_sender, join_handles) = {
                    let (chunk_sender, chunk_receiver) = crossbeam_channel::bounded::<&[u8]>(0);

                    let join_handles = (0..threads)
                        .map(|_| {
                            // spawn the thread
                            let random_state = random_state.clone();
                            let mut counter = self.clone();
                            let chunk_receiver = chunk_receiver.clone();
                            scope.spawn(move || {
                                while let Ok(chunk) = chunk_receiver.recv() {
                                    counter.count_unique_in_bytes(&random_state, chunk);
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
                    self.0.merge(&counter.0);
                }

                Ok(())
            })
        }
    }

    /// This approach works by splitting the file into roughly equal chunks. Each chunk is processed
    /// in its own thread. Once all chunks are done, the results are merged.
    fn count_unique_parallel_file<H>(&mut self, random_state: &H, file: &File, threads: usize) -> Result<()>
    where
        H: BuildHasher + Clone + Send,
    {
        // SAFETY: dealing with external file modification is out of scope
        let mem_map = unsafe {
            MmapOptions::new()
                .map(file)
                .map_err(|e| Error::io_static("failed to memmap file", e))?
        };

        #[cfg(unix)]
        {
            use memmap2::Advice;
            mem_map
                .advise(Advice::WillNeed)
                .map_err(|e| Error::io_static("failed to set memmap file to WillNeed mode", e))?;
        }

        let chunk_size = mem_map.len() / threads;
        let chunk_iter = ChunkIterator::from_memmap(&mem_map, chunk_size);

        thread::scope(|scope| {
            let join_handles = chunk_iter
                .map(|chunk| {
                    // spawn the thread
                    let random_state = random_state.clone();
                    let mut counter = self.clone();
                    scope.spawn(move || {
                        counter.count_unique_in_bytes(&random_state, chunk);
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
                self.0.merge(&counter.0);
            }

            Ok(())
        })
    }
}
