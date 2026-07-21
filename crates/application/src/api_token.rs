use std::{error::Error, fmt, net::IpAddr, str::FromStr};

use async_trait::async_trait;
use takt_domain::{
    ApiTokenId, AuditEvent, OrganizationId, ProjectId, UtcTimestamp,
    api_token::{
        ApiToken, ApiTokenKind, ApiTokenPrefix, ApiTokenScope, ApiTokenStatus, IpNetwork,
        TokenActor,
    },
};
use zeroize::Zeroizing;

use crate::{Argon2idConfig, NewAuditEvent, PasswordHash, PasswordHasher, RepositoryError};

const PREFIX_HEX_BYTES: usize = 8;
const SECRET_BYTES: usize = 32;
const PREFIX_LENGTH: usize = "takt_".len() + PREFIX_HEX_BYTES * 2;

pub const API_TOKEN_CREATED_AUDIT_ACTION: &str = "auth.api_token.created";
pub const API_TOKEN_UPDATED_AUDIT_ACTION: &str = "auth.api_token.updated";
pub const API_TOKEN_REVOKED_AUDIT_ACTION: &str = "auth.api_token.revoked";

#[derive(Clone, Eq, PartialEq)]
pub struct ApiTokenSecret(Zeroizing<String>);

impl ApiTokenSecret {
    pub fn from_client_input(value: String) -> Result<Self, ApiTokenApplicationError> {
        validate_secret(&value)?;
        Ok(Self(Zeroizing::new(value)))
    }

    #[must_use]
    pub fn expose_once(&self) -> &str {
        self.0.as_str()
    }

    #[must_use]
    pub fn lookup_prefix(&self) -> &str {
        &self.0[..PREFIX_LENGTH]
    }

    #[must_use]
    pub const fn secret_entropy_bits(&self) -> usize {
        SECRET_BYTES * 8
    }
}

impl fmt::Debug for ApiTokenSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiTokenSecret([REDACTED])")
    }
}

fn validate_secret(value: &str) -> Result<(), ApiTokenApplicationError> {
    if value.len() != PREFIX_LENGTH + SECRET_BYTES * 2
        || !value.starts_with("takt_")
        || !value[5..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ApiTokenApplicationError::InvalidSecret);
    }
    Ok(())
}

pub trait TokenSecretGenerator: Send + Sync {
    fn generate(&self) -> Result<ApiTokenSecret, ApiTokenApplicationError>;
}

pub struct ApiTokenSecretGenerator;

impl TokenSecretGenerator for ApiTokenSecretGenerator {
    fn generate(&self) -> Result<ApiTokenSecret, ApiTokenApplicationError> {
        let mut bytes = [0_u8; PREFIX_HEX_BYTES + SECRET_BYTES];
        getrandom::fill(&mut bytes).map_err(|_| ApiTokenApplicationError::TokenGeneration)?;
        let encoded: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        ApiTokenSecret::from_client_input(format!("takt_{encoded}"))
    }
}

#[derive(Clone)]
pub struct ApiTokenHash(PasswordHash);

impl ApiTokenHash {
    pub fn from_persistence(value: String) -> Result<Self, ApiTokenApplicationError> {
        PasswordHash::from_persistence(value)
            .map(Self)
            .map_err(|_| ApiTokenApplicationError::InvalidHash)
    }

    #[must_use]
    pub fn expose_for_persistence(&self) -> &str {
        self.0.expose_for_persistence()
    }
}

impl fmt::Debug for ApiTokenHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiTokenHash([REDACTED])")
    }
}

pub struct ApiTokenHasher(PasswordHasher);

impl ApiTokenHasher {
    #[must_use]
    pub const fn new(config: Argon2idConfig) -> Self {
        Self(PasswordHasher::new(config))
    }

    pub fn hash(&self, secret: &ApiTokenSecret) -> Result<ApiTokenHash, ApiTokenApplicationError> {
        self.0
            .hash(secret.expose_once())
            .map(ApiTokenHash)
            .map_err(|_| ApiTokenApplicationError::Hashing)
    }

    pub fn verify(
        &self,
        secret: &ApiTokenSecret,
        hash: &ApiTokenHash,
    ) -> Result<bool, ApiTokenApplicationError> {
        self.0
            .verify(secret.expose_once(), &hash.0)
            .map_err(|_| ApiTokenApplicationError::Hashing)
    }
}

#[async_trait]
pub trait ApiTokenHashing: Send + Sync {
    async fn hash(&self, secret: &ApiTokenSecret)
    -> Result<ApiTokenHash, ApiTokenApplicationError>;
    async fn verify(
        &self,
        secret: &ApiTokenSecret,
        hash: &ApiTokenHash,
    ) -> Result<bool, ApiTokenApplicationError>;
}

#[derive(Clone, Debug)]
pub struct NewApiToken {
    pub id: ApiTokenId,
    pub organization_id: OrganizationId,
    pub project_id: Option<ProjectId>,
    pub name: String,
    pub kind: ApiTokenKind,
    pub token_prefix: ApiTokenPrefix,
    pub token_hash: ApiTokenHash,
    pub scopes: Vec<ApiTokenScope>,
    pub ip_networks: Vec<IpNetwork>,
    pub expires_at: Option<UtcTimestamp>,
    pub now: UtcTimestamp,
}

#[derive(Clone, Debug)]
pub struct CreateApiTokenPlan {
    pub token: NewApiToken,
    pub audit_event: NewAuditEvent,
}

