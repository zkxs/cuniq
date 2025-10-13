// This file is part of cuniq. Copyright © 2025 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use std::fs::File;
use std::io::{BufWriter, ErrorKind, IsTerminal, Write};
use std::process::ExitCode;

use clap::Parser;

use cfg_if::cfg_if;

use line_cardinality::{
    CountUnique, Error, ErrorCause, HyperLogLog, LosslessHashingLineCounter, LossyHashingLineCounter,
    ReportUniqueLineHash,
};

use crate::cli_args::{CliArgs, Mode};
use crate::io::buf::CountBuf;
use crate::io::{ByHash, ByLine, ByMerge};
use crate::io::read::CountRead;

#[cfg(feature = "parallel")]
use crate::io::parallel::CountParallel;

mod cli_args;
mod hash;
mod io;

/// constants generated in build.rs
pub(crate) mod constants {
    include!(env!("CONSTANTS_PATH"));
}

type Count = u64;

/// This can happen if someone pipes our stdout into `head` or some such
static STDOUT_ERROR_MESSAGE: &str = "failed to write to stdout";

fn main() -> ExitCode {
    let args = CliArgs::parse();
    let result = if args.report { report(args) } else { count(args) };
    if let Err(e) = result {
        match e.get_cause() {
            ErrorCause::Io(cause) => match cause.kind() {
                ErrorKind::BrokenPipe => (),
                _ => eprintln!("{e}: {cause:?}"),
            },
            ErrorCause::Size(_) | ErrorCause::User => eprintln!("{e}"),
        }
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn report(args: CliArgs) -> Result<(), Error> {
    match args.mode {
        Mode::Exact => {
            let processor = LosslessHashingLineCounter::<Count>::with_capacity(args.size.unwrap_or(0));
            let mut processor = ByLine(processor);
            process_input(&args, &mut processor)?;
            let processor = processor.0;
            let stdout = std::io::stdout().lock();
            let mut writer = BufWriter::new(stdout);
            if args.sort {
                let mut report = processor.to_report_vec();
                report.sort_unstable_by(|(a, _), (b, _)| a.as_slice().cmp(b.as_slice()));
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
        _ => Err(Error::message(format!(
            "{} mode cannot generate cardinality reports",
            args.mode
        ))),
    }
}

#[inline(always)]
fn write_line<T: Write>(writer: &mut T, line: &[u8], count: &Count) -> Result<(), Error> {
    write!(writer, "{count:7} ").map_err(|e| Error::io_static(STDOUT_ERROR_MESSAGE, e))?;
    writer
        .write_all(line)
        .map_err(|e| Error::io_static(STDOUT_ERROR_MESSAGE, e))?;
    writeln!(writer).map_err(|e| Error::io_static(STDOUT_ERROR_MESSAGE, e))
}

fn count(args: CliArgs) -> Result<(), Error> {
    match args.mode {
        Mode::Exact => {
            let processor = LosslessHashingLineCounter::<()>::with_capacity(args.size.unwrap_or(0));
            let mut processor = ByLine(processor);
            process_input(&args, &mut processor)?;
            let processor = processor.0;
            println!("{}", processor.count());
            std::mem::forget(processor); // same explanation as above
        }
        Mode::NearExact => {
            let processor = LossyHashingLineCounter::with_capacity(args.size.unwrap_or(0));
            let mut processor = ByHash(processor);
            cfg_if! {
                if #[cfg(feature = "memmap")] {
                    cfg_if! {
                        if #[cfg(feature = "parallel")] {
                            let threads = args.threads.unwrap_or_else(num_cpus::get);
                            if threads > 1 {
                                parallel_process_input(&args, &mut processor, threads)?;
                            } else {
                                process_input(&args, &mut processor)?;
                            }
                        } else {
                            process_input(&args, &mut processor)?;
                        }
                    }
                } else {
                    process_input(&args, &mut processor)?;
                }
            }
            let processor = processor.0;
            println!("{}", processor.count());
            std::mem::forget(processor); // same explanation as above
        }
        Mode::Estimate => {
            let processor = if let Some(size) = args.size {
                let size = usize::max(16, size); // make size at least 16
                let size = previous_power_of_2(size); // reduce size to nearest power of 2
                HyperLogLog::with_capacity(size)?
            } else {
                HyperLogLog::new()
            };
            let mut processor = ByMerge(processor);
            cfg_if! {
                if #[cfg(feature = "memmap")] {
                    cfg_if! {
                        if #[cfg(feature = "parallel")] {
                            let threads = args.threads.unwrap_or_else(num_cpus::get);
                            if threads > 1 {
                                parallel_process_input(&args, &mut processor, threads)?;
                            } else {
                                process_input(&args, &mut processor)?;
                            }
                        } else {
                            process_input(&args, &mut processor)?;
                        }
                    }
                } else {
                    process_input(&args, &mut processor)?;
                }
            }
            let processor = processor.0;
            println!("{}", processor.count());
            std::mem::forget(processor); // same explanation as above
        }
    }
    Ok(())
}

#[cfg(feature = "parallel")]
fn parallel_process_input<T>(args: &CliArgs, processor: &mut T, threads: usize) -> Result<(), Error>
where
    T: CountParallel,
{
    // pre-open all files so that we can display any errors and abort *before* doing work
    let mut files: Vec<File> = Vec::with_capacity(args.files.len());
    for path in &args.files {
        let file = File::open(path).map_err(|e| Error::io(format!("error opening file \"{}\"", path.display()), e))?;
        files.push(file);
    }

    process_stdin(args, processor)?;

    processor.count_unique_parallel_files(&files, threads)
}

fn process_input<T>(args: &CliArgs, processor: &mut T) -> Result<(), Error>
where
    T: CountBuf,
{
    // pre-open all files so that we can display any errors and abort *before* doing work
    let mut files: Vec<File> = Vec::with_capacity(args.files.len());
    for path in &args.files {
        let file = File::open(path).map_err(|e| Error::io(format!("error opening file \"{}\"", path.display()), e))?;
        files.push(file);
    }

    process_stdin(args, processor)?;

    cfg_if! {
        if #[cfg(feature = "memmap")] {
            use io::memmap::CountMemmap;
            if args.no_memmap {
                // process without memmap
                processor.count_unique_in_files(&files)?;
            } else if args.memmap {
                // use memmap forced by user
                processor.count_unique_in_memmap_files(&files)?;
            } else {
                cfg_if! {
                    if #[cfg(unix)] {
                        // by default, process with memmap on unix platforms
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
    T: CountBuf,
{
    if !args.no_stdin {
        let stdin_handle = std::io::stdin().lock();
        if !stdin_handle.is_terminal() {
            processor.count_unique_in_read(stdin_handle)?;
        }
    }
    Ok(())
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
