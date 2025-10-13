// This file is part of cuniq. Copyright © 2025 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

//! Single-threaded read-based file processing

use super::Result;
use crate::io::buf::CountBuf;
use std::fs::File;
use std::io::BufReader;

/// Provides capability to read data from newline-delimited files
pub(crate) trait CountRead {
    /// Count unique lines in some newline-delimited files.
    fn count_unique_in_files(&mut self, files: &[File]) -> Result<()>;

    /// Count unique lines in a newline-delimited file.
    fn count_unique_in_file(&mut self, file: &File) -> Result<()>;
}

impl<C> CountRead for C
where
    C: CountBuf,
{
    fn count_unique_in_files(&mut self, files: &[File]) -> Result<()> {
        for file in files {
            self.count_unique_in_file(file)?;
        }
        Ok(())
    }

    fn count_unique_in_file(&mut self, file: &File) -> Result<()> {
        let reader = BufReader::new(file);
        self.count_unique_in_read(reader)
    }
}