#[derive(Clone, Debug)]
pub struct ApiTokenPatch {
    pub name: Option<String>,
    pub expires_at: Option<Option<UtcTimestamp>>,
    pub ip_networks: Option<Vec<IpNetwork>>,
}

#[derive(Clone, Debug)]
pub struct UpdateApiTokenPlan {
    pub id: ApiTokenId,
    pub expected_version: i64,
    pub patch: ApiTokenPatch,
    pub now: UtcTimestamp,
    pub audit_event: NewAuditEvent,
}

#[derive(Clone, Debug)]
pub struct RevokeApiTokenPlan {
    pub id: ApiTokenId,
    pub expected_version: i64,
    pub now: UtcTimestamp,
    pub audit_event: NewAuditEvent,
}

#[derive(Clone, Debug)]
pub struct StoredApiToken {
    pub token: ApiToken,
    pub token_hash: ApiTokenHash,
}

#[derive(Clone, Debug)]
pub struct ApiTokenListQuery {
    pub organization_id: OrganizationId,
    pub project_id: Option<ProjectId>,
    pub kind: Option<ApiTokenKind>,
    pub status: Option<ApiTokenStatus>,
    pub scope: Option<ApiTokenScope>,
    pub before: Option<(UtcTimestamp, ApiTokenId)>,
    pub limit: u16,
    pub now: UtcTimestamp,
}

#[async_trait]
pub trait ApiTokenStore: Send + Sync {
    async fn create_api_token(&self, plan: CreateApiTokenPlan)
    -> Result<ApiToken, RepositoryError>;
    async fn api_token_by_id(&self, id: ApiTokenId) -> Result<ApiToken, RepositoryError>;
    async fn api_token_by_prefix(
        &self,
        prefix: &ApiTokenPrefix,
    ) -> Result<StoredApiToken, RepositoryError>;
    async fn list_api_tokens(
        &self,
        query: ApiTokenListQuery,
    ) -> Result<Vec<ApiToken>, RepositoryError>;
}

#[async_trait]
pub trait ApiTokenLifecycleRepository: Send + Sync {
    async fn update_api_token(&self, plan: UpdateApiTokenPlan)
    -> Result<ApiToken, RepositoryError>;
    async fn revoke_api_token(&self, plan: RevokeApiTokenPlan)
    -> Result<ApiToken, RepositoryError>;
    async fn record_api_token_used(
        &self,
        id: ApiTokenId,
        now: UtcTimestamp,
    ) -> Result<(), RepositoryError>;
}

pub trait ApiTokenRepository: ApiTokenStore + ApiTokenLifecycleRepository {}

impl<T> ApiTokenRepository for T where T: ApiTokenStore + ApiTokenLifecycleRepository {}

pub fn authenticated_token_actor(
    stored: &ApiToken,
    now: UtcTimestamp,
    source: IpAddr,
) -> Result<TokenActor, ApiTokenApplicationError> {
    if !stored.authorizes_source(now, source) {
        return Err(ApiTokenApplicationError::AuthenticationFailed);
    }
    TokenActor::new(
        stored.id,
        stored.organization_id,
        stored.project_id,
        stored.scopes.clone(),
    )
    .map_err(|_| ApiTokenApplicationError::InvalidMetadata)
}

pub fn validate_new_api_token(token: &NewApiToken) -> Result<(), ApiTokenApplicationError> {
    if token.name.is_empty()
        || token.name.chars().count() > 120
        || token.scopes.is_empty()
        || token.scopes.len() > 100
        || token.ip_networks.len() > 32
        || token.expires_at.is_some_and(|expiry| expiry <= token.now)
        || token.token_prefix.as_str().is_empty()
    {
        return Err(ApiTokenApplicationError::InvalidMetadata);
    }
    let mut scopes = token.scopes.clone();
    scopes.sort();
    scopes.dedup();
    let mut networks = token.ip_networks.clone();
    networks.sort_by_key(ToString::to_string);
    networks.dedup();
    if scopes.len() != token.scopes.len() || networks.len() != token.ip_networks.len() {
        return Err(ApiTokenApplicationError::InvalidMetadata);
    }
    Ok(())
}

pub fn prefix_from_secret(
    secret: &ApiTokenSecret,
) -> Result<ApiTokenPrefix, ApiTokenApplicationError> {
    ApiTokenPrefix::from_str(secret.lookup_prefix())
        .map_err(|_| ApiTokenApplicationError::InvalidSecret)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenApplicationError {
    InvalidSecret,
    TokenGeneration,
    InvalidHash,
    Hashing,
    InvalidMetadata,
    AuthenticationFailed,
    PermissionDenied,
    Repository(RepositoryError),
}

impl fmt::Display for ApiTokenApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSecret | Self::AuthenticationFailed => "API token authentication failed",
            Self::TokenGeneration | Self::InvalidHash | Self::Hashing => {
                "API token security operation failed"
            }
            Self::InvalidMetadata => "API token metadata is invalid",
            Self::PermissionDenied => "API token permission denied",
            Self::Repository(_) => "API token repository operation failed",
        })
    }
}

impl Error for ApiTokenApplicationError {}

impl From<RepositoryError> for ApiTokenApplicationError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

#[derive(Clone, Debug)]
pub struct CreatedApiToken {
    pub token: ApiTokenSecret,
    pub api_token: ApiToken,
    pub audit_event: AuditEvent,
}
