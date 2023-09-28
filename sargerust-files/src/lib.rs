use thiserror::Error;

// TODO: At some point, get rid of all the highlevel impl Parseables and use #[derive(Parseable)] or maybe #[derive(Deserializable)]
#[derive(Error, Debug)]
pub enum ParserError {
    #[error("The file's magic value does not match the expectation {magic}")]
    InvalidMagicValue { magic: u32 },

    #[error("The file is violating the expected format, because: {reason}")]
    FormatError { reason: &'static str },

    /// Represents an empty source. For example, an empty text file being given
    /// as input to `count_words()`.
    #[error("Source contains no data")]
    EmptySource,

    /// Represents a failure to read from input.
    #[error("Read error")]
    ReadError { source: std::io::Error },

    /// Represents all other cases of `std::io::Error`.
    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    StringConversationError(#[from] std::ffi::IntoStringError),

    #[error(transparent)]
    UTF8ConversationError(#[from] std::string::FromUtf8Error),
}

pub mod adt;
pub mod common;
pub mod highlevel;
pub mod m2;
pub mod wdt;
pub mod wmo;
