#![forbid(unsafe_code)]

use std::{error::Error, fmt, str::FromStr};

use uuid::{Uuid, Version};

pub mod session;

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

macro_rules! typed_id {
    ($name:ident) => {
        #[doc = concat!("Typed UUIDv7 identifier for ", stringify!($name), ".")]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(ResourceId);

        impl $name {
            /// Creates this typed identifier from a validated resource ID.
            #[must_use]
            pub const fn from_resource_id(value: ResourceId) -> Self {
                Self(value)
            }

            /// Creates this typed identifier from a UUIDv7 value.
            pub fn from_uuid(value: Uuid) -> Result<Self, ParseResourceIdError> {
                ResourceId::from_uuid(value).map(Self)
            }

            /// Returns the underlying UUID.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0.as_uuid()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = ParseResourceIdError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                ResourceId::parse(value).map(Self)
            }
        }
    };
}

typed_id!(OrganizationId);
typed_id!(ProjectId);
typed_id!(UserId);
typed_id!(MembershipId);
typed_id!(AuditEventId);
typed_id!(OperationId);
typed_id!(SessionId);

/// UTC time represented as signed microseconds since the Unix epoch.
///
/// Persistence adapters map this value to `TIMESTAMPTZ(6)` on PostgreSQL and
/// an integer UTC epoch value on SQLite.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UtcTimestamp(i64);

impl UtcTimestamp {
    /// Creates a UTC timestamp from Unix epoch microseconds.
    #[must_use]
    pub const fn from_unix_micros(value: i64) -> Self {
        Self(value)
    }

    /// Returns signed Unix epoch microseconds.
    #[must_use]
    pub const fn unix_micros(self) -> i64 {
        self.0
    }
}

/// Organization persisted by the repository contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Organization {
    pub id: OrganizationId,
    pub slug: String,
    pub name: String,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub version: i64,
}

/// Project persisted within an organization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    pub id: ProjectId,
    pub organization_id: OrganizationId,
    pub slug: String,
    pub name: String,
    pub default_timezone: String,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub version: i64,
}

/// A local user. Credential material is deliberately not part of this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalUser {
    pub id: UserId,
    pub normalized_username: String,
    pub display_name: String,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub version: i64,
}

/// Stable role values shared by both persistence engines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Owner,
    Admin,
    Editor,
    Operator,
    Viewer,
}

impl Role {
    /// Returns the stable database/API spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Editor => "editor",
            Self::Operator => "operator",
            Self::Viewer => "viewer",
        }
    }
}

impl FromStr for Role {
    type Err = ParseRoleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "editor" => Ok(Self::Editor),
            "operator" => Ok(Self::Operator),
            "viewer" => Ok(Self::Viewer),
            _ => Err(ParseRoleError),
        }
    }
}

/// Error returned for a role value outside the stable role contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseRoleError;

impl fmt::Display for ParseRoleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unknown membership role")
    }
}

impl Error for ParseRoleError {}

/// Organization- or project-scoped user membership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Membership {
    pub id: MembershipId,
    pub organization_id: OrganizationId,
    pub project_id: Option<ProjectId>,
    pub user_id: UserId,
    pub role: Role,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub version: i64,
}

/// Audit actor classes supported by the initial persistence schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditActorType {
    System,
    LocalCli,
}

impl AuditActorType {
    /// Returns the stable database spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::LocalCli => "local_cli",
        }
    }
}

/// Redacted bootstrap-specific audit metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapAuditMetadata {
    pub organization_id: OrganizationId,
    pub project_id: ProjectId,
    pub user_id: UserId,
    pub membership_id: MembershipId,
}

/// Append-only audit record exposed through the repository contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEvent {
    pub id: AuditEventId,
    pub organization_id: OrganizationId,
    pub project_id: Option<ProjectId>,
    pub actor_type: AuditActorType,
    pub actor_id: Option<UserId>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: ResourceId,
    pub request_id: OperationId,
    pub metadata: BootstrapAuditMetadata,
    pub occurred_at: UtcTimestamp,
}
