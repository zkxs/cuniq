// This file is part of cuniq. Copyright © 2025 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! Internal utilities used for IO.

pub(crate) struct ChunkIterator<'a> {
    /// desired chunk size
    chunk_size: usize,
    /// entire range of data
    data: &'a [u8],
    /// inclusive start index for next chunk
    chunk_start_index: usize,
    /// desired exclusive end index for next chunk
    chunk_end_index: usize,
}

impl<'a> ChunkIterator<'a> {
    #[cfg(feature = "memmap")]
    pub fn from_memmap(mem_map: &'a memmap2::Mmap, chunk_size: usize) -> Self {
        Self::new(mem_map, chunk_size)
    }

    pub fn new(data: &'a [u8], chunk_size: usize) -> Self {
        let chunk_start_index = 0;
        let chunk_end_index = chunk_size;
        Self {
            chunk_size,
            data,
            chunk_start_index,
            chunk_end_index,
        }
    }
}

impl<'a> Iterator for ChunkIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.chunk_start_index >= self.data.len() {
            // edge case: we have ran out of data and are ready to stop iterating
            None
        } else if self.chunk_end_index > self.data.len() {
            // edge case: end index has passed end of mem_map
            // just use real end and skip the newline search shit
            let chunk = &self.data[self.chunk_start_index..];

            // update start index so that the next iteration returns None
            self.chunk_start_index = self.data.len();

            Some(chunk)
        } else {
            let search_range = &self.data[self.chunk_end_index..];
            if let Some(newline_index) = memchr::memchr(b'\n', search_range) {
                let newline_index = self.chunk_end_index + newline_index;
                let chunk = &self.data[self.chunk_start_index..newline_index];

                // update start of next chunk to be directly after this newline
                self.chunk_start_index = newline_index + 1;

                // update next of next chunk to be 1 chunk worth of size after the start
                self.chunk_end_index = self.chunk_start_index + self.chunk_size;

                Some(chunk)
            } else {
                // edge case: we couldn't find a newline so this 1-word chunk will be the last thread
                let chunk = &self.data[self.chunk_start_index..];

                // update start ptr so that the next iteration returns None
                self.chunk_start_index = self.data.len();

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
    pub fn new(bytes: &'a [u8]) -> Self {
        let start = 0;
        let memchr = memchr::memchr_iter(b'\n', bytes);
        Self { bytes, start, memchr }
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

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use std::io::Read;
    use std::path::PathBuf;

    #[test]
    fn test_line_iterator() {
        let mut path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.push("test_files");
        path.push("large.txt");
        let mut file = File::open(path).unwrap();
        let mut buf = Vec::new();
        let expected_size = file.read_to_end(&mut buf).unwrap();
        let actual_size: usize = LineIterator::new(&buf).map(|chunk| chunk.len() + 1).sum();
        assert_eq!(
            actual_size, expected_size,
            "expected sum of all lines to match file size"
        );
    }

    #[test]
    fn test_chunk_iterator() {
        let mut path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.push("test_files");
        path.push("large.txt");
        let mut file = File::open(path).unwrap();
        let mut buf = Vec::new();
        let expected_size = file.read_to_end(&mut buf).unwrap();
        let actual_size = ChunkIterator::new(&buf, 1024)
            .map(|chunk| chunk.len() + 1) // +1 because we skip a newline character
            .sum::<usize>()
            - 1; // -1 because we DON'T omit the final newline
        assert_eq!(
            actual_size, expected_size,
            "expected sum of all chunks to match file size"
        );
    }
}
