use std::{fmt::Display, str::Utf8Error};

#[derive(Debug)]
pub enum DebugError {
    NixError(nix::Error),
    FunctionNotFound,
    InvalidType,
    IoError(std::io::Error),
    GimliError(gimli::Error),
    BreakpointInvalidState,
    InvalidRegister,
    NoBreakpointFound,
    NoSourceUnitFoundForCurrentPC,
    InvalidPC(u64),
    InvalidCommand(String),
    InvalidArgument(String),
    EncodingError(String),
}

impl From<Utf8Error> for DebugError {
    fn from(value: Utf8Error) -> Self {
        DebugError::EncodingError(value.to_string())
    }
}

impl From<gimli::Error> for DebugError {
    fn from(e: gimli::Error) -> Self {
        DebugError::GimliError(e)
    }
}

impl From<nix::Error> for DebugError {
    fn from(e: nix::Error) -> Self {
        DebugError::NixError(e)
    }
}

impl From<std::io::Error> for DebugError {
    fn from(e: std::io::Error) -> Self {
        DebugError::IoError(e)
    }
}

impl Display for DebugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format!("{:?}", self).fmt(f)
    }
}
