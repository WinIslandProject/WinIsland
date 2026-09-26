use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum PlatformError {
    Unavailable(&'static str),
    Backend(String),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(capability) => write!(formatter, "{capability} is unavailable"),
            Self::Backend(message) => formatter.write_str(message),
        }
    }
}

impl Error for PlatformError {}
