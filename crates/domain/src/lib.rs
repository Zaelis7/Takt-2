#![forbid(unsafe_code)]

use std::{error::Error, fmt, str::FromStr};

use uuid::{Uuid, Version};

/// Error returned when an external value is not a valid Takt resource UUIDv7.
#[derive(Debug)]
pub enum ParseResourceIdError {
    /// The input is not a syntactically valid UUID.
    InvalidUuid(uuid::Error),
    /// The UUID is valid but does not use version 7.
    UnsupportedVersion,
}

impl fmt::Display for ParseResourceIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUuid(_) => formatter.write_str("resource ID is not a valid UUID"),
            Self::UnsupportedVersion => formatter.write_str("resource ID must use UUID version 7"),
        }
    }
}

impl Error for ParseResourceIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidUuid(error) => Some(error),
            Self::UnsupportedVersion => None,
        }
    }
}

/// A typed, immutable identifier for a domain resource.
///
/// ID generation belongs at an application boundary so pure domain behavior
/// can use injected, deterministic values.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceId(Uuid);

impl ResourceId {
    /// Creates an identifier from an already generated UUID.
    pub fn from_uuid(value: Uuid) -> Result<Self, ParseResourceIdError> {
        if value.get_version() == Some(Version::SortRand) {
            Ok(Self(value))
        } else {
            Err(ParseResourceIdError::UnsupportedVersion)
        }
    }

    /// Parses a canonical UUID without consulting a clock or random source.
    pub fn parse(value: &str) -> Result<Self, ParseResourceIdError> {
        value.parse()
    }

    /// Returns the underlying UUID value.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ResourceId {
    type Err = ParseResourceIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::parse_str(value).map_err(ParseResourceIdError::InvalidUuid)?;
        Self::from_uuid(uuid)
    }
}
