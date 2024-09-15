// This file is part of cuniq. Copyright © 2024 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use std::fs::File;
use std::io::{self, BufWriter, ErrorKind, IsTerminal, Write};
use std::process::ExitCode;

use bstr::ByteSlice;
use clap::Parser;

use line_cardinality::{CountUnique, Error, ErrorCause, HashingLineCounter, HyperLogLog, InexactHashingLineCounter, LineCounter, ReportUnique};

use crate::cli_args::{CliArgs, Mode};

mod cli_args;

/// constants generated in build.rs
pub mod constants {
    include!(env!("CONSTANTS_PATH"));
}

type Count = u64;

/// This can happen if someone pipes our stdout into `head` or some such
static STDOUT_ERROR_MESSAGE: &str = "failed to write to stdout";

fn main() -> ExitCode {
    let args = CliArgs::parse();
    match (args.trim, args.lowercase) {
        (false, false) => run_with_const_parameters::<false, false>(args),
        (false, true) => run_with_const_parameters::<false, true>(args),
        (true, false) => run_with_const_parameters::<true, false>(args),
        (true, true) => run_with_const_parameters::<true, true>(args),
    }
}

fn run_with_const_parameters<const TRIM: bool, const LOWERCASE: bool>(args: CliArgs) -> ExitCode {
    let result = if args.report {
        report::<TRIM, LOWERCASE>(args)
    } else {
        count::<TRIM, LOWERCASE>(args)
    };
    if let Err(e) = result {
        match e.get_cause() {
            ErrorCause::Io(cause) => {
                match cause.kind() {
                    ErrorKind::BrokenPipe => (),
                    _ => eprintln!("{e}: {cause:?}"),
                }
            }
            ErrorCause::Size(_) | ErrorCause::User => eprintln!("{e}"),
        }
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn report<const TRIM: bool, const LOWERCASE: bool>(args: CliArgs) -> Result<(), Error> {
    match args.mode {
        Mode::Exact => {
            let mut processor = HashingLineCounter::<Count, _>::with_line_mapper_and_capacity(preprocess_line::<TRIM, LOWERCASE>, args.size.unwrap_or(0));
            process_input(&args, &mut processor)?;
            let stdout = io::stdout().lock();
            let mut writer = BufWriter::new(stdout);
            if args.sort {
                let mut report = processor.to_report_vec();
                report.sort_unstable_by(|(a, _), (b, _)| a.as_slice().as_bstr().cmp(b.as_slice().as_bstr()));
                for (line, count) in report.iter() {
                    write_line(&mut writer, line, count)?;
                }
                writer.flush().map_err(|e| Error::io_static(STDOUT_ERROR_MESSAGE, e))?;
                std::mem::forget(report); // same explanation as below
            } else {
                for (line, count) in &processor {
                    write_line(&mut writer, line, count)?;
                }
                writer.flush().map_err(|e| Error::io_static(STDOUT_ERROR_MESSAGE, e))?;

                // leak the hash map and prevent Drop (and its destructor) from being run.
                // This is useful because cleaning up the hash set takes a significant amount of time, and the
                // OS is going to do it for us regardless.
                std::mem::forget(processor);
            }
            Ok(())
        }
        _ => Err(Error::message(format!("{} mode cannot generate cardinality reports", args.mode))),
    }
}

#[inline(always)]
fn write_line<T: Write>(writer: &mut T, line: &[u8], count: &Count) -> Result<(), Error> {
    write!(writer, "{count:7} ").map_err(|e| Error::io_static(STDOUT_ERROR_MESSAGE, e))?;
    writer.write_all(line).map_err(|e| Error::io_static(STDOUT_ERROR_MESSAGE, e))?;
    writeln!(writer).map_err(|e| Error::io_static(STDOUT_ERROR_MESSAGE, e))
}

fn count<const TRIM: bool, const LOWERCASE: bool>(args: CliArgs) -> Result<(), Error> {
    match args.mode {
        Mode::Exact => {
            let mut processor = LineCounter::with_line_mapper_and_capacity(preprocess_line::<TRIM, LOWERCASE>, args.size.unwrap_or(0));
            process_input(&args, &mut processor)?;
            println!("{}", processor.count());
            std::mem::forget(processor); // same explanation as above
        }
        Mode::NearExact => {
            let mut processor = InexactHashingLineCounter::with_line_mapper_and_capacity(preprocess_line::<TRIM, LOWERCASE>, args.size.unwrap_or(0));
            process_input(&args, &mut processor)?;
            println!("{}", processor.count());
            std::mem::forget(processor); // same explanation as above
        }
        Mode::Estimate => {
            let mut processor = if let Some(size) = args.size {
                let size = usize::max(16, size); // make size at least 16
                let size = previous_power_of_2(size); // reduce size to nearest power of 2
                HyperLogLog::with_line_mapper_and_capacity(preprocess_line::<TRIM, LOWERCASE>, size)?
            } else {
                HyperLogLog::with_line_mapper(preprocess_line::<TRIM, LOWERCASE>)
            };
            process_input(&args, &mut processor)?;
            println!("{}", processor.count());
            std::mem::forget(processor); // same explanation as above
        }
    }
    Ok(())
}

fn process_input<T>(args: &CliArgs, processor: &mut T) -> Result<(), Error>
where
    T: line_cardinality::CountUniqueFromReadFile,
{

    // pre-open all files so that we can display any errors and abort *before* doing work
    let mut files: Vec<File> = Vec::with_capacity(args.files.len());
    for path in &args.files {
        let file = File::open(path).map_err(|e| Error::io(format!("error opening file \"{}\"", path.display()), e))?;
        files.push(file);
    }

    process_stdin(args, processor)?;

    use cfg_if::cfg_if;
    cfg_if! {
        if #[cfg(feature = "memmap")] {
            if args.no_memmap {
                // process without memmap
                processor.count_unique_in_files(&files)?;
            } else if args.memmap {
                // use memmap forced by user
                use line_cardinality::CountUniqueFromMemmapFile;
                processor.count_unique_in_memmap_files(&files)?;
            } else {
                cfg_if! {
                    if #[cfg(unix)] {
                        // by default, process with memmap on unix platforms
                        use line_cardinality::CountUniqueFromMemmapFile;
                        processor.count_unique_in_memmap_files(&files)?;
                    } else {
                        // by default, process without memmap on non-unix platforms
                        processor.count_unique_in_files(&files)?;
                    }
                }
            }
        } else {
            if args.memmap {
                Err(Error::message_static("This cuniq binary was compiled without memmap support"))?;
            } else {
                // process without memmap
                processor.count_unique_in_files(&files)?;
            }
        }
    }
    Ok(())
}

