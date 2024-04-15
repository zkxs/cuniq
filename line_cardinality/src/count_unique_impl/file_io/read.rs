// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use std::fs::File;
use std::io::BufReader;

use crate::{CountUnique, Result};

/// Provides capability to read data from newline-delimited files
pub trait CountUniqueFromReadFile: CountUnique {
    /// Count unique lines in some newline-delimited files.
    fn count_unique_in_files(&mut self, files: &[File]) -> Result;

    /// Count unique lines in a newline-delimited file.
    fn count_unique_in_file(&mut self, file: &File) -> Result;
}

impl<T> CountUniqueFromReadFile for T
where
    T: CountUnique,
{
    fn count_unique_in_files(&mut self, files: &[File]) -> Result {
        for file in files {
            self.count_unique_in_file(file)?;
        }
        Ok(())
    }

    fn count_unique_in_file(&mut self, file: &File) -> Result {
        let reader = BufReader::new(file);
        self.count_unique_in_read(reader)
    }
}
