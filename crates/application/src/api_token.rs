use std::{error::Error, fmt, net::IpAddr, str::FromStr};

use async_trait::async_trait;
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use takt_domain::{
    ApiTokenId, AuditActorType, AuditEvent, OrganizationId, ProjectId, ResourceId, UtcTimestamp,
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
const REPLAY_NONCE_BYTES: usize = 12;
const REPLAY_RETENTION_MICROS: i64 = 24 * 60 * 60 * 1_000_000;
const MAX_REPLAY_BYTES: usize = 64 * 1024;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenWriteMethod {
    Post,
    Patch,
    Delete,
}

impl ApiTokenWriteMethod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Post => "POST",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ApiTokenIdempotencyContext {
    actor_type: AuditActorType,
    actor_id: ResourceId,
    method: ApiTokenWriteMethod,
    path: String,
    key: String,
    request_hash: [u8; 32],
    created_at: UtcTimestamp,
    expires_at: UtcTimestamp,
}

impl ApiTokenIdempotencyContext {
    pub fn new(
        actor_type: AuditActorType,
        actor_id: ResourceId,
        method: ApiTokenWriteMethod,
        path: String,
        key: String,
        request_hash: [u8; 32],
        created_at: UtcTimestamp,
    ) -> Result<Self, ApiTokenApplicationError> {
        let key_characters = key.chars().count();
        let path_matches_method = match method {
            ApiTokenWriteMethod::Post => path == "/api/v1/api-tokens",
            ApiTokenWriteMethod::Patch | ApiTokenWriteMethod::Delete => {
                path.starts_with("/api/v1/api-tokens/")
            }
        };
        let expires_at = created_at
            .unix_micros()
            .checked_add(REPLAY_RETENTION_MICROS)
            .map(UtcTimestamp::from_unix_micros)
            .ok_or(ApiTokenApplicationError::InvalidMetadata)?;
        if !(8..=128).contains(&key_characters)
            || key.chars().any(char::is_control)
            || path.len() > 512
            || !path_matches_method
        {
            return Err(ApiTokenApplicationError::InvalidMetadata);
        }
        Ok(Self {
            actor_type,
            actor_id,
            method,
            path,
            key,
            request_hash,
            created_at,
            expires_at,
        })
    }

    #[must_use]
    pub const fn actor_type(&self) -> AuditActorType {
        self.actor_type
    }
    #[must_use]
    pub const fn actor_id(&self) -> ResourceId {
        self.actor_id
    }
    #[must_use]
    pub const fn method(&self) -> ApiTokenWriteMethod {
        self.method
    }
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }
    #[must_use]
    pub const fn request_hash(&self) -> &[u8; 32] {
        &self.request_hash
    }
    #[must_use]
    pub const fn created_at(&self) -> UtcTimestamp {
        self.created_at
    }
    #[must_use]
    pub const fn expires_at(&self) -> UtcTimestamp {
        self.expires_at
    }

    fn associated_data(&self) -> Vec<u8> {
        let actor_id = self.actor_id.to_string();
        let mut value = Vec::with_capacity(256);
        for part in [
            self.actor_type.as_str().as_bytes(),
            actor_id.as_bytes(),
            self.method.as_str().as_bytes(),
            self.path.as_bytes(),
            self.key.as_bytes(),
            self.request_hash.as_slice(),
        ] {
            value.extend_from_slice(&(part.len() as u64).to_be_bytes());
            value.extend_from_slice(part);
        }
        value
    }
}

impl fmt::Debug for ApiTokenIdempotencyContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiTokenIdempotencyContext")
            .field("actor_type", &self.actor_type)
            .field("method", &self.method)
            .field("path", &self.path)
            .field("created_at", &self.created_at)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct EncryptedApiTokenReplay {
    key_version: i32,
    nonce: [u8; REPLAY_NONCE_BYTES],
    ciphertext: Zeroizing<Vec<u8>>,
}

impl EncryptedApiTokenReplay {
    pub fn from_persistence(
        key_version: i32,
        nonce: Vec<u8>,
        ciphertext: Vec<u8>,
    ) -> Result<Self, ApiTokenApplicationError> {
        let nonce = nonce
            .try_into()
            .map_err(|_| ApiTokenApplicationError::ReplayEncryption)?;
        if key_version < 1 || ciphertext.len() < 16 || ciphertext.len() > MAX_REPLAY_BYTES + 16 {
            return Err(ApiTokenApplicationError::ReplayEncryption);
        }
        Ok(Self {
            key_version,
            nonce,
            ciphertext: Zeroizing::new(ciphertext),
        })
    }

    #[must_use]
    pub const fn key_version(&self) -> i32 {
        self.key_version
    }
    #[must_use]
    pub const fn nonce(&self) -> &[u8; REPLAY_NONCE_BYTES] {
        &self.nonce
    }
    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }
}

impl fmt::Debug for EncryptedApiTokenReplay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EncryptedApiTokenReplay([REDACTED])")
    }
}

pub struct ApiTokenReplayCipher {
    key_version: i32,
    key: Zeroizing<[u8; 32]>,
}

impl ApiTokenReplayCipher {
    pub fn new(key_version: i32, key: [u8; 32]) -> Result<Self, ApiTokenApplicationError> {
        if key_version < 1 {
            return Err(ApiTokenApplicationError::ReplayEncryption);
        }
        Ok(Self {
            key_version,
            key: Zeroizing::new(key),
        })
    }

    pub fn encrypt(
        &self,
        context: &ApiTokenIdempotencyContext,
        plaintext: &[u8],
    ) -> Result<EncryptedApiTokenReplay, ApiTokenApplicationError> {
        if plaintext.is_empty() || plaintext.len() > MAX_REPLAY_BYTES {
            return Err(ApiTokenApplicationError::ReplayEncryption);
        }
        let mut nonce = [0_u8; REPLAY_NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| ApiTokenApplicationError::ReplayEncryption)?;
        let cipher = ChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .map_err(|_| ApiTokenApplicationError::ReplayEncryption)?;
        let ciphertext = cipher
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: &context.associated_data(),
                },
            )
            .map_err(|_| ApiTokenApplicationError::ReplayEncryption)?;
        Ok(EncryptedApiTokenReplay {
            key_version: self.key_version,
            nonce,
            ciphertext: Zeroizing::new(ciphertext),
        })
    }

    pub fn decrypt(
        &self,
        context: &ApiTokenIdempotencyContext,
        encrypted: &EncryptedApiTokenReplay,
    ) -> Result<Zeroizing<Vec<u8>>, ApiTokenApplicationError> {
        if encrypted.key_version != self.key_version {
            return Err(ApiTokenApplicationError::ReplayEncryption);
        }
        ChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .map_err(|_| ApiTokenApplicationError::ReplayEncryption)?
            .decrypt(
                &Nonce::from(encrypted.nonce),
                Payload {
                    msg: encrypted.ciphertext(),
                    aad: &context.associated_data(),
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| ApiTokenApplicationError::ReplayEncryption)
    }
}

impl fmt::Debug for ApiTokenReplayCipher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiTokenReplayCipher([REDACTED])")
    }
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
    ReplayEncryption,
    Repository(RepositoryError),
}

impl fmt::Display for ApiTokenApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSecret | Self::AuthenticationFailed => "API token authentication failed",
            Self::TokenGeneration | Self::InvalidHash | Self::Hashing | Self::ReplayEncryption => {
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
