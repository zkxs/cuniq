// This file is part of cuniq. Copyright © 2024 cuniq contributors.
// cuniq is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use std::{env, fs, io};
use std::collections::HashSet;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> io::Result<()> {
    let out_dir: PathBuf = env::var("OUT_DIR").expect("bad out dir?").into();
    let constants_path = out_dir.join("constants.rs");
    create_constants(constants_path.as_path())?;
    println!("cargo:rustc-env=CONSTANTS_PATH={}", constants_path.to_str().unwrap());
    Ok(())
}

/// generate rust source to send constants into the actual build
fn create_constants<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let git_commit_hash = git_commit_hash();
    let clap_version = clap_version(&git_commit_hash);

    let file = fs::File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_fmt(format_args!("pub const GIT_COMMIT_HASH: &str = \"{git_commit_hash}\";\n"))?;
    writer.write_fmt(format_args!("pub const CLAP_VERSION: &str = \"{clap_version}\";\n"))?;
    writer.flush()
}

/// override version string displayed by clap
fn clap_version(git_commit_hash: &str) -> String {
    format!("{} commit {}\\nBuilt with feature flags: [{}]\\nCopyright 2024 cuniq contributors\\nLicense: GNU GPL v3.0 or any later version\\nWritten by: {}",
            env!("CARGO_PKG_VERSION"),
            git_commit_hash, feature_diff(),
            env!("CARGO_PKG_AUTHORS"),
    )
}

/// Read git commit hash
fn git_commit_hash() -> String {
    let output = Command::new("git").args(["rev-parse", "HEAD"]).output().expect("failed to get git commit hash");
    let untrimmed_git_commit_hash = String::from_utf8(output.stdout).expect("failed to read git commit hash as UTF-8");
    untrimmed_git_commit_hash.trim().to_string()
}

/// Calculate the diff from actual features and expected features
fn feature_diff() -> String {
    // features we expect for this binary
    let mut expected_features = HashSet::from([
        "compile-time-rng",
        "memmap",
    ]);

    // features that are just aliases for sets of real features
    let ignored_features = HashSet::from([
        "default",
    ]);

    const FEATURE_PREFIX: &str = "CARGO_FEATURE_"; // all features are passed as environment vars with this prefix
    let mut features = String::new();
    let mut first = true;
    env::vars()
        .filter(|(var, _value)| var.starts_with(FEATURE_PREFIX))
        .map(|(var, _value)| var[FEATURE_PREFIX.len()..].to_lowercase()) // environment vars need to be lowercased
        .map(|var| var.replace('_', "-")) // environment vars need to have their snake case fixed
        .filter(|feature| !ignored_features.contains(feature.as_str()))
        .for_each(|feature| {
            // add each unexpected feature
            if !expected_features.remove(feature.as_str()) {
                if first {
                    first = false;
                } else {
                    features.push(',');
                }
                features.push('+');
                features.push_str(&feature);
            }
        });
    expected_features.into_iter().for_each(|feature| {
        // add each missing feature
        if first {
            first = false;
        } else {
            features.push(',');
        }
        features.push('-');
        features.push_str(feature);
    });
    features
}
