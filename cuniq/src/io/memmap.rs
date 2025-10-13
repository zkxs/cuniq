// This file is part of cuniq. Copyright © 2025 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! Single-threaded memmap-based file processing
//!
//! Optional feature if `memmap` is enabled.

use super::Result;
use crate::CountUnique;
use line_cardinality::Error;
use memmap2::Mmap;
use std::fs::File;

/// Provides capability to read data from newline-delimited memory-mapped files
pub trait CountUniqueFromMemmapFile: CountUnique {
    /// Count unique lines in some newline-delimited files.
    fn count_unique_in_memmap_files(&mut self, files: &[File]) -> Result<()>;

    /// Count unique lines in a newline-delimited file.
    fn count_unique_in_memmap_file(&mut self, file: &File) -> Result<()>;
}

impl<T> CountUniqueFromMemmapFile for T
where
    T: CountUnique,
{
    fn count_unique_in_memmap_files(&mut self, files: &[File]) -> Result<()> {
        for file in files {
            self.count_unique_in_memmap_file(file)?;
        }
        Ok(())
    }

    fn count_unique_in_memmap_file(&mut self, file: &File) -> Result<()> {
        // SAFETY: dealing with external file modification is out of scope
        let mem_map = unsafe { Mmap::map(file) }.map_err(|e| Error::io_static("failed to memmap file", e))?;

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