#[inline(always)]
fn process_stdin<T>(args: &CliArgs, processor: &mut T) -> Result<(), Error>
where
    T: CountUnique,
{
    if !args.no_stdin {
        let stdin_handle = io::stdin().lock();
        if !stdin_handle.is_terminal() {
            processor.count_unique_in_read(stdin_handle)?;
        }
    }
    Ok(())
}

#[inline(always)]
fn preprocess_line<'a, const TRIM: bool, const LOWERCASE: bool>(line: &'a [u8], buffer: &'a mut Vec<u8>) -> &'a [u8] {
    let trimmed = if TRIM {
        line.trim()
    } else {
        line
    };
    if LOWERCASE {
        buffer.clear();
        trimmed.to_lowercase_into(buffer);
        buffer
    } else {
        trimmed
    }
}

/// Get the previous (or current) power of 2 for a number.
fn previous_power_of_2(n: usize) -> usize {
    if n == 0 {
        0
    } else {
        let zeros = n.leading_zeros();
        1usize << (usize::BITS - zeros - 1)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_previous_power_of_2() {
        assert_eq!(previous_power_of_2(0), 0, "case 0");
        assert_eq!(previous_power_of_2(1), 1, "case 1");
        assert_eq!(previous_power_of_2(2), 2, "case 2");
        assert_eq!(previous_power_of_2(3), 2, "case 3");
        assert_eq!(previous_power_of_2(4), 4, "case 4");
        assert_eq!(previous_power_of_2(5), 4, "case 5");
        assert_eq!(previous_power_of_2(6), 4, "case 6");
        assert_eq!(previous_power_of_2(7), 4, "case 7");
        assert_eq!(previous_power_of_2(8), 8, "case 8");
        assert_eq!(previous_power_of_2(9), 8, "case 9");
        assert_eq!(previous_power_of_2(10), 8, "case 10");
        assert_eq!(previous_power_of_2(11), 8, "case 11");
        assert_eq!(previous_power_of_2(12), 8, "case 12");
        assert_eq!(previous_power_of_2(13), 8, "case 13");
        assert_eq!(previous_power_of_2(14), 8, "case 14");
        assert_eq!(previous_power_of_2(15), 8, "case 15");
        assert_eq!(previous_power_of_2(16), 16, "case 16");
        assert_eq!(previous_power_of_2(17), 16, "case 17");
        assert_eq!(previous_power_of_2(65535), 32768, "case 65535");
        assert_eq!(previous_power_of_2(65536), 65536, "case 65536");
        assert_eq!(previous_power_of_2(65537), 65536, "case 65537");
        assert_eq!(previous_power_of_2(usize::MAX), 1usize.rotate_right(1), "case max");
    }
}
