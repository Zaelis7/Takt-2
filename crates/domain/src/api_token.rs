use std::{error::Error, fmt, net::IpAddr, str::FromStr};

use crate::{ApiTokenId, OrganizationId, ProjectId, UtcTimestamp};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenKind {
    Personal,
    Service,
}

impl ApiTokenKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Service => "service",
        }
    }
}

impl FromStr for ApiTokenKind {
    type Err = ApiTokenValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "personal" => Ok(Self::Personal),
            "service" => Ok(Self::Service),
            _ => Err(ApiTokenValidationError::InvalidKind),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenStatus {
    Active,
    Revoked,
    Expired,
}

impl ApiTokenStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ApiTokenScope(String);

impl ApiTokenScope {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ApiTokenPrefix(String);

impl ApiTokenPrefix {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ApiTokenPrefix {
    type Err = ApiTokenValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !(8..=32).contains(&value.len())
            || !value.starts_with("takt_")
            || !value[5..]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(ApiTokenValidationError::InvalidPrefix);
        }
        Ok(Self(value.to_owned()))
    }
}

impl FromStr for ApiTokenScope {
    type Err = ApiTokenValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (resource, verb) = value
            .split_once(':')
            .ok_or(ApiTokenValidationError::InvalidScope)?;
        if value.len() > 100 || !valid_scope_part(resource) || !valid_scope_part(verb) {
            return Err(ApiTokenValidationError::InvalidScope);
        }
        Ok(Self(value.to_owned()))
    }
}

fn valid_scope_part(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IpNetwork {
    address: IpAddr,
    prefix_length: u8,
}

impl IpNetwork {
    #[must_use]
    pub fn contains(self, candidate: IpAddr) -> bool {
        match (self.address, candidate) {
            (IpAddr::V4(network), IpAddr::V4(candidate)) => {
                let mask = ipv4_mask(self.prefix_length);
                u32::from(network) & mask == u32::from(candidate) & mask
            }
            (IpAddr::V6(network), IpAddr::V6(candidate)) => {
                let mask = ipv6_mask(self.prefix_length);
                u128::from(network) & mask == u128::from(candidate) & mask
            }
            _ => false,
        }
    }
}

impl fmt::Display for IpNetwork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.address, self.prefix_length)
    }
}

impl FromStr for IpNetwork {
    type Err = ApiTokenValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !(3..=64).contains(&value.len()) {
            return Err(ApiTokenValidationError::InvalidIpNetwork);
        }
        let (address, prefix) = value
            .split_once('/')
            .ok_or(ApiTokenValidationError::InvalidIpNetwork)?;
        let address =
            IpAddr::from_str(address).map_err(|_| ApiTokenValidationError::InvalidIpNetwork)?;
        let prefix_length = prefix
            .parse::<u8>()
            .map_err(|_| ApiTokenValidationError::InvalidIpNetwork)?;
        let canonical = match address {
            IpAddr::V4(address) if prefix_length <= 32 => {
                u32::from(address) & ipv4_mask(prefix_length) == u32::from(address)
            }
            IpAddr::V6(address) if prefix_length <= 128 => {
                u128::from(address) & ipv6_mask(prefix_length) == u128::from(address)
            }
            _ => false,
        };
        if !canonical {
            return Err(ApiTokenValidationError::InvalidIpNetwork);
        }
        Ok(Self {
            address,
            prefix_length,
        })
    }
}

const fn ipv4_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}

const fn ipv6_mask(prefix: u8) -> u128 {
    if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiToken {
    pub id: ApiTokenId,
    pub organization_id: OrganizationId,
    pub project_id: Option<ProjectId>,
    pub name: String,
    pub kind: ApiTokenKind,
    pub token_prefix: ApiTokenPrefix,
    pub scopes: Vec<ApiTokenScope>,
    pub ip_networks: Vec<IpNetwork>,
    pub expires_at: Option<UtcTimestamp>,
    pub last_used_at: Option<UtcTimestamp>,
    pub revoked_at: Option<UtcTimestamp>,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub version: i64,
}

impl ApiToken {
    #[must_use]
    pub fn status(&self, now: UtcTimestamp) -> ApiTokenStatus {
        if self.revoked_at.is_some() {
            ApiTokenStatus::Revoked
        } else if self.expires_at.is_some_and(|expires_at| expires_at <= now) {
            ApiTokenStatus::Expired
        } else {
            ApiTokenStatus::Active
        }
    }

    #[must_use]
    pub fn authorizes_source(&self, now: UtcTimestamp, source: IpAddr) -> bool {
        self.status(now) == ApiTokenStatus::Active
            && (self.ip_networks.is_empty()
                || self
                    .ip_networks
                    .iter()
                    .any(|network| network.contains(source)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenActor {
    token_id: ApiTokenId,
    organization_id: OrganizationId,
    project_id: Option<ProjectId>,
    scopes: Vec<ApiTokenScope>,
}

impl TokenActor {
    pub fn new(
        token_id: ApiTokenId,
        organization_id: OrganizationId,
        project_id: Option<ProjectId>,
        mut scopes: Vec<ApiTokenScope>,
    ) -> Result<Self, ApiTokenValidationError> {
        scopes.sort();
        scopes.dedup();
        if scopes.is_empty() || scopes.len() > 100 {
            return Err(ApiTokenValidationError::InvalidScopes);
        }
        Ok(Self {
            token_id,
            organization_id,
            project_id,
            scopes,
        })
    }

    #[must_use]
    pub fn allows(&self, required: &ApiTokenScope) -> bool {
        self.scopes.binary_search(required).is_ok()
    }

    #[must_use]
    pub const fn token_id(&self) -> ApiTokenId {
        self.token_id
    }

    #[must_use]
    pub const fn organization_id(&self) -> OrganizationId {
        self.organization_id
    }

    #[must_use]
    pub const fn project_id(&self) -> Option<ProjectId> {
        self.project_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenValidationError {
    InvalidKind,
    InvalidScope,
    InvalidScopes,
    InvalidPrefix,
    InvalidIpNetwork,
}

impl fmt::Display for ApiTokenValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidKind => "API token kind is invalid",
            Self::InvalidScope => "API token scope is invalid",
            Self::InvalidScopes => "API token scopes must contain 1-100 unique values",
            Self::InvalidPrefix => "API token prefix is invalid",
            Self::InvalidIpNetwork => "API token IP network must be canonical IPv4 or IPv6 CIDR",
        })
    }
}

impl Error for ApiTokenValidationError {}
