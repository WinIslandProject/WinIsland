mod abi;

pub mod draw;
pub mod fault;
pub mod host;
pub mod lifecycle;
pub mod loader;
pub mod registry;
pub mod resources;
pub mod runtime;
mod services;

use std::fmt;

#[derive(Debug)]
pub enum PluginHostError {
    Io(String),
    Invalid(String),
    Execution(String),
    Worker(String),
}

impl fmt::Display for PluginHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message)
            | Self::Invalid(message)
            | Self::Execution(message)
            | Self::Worker(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for PluginHostError {}
