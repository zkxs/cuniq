// This file is part of line_cardinality. Copyright © 2024 line_cardinality contributors.
// line_cardinality is licensed under the GNU GPL v3.0 or any later version. See LICENSE file for full text.

use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::io;

pub(crate) type Result = std::result::Result<(), Error>;

#[derive(Debug)]
enum Message {
    Dynamic(String),
    Static(&'static str),
}

/// Contains the cause of an [`Error`]
#[derive(Debug)]
pub enum Cause {
    /// IO error
    Io(io::Error),
    /// Improper usize
    Size(usize),
    /// User-proved error message
    User,
}

/// Errors returned by line_cardinality
#[derive(Debug)]
pub struct Error {
    message: Message,
    cause: Cause,
}

impl Error {
    /// Wraps IO errors with a static message
    pub fn io_static(message: &'static str, cause: io::Error) -> Self {
        Self {
            message: Message::Static(message),
            cause: Cause::Io(cause),
        }
    }

    /// Wraps IO errors
    pub fn io(message: String, cause: io::Error) -> Self {
        Self {
            message: Message::Dynamic(message),
            cause: Cause::Io(cause),
        }
    }

    /// User-provided error with a static message
    pub fn message_static(message: &'static str) -> Self {
        Self {
            message: Message::Static(message),
            cause: Cause::User,
        }
    }

    /// User-provided error
    pub fn message(message: String) -> Self {
        Self {
            message: Message::Dynamic(message),
            cause: Cause::User,
        }
    }

    pub(crate) fn hyper_log_log(message: String, size: usize) -> Self {
        Self {
            message: Message::Dynamic(message),
            cause: Cause::Size(size),
        }
    }

    pub fn get_cause(&self) -> &Cause {
        &self.cause
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.cause {
            Cause::Io(e) => Some(e),
            Cause::Size(_) => None,
            Cause::User => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.message {
            Message::Dynamic(string) => write!(f, "{}", string),
            Message::Static(str) => write!(f, "{}", str),
        }
    }
}
