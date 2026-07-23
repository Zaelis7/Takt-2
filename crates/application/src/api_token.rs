use std::{error::Error, fmt, net::IpAddr, str::FromStr};

use async_trait::async_trait;
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use takt_domain::{
    ApiTokenAuditMetadata, AuditActorId, AuditActorType, AuditEvent, AuditEventId, AuditMetadata,
    BootstrapAuditMetadata, MembershipId, OrganizationId, UserId,
    api_token::{ApiToken, ApiTokenPrefix, IpNetwork, TokenActor},
};
pub use takt_domain::{
    ApiTokenId, OperationId, ProjectId, ResourceId, UtcTimestamp,
    api_token::{ApiTokenKind, ApiTokenScope, ApiTokenStatus},
};
use zeroize::Zeroizing;

use crate::{
    ApplicationError, Argon2idConfig, BrowserSessionReadAuthentication, Clock, IdGenerator,
    NewAuditEvent, PasswordHash, PasswordHasher, RepositoryError,
};

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
        let value = Zeroizing::new(value);
        validate_secret(&value)?;
        Ok(Self(value))
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

#[derive(Clone, Debug)]
pub struct CreateApiTokenIdempotencyPlan {
    pub create: CreateApiTokenPlan,
    pub context: ApiTokenIdempotencyContext,
    pub encrypted_replay: EncryptedApiTokenReplay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredApiTokenCreateReplay {
    pub api_token_id: ApiTokenId,
    pub result_version: i64,
    pub encrypted_replay: EncryptedApiTokenReplay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApiTokenCreateIdempotencyResult {
    Created {
        api_token: Box<ApiToken>,
        replay: StoredApiTokenCreateReplay,
    },
    Replay(StoredApiTokenCreateReplay),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenCreateIdempotencyError {
    KeyReused,
    Repository(RepositoryError),
}

impl fmt::Display for ApiTokenCreateIdempotencyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::KeyReused => "API token idempotency key was reused",
            Self::Repository(_) => "API token idempotency repository operation failed",
        })
    }
}

impl Error for ApiTokenCreateIdempotencyError {}

impl From<RepositoryError> for ApiTokenCreateIdempotencyError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

#[async_trait]
pub trait ApiTokenCreateIdempotencyRepository: Send + Sync {
    async fn create_api_token_idempotent(
        &self,
        plan: CreateApiTokenIdempotencyPlan,
    ) -> Result<ApiTokenCreateIdempotencyResult, ApiTokenCreateIdempotencyError>;

    async fn purge_expired_api_token_idempotency(
        &self,
        now: UtcTimestamp,
        limit: u16,
    ) -> Result<u64, RepositoryError>;
}

#[derive(Clone, Debug)]
pub struct UpdateApiTokenIdempotencyPlan {
    pub update: UpdateApiTokenPlan,
    pub context: ApiTokenIdempotencyContext,
}

#[derive(Clone, Debug)]
pub struct RevokeApiTokenIdempotencyPlan {
    pub revoke: RevokeApiTokenPlan,
    pub context: ApiTokenIdempotencyContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredApiTokenMutationResult {
    pub api_token_id: ApiTokenId,
    pub result_version: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApiTokenMutationIdempotencyResult {
    Mutated {
        api_token: Box<ApiToken>,
        result: StoredApiTokenMutationResult,
    },
    Replay(StoredApiTokenMutationResult),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenMutationIdempotencyError {
    KeyReused,
    Repository(RepositoryError),
}

impl fmt::Display for ApiTokenMutationIdempotencyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::KeyReused => "API token idempotency key was reused",
            Self::Repository(_) => "API token mutation idempotency operation failed",
        })
    }
}

impl Error for ApiTokenMutationIdempotencyError {}

impl From<RepositoryError> for ApiTokenMutationIdempotencyError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

#[async_trait]
pub trait ApiTokenMutationIdempotencyRepository: Send + Sync {
    async fn update_api_token_idempotent(
        &self,
        plan: UpdateApiTokenIdempotencyPlan,
    ) -> Result<ApiTokenMutationIdempotencyResult, ApiTokenMutationIdempotencyError>;

    async fn revoke_api_token_idempotent(
        &self,
        plan: RevokeApiTokenIdempotencyPlan,
    ) -> Result<ApiTokenMutationIdempotencyResult, ApiTokenMutationIdempotencyError>;
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

/// Secret-free authorization context for API-token metadata reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiTokenReadActor {
    organization_id: OrganizationId,
    project_scope: Option<ProjectId>,
}

impl ApiTokenReadActor {
    /// Creates the organization-wide read actor used by the local 0.1 administrator session.
    #[must_use]
    pub const fn from_browser_session(authentication: &BrowserSessionReadAuthentication) -> Self {
        Self {
            organization_id: authentication.organization_id,
            project_scope: None,
        }
    }

    /// Creates a project- or organization-scoped actor from an exact `api_tokens:read` token.
    pub fn from_token_actor(actor: &TokenActor) -> Result<Self, ApiTokenApplicationError> {
        let required_scope = ApiTokenScope::from_str("api_tokens:read")
            .map_err(|_| ApiTokenApplicationError::InvalidMetadata)?;
        if !actor.allows(&required_scope) {
            return Err(ApiTokenApplicationError::PermissionDenied);
        }
        Ok(Self {
            organization_id: actor.organization_id(),
            project_scope: actor.project_id(),
        })
    }

    /// Returns the organization that must be applied to every repository query.
    #[must_use]
    pub const fn organization_id(&self) -> OrganizationId {
        self.organization_id
    }

    fn authorize(
        &self,
        organization_id: OrganizationId,
        project_id: Option<ProjectId>,
    ) -> Result<(), ApiTokenApplicationError> {
        if self.organization_id != organization_id
            || self
                .project_scope
                .is_some_and(|scope| project_id != Some(scope))
        {
            return Err(ApiTokenApplicationError::PermissionDenied);
        }
        Ok(())
    }
}

/// Redacted token metadata and its status at one application-controlled instant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiTokenReadResource {
    pub token: ApiToken,
    pub status: ApiTokenStatus,
}

/// Stable API-token page plus an exact indication that another item exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiTokenReadPage {
    pub items: Vec<ApiTokenReadResource>,
    pub has_more: bool,
}

/// Context-checking List/Get use cases over the engine-neutral token store.
pub struct ApiTokenReadService<'a, R, C> {
    repository: &'a R,
    clock: &'a C,
}

impl<'a, R, C> ApiTokenReadService<'a, R, C>
where
    R: ApiTokenStore,
    C: Clock,
{
    /// Builds a read service with injected persistence and clock boundaries.
    #[must_use]
    pub const fn new(repository: &'a R, clock: &'a C) -> Self {
        Self { repository, clock }
    }

    /// Lists one stable page after enforcing actor, organization and project scope.
    pub async fn list(
        &self,
        actor: &ApiTokenReadActor,
        mut query: ApiTokenListQuery,
    ) -> Result<ApiTokenReadPage, ApiTokenApplicationError> {
        actor.authorize(query.organization_id, query.project_id)?;
        let query_organization_id = query.organization_id;
        let query_project_id = query.project_id;
        if !(1..=200).contains(&query.limit) {
            return Err(ApiTokenApplicationError::InvalidMetadata);
        }
        let now = self.clock.now().map_err(map_application_error)?;
        query.now = now;
        let tokens = self.repository.list_api_tokens(query.clone()).await?;
        if tokens.len() > usize::from(query.limit) {
            return Err(ApiTokenApplicationError::Repository(
                RepositoryError::UnknownInfrastructure,
            ));
        }
        ensure_read_context(actor, query_organization_id, query_project_id, &tokens)?;

        let has_more = if tokens.len() == usize::from(query.limit) {
            let last = tokens.last().ok_or(ApiTokenApplicationError::Repository(
                RepositoryError::UnknownInfrastructure,
            ))?;
            let mut lookahead = query;
            lookahead.before = Some((last.created_at, last.id));
            lookahead.limit = 1;
            let following = self.repository.list_api_tokens(lookahead).await?;
            if following.len() > 1 {
                return Err(ApiTokenApplicationError::Repository(
                    RepositoryError::UnknownInfrastructure,
                ));
            }
            ensure_read_context(actor, query_organization_id, query_project_id, &following)?;
            !following.is_empty()
        } else {
            false
        };
        Ok(ApiTokenReadPage {
            items: tokens
                .into_iter()
                .map(|token| ApiTokenReadResource {
                    status: token.status(now),
                    token,
                })
                .collect(),
            has_more,
        })
    }

    /// Reads one redacted token projection after checking its persisted context.
    pub async fn get(
        &self,
        actor: &ApiTokenReadActor,
        id: ApiTokenId,
    ) -> Result<ApiTokenReadResource, ApiTokenApplicationError> {
        let token = self.repository.api_token_by_id(id).await?;
        actor.authorize(token.organization_id, token.project_id)?;
        let now = self.clock.now().map_err(map_application_error)?;
        Ok(ApiTokenReadResource {
            status: token.status(now),
            token,
        })
    }
}

fn ensure_read_context(
    actor: &ApiTokenReadActor,
    organization_id: OrganizationId,
    project_id: Option<ProjectId>,
    tokens: &[ApiToken],
) -> Result<(), ApiTokenApplicationError> {
    if tokens.iter().any(|token| {
        token.organization_id != organization_id
            || project_id.is_some() && token.project_id != project_id
            || actor
                .authorize(token.organization_id, token.project_id)
                .is_err()
    }) {
        return Err(ApiTokenApplicationError::PermissionDenied);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ApiTokenManagementPermission {
    Read,
    Write,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiTokenManagementActor {
    organization_id: OrganizationId,
    project_scope: Option<ProjectId>,
    audit_project_id: ProjectId,
    user_id: UserId,
    membership_id: MembershipId,
    permissions: Vec<ApiTokenManagementPermission>,
}

impl ApiTokenManagementActor {
    pub fn new(
        organization_id: OrganizationId,
        project_scope: Option<ProjectId>,
        audit_project_id: ProjectId,
        user_id: UserId,
        membership_id: MembershipId,
        mut permissions: Vec<ApiTokenManagementPermission>,
    ) -> Result<Self, ApiTokenApplicationError> {
        permissions.sort_unstable();
        permissions.dedup();
        if permissions.is_empty() || project_scope.is_some_and(|scope| scope != audit_project_id) {
            return Err(ApiTokenApplicationError::InvalidMetadata);
        }
        Ok(Self {
            organization_id,
            project_scope,
            audit_project_id,
            user_id,
            membership_id,
            permissions,
        })
    }

    fn authorize(
        &self,
        permission: ApiTokenManagementPermission,
        organization_id: OrganizationId,
        project_id: Option<ProjectId>,
    ) -> Result<(), ApiTokenApplicationError> {
        if self.organization_id != organization_id
            || self
                .project_scope
                .is_some_and(|scope| Some(scope) != project_id)
            || self.permissions.binary_search(&permission).is_err()
        {
            return Err(ApiTokenApplicationError::PermissionDenied);
        }
        Ok(())
    }
}

/// Authenticated identity accepted by API-token write use cases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApiTokenWriteActor {
    Browser(ApiTokenManagementActor),
    Bearer(TokenActor),
}

impl ApiTokenWriteActor {
    /// Wraps an already permission-bearing browser management identity.
    #[must_use]
    pub const fn from_browser_management(actor: ApiTokenManagementActor) -> Self {
        Self::Browser(actor)
    }

    /// Accepts a Bearer actor only when it has the exact `api_tokens:write` scope.
    pub fn from_token_actor(actor: &TokenActor) -> Result<Self, ApiTokenApplicationError> {
        let required = ApiTokenScope::from_str("api_tokens:write")
            .map_err(|_| ApiTokenApplicationError::InvalidMetadata)?;
        if !actor.allows(&required) {
            return Err(ApiTokenApplicationError::PermissionDenied);
        }
        Ok(Self::Bearer(actor.clone()))
    }

    fn authorize(
        &self,
        organization_id: OrganizationId,
        project_id: Option<ProjectId>,
    ) -> Result<(), ApiTokenApplicationError> {
        match self {
            Self::Browser(actor) => actor.authorize(
                ApiTokenManagementPermission::Write,
                organization_id,
                project_id,
            ),
            Self::Bearer(actor) => {
                let required = ApiTokenScope::from_str("api_tokens:write")
                    .map_err(|_| ApiTokenApplicationError::InvalidMetadata)?;
                authorize_token_actor(actor, organization_id, project_id, &required)
            }
        }
    }

    fn idempotency_identity(
        &self,
    ) -> Result<(AuditActorType, ResourceId), ApiTokenApplicationError> {
        let (actor_type, actor_id) = match self {
            Self::Browser(actor) => (AuditActorType::System, actor.user_id.as_uuid()),
            Self::Bearer(actor) => (AuditActorType::ApiToken, actor.token_id().as_uuid()),
        };
        ResourceId::from_uuid(actor_id)
            .map(|actor_id| (actor_type, actor_id))
            .map_err(|_| ApiTokenApplicationError::InvalidMetadata)
    }

    fn audit_identity(
        &self,
        organization_id: OrganizationId,
        project_id: Option<ProjectId>,
    ) -> (AuditActorType, AuditActorId, AuditMetadata) {
        match self {
            Self::Browser(actor) => (
                AuditActorType::System,
                AuditActorId::User(actor.user_id),
                AuditMetadata::LocalIdentity(BootstrapAuditMetadata {
                    organization_id,
                    project_id: actor.audit_project_id,
                    user_id: actor.user_id,
                    membership_id: actor.membership_id,
                }),
            ),
            Self::Bearer(actor) => (
                AuditActorType::ApiToken,
                AuditActorId::ApiToken(actor.token_id()),
                AuditMetadata::ApiToken(ApiTokenAuditMetadata {
                    organization_id,
                    project_id,
                    api_token_id: actor.token_id(),
                }),
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ApiTokenCreateCommand {
    pub organization_id: OrganizationId,
    pub project_id: Option<ProjectId>,
    pub name: String,
    pub kind: ApiTokenKind,
    pub scopes: Vec<ApiTokenScope>,
    pub ip_networks: Vec<IpNetwork>,
    pub expires_at: Option<UtcTimestamp>,
    pub request_id: OperationId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApiTokenTarget {
    pub id: ApiTokenId,
    pub organization_id: OrganizationId,
    pub project_id: Option<ProjectId>,
}

#[derive(Clone, Debug)]
pub struct ApiTokenUpdateCommand {
    pub target: ApiTokenTarget,
    pub expected_version: i64,
    pub patch: ApiTokenPatch,
    pub request_id: OperationId,
}

#[derive(Clone, Debug)]
pub struct ApiTokenRevokeCommand {
    pub target: ApiTokenTarget,
    pub expected_version: i64,
    pub request_id: OperationId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiTokenIdempotencyInput {
    pub key: String,
    pub request_hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct ApiTokenIdempotentCreateCommand {
    pub create: ApiTokenCreateCommand,
    pub idempotency: ApiTokenIdempotencyInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiTokenIdempotentCreateOutput {
    pub token: ApiTokenSecret,
    pub api_token: ApiToken,
    pub replayed: bool,
}

pub struct ApiTokenManagementService<'a, R, H, C, I, G> {
    repository: &'a R,
    hashing: &'a H,
    clock: &'a C,
    ids: &'a I,
    secrets: &'a G,
}

impl<'a, R, H, C, I, G> ApiTokenManagementService<'a, R, H, C, I, G>
where
    R: ApiTokenRepository,
    H: ApiTokenHashing,
    C: Clock,
    I: IdGenerator,
    G: TokenSecretGenerator,
{
    #[must_use]
    pub const fn new(
        repository: &'a R,
        hashing: &'a H,
        clock: &'a C,
        ids: &'a I,
        secrets: &'a G,
    ) -> Self {
        Self {
            repository,
            hashing,
            clock,
            ids,
            secrets,
        }
    }

    pub async fn create(
        &self,
        actor: &ApiTokenManagementActor,
        command: ApiTokenCreateCommand,
    ) -> Result<CreatedApiToken, ApiTokenApplicationError> {
        actor.authorize(
            ApiTokenManagementPermission::Write,
            command.organization_id,
            command.project_id,
        )?;
        let now = self.now()?;
        let secret = self.secrets.generate()?;
        let token = NewApiToken {
            id: ApiTokenId::from_resource_id(self.next_id()?),
            organization_id: command.organization_id,
            project_id: command.project_id,
            name: command.name,
            kind: command.kind,
            token_prefix: prefix_from_secret(&secret)?,
            token_hash: self.hashing.hash(&secret).await?,
            scopes: command.scopes,
            ip_networks: command.ip_networks,
            expires_at: command.expires_at,
            now,
        };
        validate_new_api_token(&token)?;
        let audit_event = self.audit(
            actor,
            token.id,
            token.organization_id,
            token.project_id,
            command.request_id,
            API_TOKEN_CREATED_AUDIT_ACTION,
            now,
        )?;
        let api_token = self
            .repository
            .create_api_token(CreateApiTokenPlan {
                token,
                audit_event: NewAuditEvent {
                    event: audit_event.clone(),
                },
            })
            .await?;
        Ok(CreatedApiToken {
            token: secret,
            api_token,
            audit_event,
        })
    }

    pub async fn list(
        &self,
        actor: &ApiTokenManagementActor,
        mut query: ApiTokenListQuery,
    ) -> Result<Vec<ApiToken>, ApiTokenApplicationError> {
        actor.authorize(
            ApiTokenManagementPermission::Read,
            query.organization_id,
            query.project_id,
        )?;
        query.now = self.now()?;
        let tokens = self.repository.list_api_tokens(query.clone()).await?;
        if tokens.iter().any(|token| {
            token.organization_id != query.organization_id
                || query.project_id.is_some() && token.project_id != query.project_id
        }) {
            return Err(ApiTokenApplicationError::PermissionDenied);
        }
        Ok(tokens)
    }

    pub async fn get(
        &self,
        actor: &ApiTokenManagementActor,
        target: ApiTokenTarget,
    ) -> Result<ApiToken, ApiTokenApplicationError> {
        actor.authorize(
            ApiTokenManagementPermission::Read,
            target.organization_id,
            target.project_id,
        )?;
        let token = self.repository.api_token_by_id(target.id).await?;
        ensure_target(&token, target)?;
        Ok(token)
    }

    pub async fn update(
        &self,
        actor: &ApiTokenManagementActor,
        command: ApiTokenUpdateCommand,
    ) -> Result<ApiToken, ApiTokenApplicationError> {
        self.prepare_mutation(actor, command.target).await?;
        let now = self.now()?;
        let audit_event = self.audit(
            actor,
            command.target.id,
            command.target.organization_id,
            command.target.project_id,
            command.request_id,
            API_TOKEN_UPDATED_AUDIT_ACTION,
            now,
        )?;
        self.repository
            .update_api_token(UpdateApiTokenPlan {
                id: command.target.id,
                expected_version: command.expected_version,
                patch: command.patch,
                now,
                audit_event: NewAuditEvent { event: audit_event },
            })
            .await
            .map_err(Into::into)
    }

    pub async fn revoke(
        &self,
        actor: &ApiTokenManagementActor,
        command: ApiTokenRevokeCommand,
    ) -> Result<ApiToken, ApiTokenApplicationError> {
        self.prepare_mutation(actor, command.target).await?;
        let now = self.now()?;
        let audit_event = self.audit(
            actor,
            command.target.id,
            command.target.organization_id,
            command.target.project_id,
            command.request_id,
            API_TOKEN_REVOKED_AUDIT_ACTION,
            now,
        )?;
        self.repository
            .revoke_api_token(RevokeApiTokenPlan {
                id: command.target.id,
                expected_version: command.expected_version,
                now,
                audit_event: NewAuditEvent { event: audit_event },
            })
            .await
            .map_err(Into::into)
    }

    async fn prepare_mutation(
        &self,
        actor: &ApiTokenManagementActor,
        target: ApiTokenTarget,
    ) -> Result<(), ApiTokenApplicationError> {
        actor.authorize(
            ApiTokenManagementPermission::Write,
            target.organization_id,
            target.project_id,
        )?;
        let token = self.repository.api_token_by_id(target.id).await?;
        ensure_target(&token, target)
    }

    fn now(&self) -> Result<UtcTimestamp, ApiTokenApplicationError> {
        self.clock.now().map_err(map_application_error)
    }

    fn next_id(&self) -> Result<ResourceId, ApiTokenApplicationError> {
        self.ids.next_resource_id().map_err(map_application_error)
    }

    #[allow(clippy::too_many_arguments)]
    fn audit(
        &self,
        actor: &ApiTokenManagementActor,
        token_id: ApiTokenId,
        organization_id: OrganizationId,
        project_id: Option<ProjectId>,
        request_id: OperationId,
        action: &str,
        now: UtcTimestamp,
    ) -> Result<AuditEvent, ApiTokenApplicationError> {
        Ok(AuditEvent {
            id: AuditEventId::from_resource_id(self.next_id()?),
            organization_id,
            project_id,
            actor_type: AuditActorType::System,
            actor_id: Some(AuditActorId::User(actor.user_id)),
            action: action.to_owned(),
            resource_type: "api_token".to_owned(),
            resource_id: ResourceId::from_uuid(token_id.as_uuid())
                .map_err(|_| ApiTokenApplicationError::IdGeneration)?,
            request_id,
            metadata: AuditMetadata::LocalIdentity(BootstrapAuditMetadata {
                organization_id,
                project_id: actor.audit_project_id,
                user_id: actor.user_id,
                membership_id: actor.membership_id,
            }),
            occurred_at: now,
        })
    }
}

const CREATE_REPLAY_MAGIC: &[u8; 4] = b"TKR1";

/// Actor- and context-bound idempotent API-token write orchestration.
pub struct ApiTokenIdempotentWriteService<'a, R, H, C, I, G> {
    repository: &'a R,
    hashing: &'a H,
    clock: &'a C,
    ids: &'a I,
    secrets: &'a G,
    replay_cipher: &'a ApiTokenReplayCipher,
}

impl<'a, R, H, C, I, G> ApiTokenIdempotentWriteService<'a, R, H, C, I, G>
where
    R: ApiTokenStore + ApiTokenCreateIdempotencyRepository,
    H: ApiTokenHashing,
    C: Clock,
    I: IdGenerator,
    G: TokenSecretGenerator,
{
    #[must_use]
    pub const fn new(
        repository: &'a R,
        hashing: &'a H,
        clock: &'a C,
        ids: &'a I,
        secrets: &'a G,
        replay_cipher: &'a ApiTokenReplayCipher,
    ) -> Self {
        Self {
            repository,
            hashing,
            clock,
            ids,
            secrets,
            replay_cipher,
        }
    }

    pub async fn create(
        &self,
        actor: &ApiTokenWriteActor,
        command: ApiTokenIdempotentCreateCommand,
    ) -> Result<ApiTokenIdempotentCreateOutput, ApiTokenApplicationError> {
        let ApiTokenIdempotentCreateCommand {
            create,
            idempotency,
        } = command;
        actor.authorize(create.organization_id, create.project_id)?;
        let now = self.now()?;
        let context = self.idempotency_context(
            actor,
            ApiTokenWriteMethod::Post,
            "/api/v1/api-tokens".to_owned(),
            idempotency,
            now,
        )?;
        let secret = self.secrets.generate()?;
        let token = NewApiToken {
            id: ApiTokenId::from_resource_id(self.next_id()?),
            organization_id: create.organization_id,
            project_id: create.project_id,
            name: create.name.clone(),
            kind: create.kind,
            token_prefix: prefix_from_secret(&secret)?,
            token_hash: self.hashing.hash(&secret).await?,
            scopes: create.scopes.clone(),
            ip_networks: create.ip_networks.clone(),
            expires_at: create.expires_at,
            now,
        };
        let expected = token_projection(&token);
        let audit_event = self.audit(
            actor,
            token.id,
            token.organization_id,
            token.project_id,
            create.request_id,
            API_TOKEN_CREATED_AUDIT_ACTION,
            now,
        )?;
        let replay_plaintext = encode_create_replay(&secret, now);
        let encrypted_replay = self.replay_cipher.encrypt(&context, &replay_plaintext)?;
        let result = self
            .repository
            .create_api_token_idempotent(CreateApiTokenIdempotencyPlan {
                create: CreateApiTokenPlan {
                    token,
                    audit_event: NewAuditEvent { event: audit_event },
                },
                context: context.clone(),
                encrypted_replay,
            })
            .await
            .map_err(map_create_idempotency_error)?;
        match result {
            ApiTokenCreateIdempotencyResult::Created { api_token, replay } => {
                if *api_token != expected
                    || replay.api_token_id != api_token.id
                    || replay.result_version != api_token.version
                {
                    return Err(invalid_repository_result());
                }
                Ok(ApiTokenIdempotentCreateOutput {
                    token: secret,
                    api_token: *api_token,
                    replayed: false,
                })
            }
            ApiTokenCreateIdempotencyResult::Replay(replay) => {
                let (token, created_at) =
                    decode_create_replay(self.replay_cipher, &context, &replay.encrypted_replay)?;
                let api_token = replay_token_projection(
                    &create,
                    replay.api_token_id,
                    replay.result_version,
                    &token,
                    created_at,
                )?;
                Ok(ApiTokenIdempotentCreateOutput {
                    token,
                    api_token,
                    replayed: true,
                })
            }
        }
    }

    fn idempotency_context(
        &self,
        actor: &ApiTokenWriteActor,
        method: ApiTokenWriteMethod,
        path: String,
        input: ApiTokenIdempotencyInput,
        now: UtcTimestamp,
    ) -> Result<ApiTokenIdempotencyContext, ApiTokenApplicationError> {
        let (actor_type, actor_id) = actor.idempotency_identity()?;
        ApiTokenIdempotencyContext::new(
            actor_type,
            actor_id,
            method,
            path,
            input.key,
            input.request_hash,
            now,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn audit(
        &self,
        actor: &ApiTokenWriteActor,
        token_id: ApiTokenId,
        organization_id: OrganizationId,
        project_id: Option<ProjectId>,
        request_id: OperationId,
        action: &str,
        now: UtcTimestamp,
    ) -> Result<AuditEvent, ApiTokenApplicationError> {
        let (actor_type, actor_id, metadata) = actor.audit_identity(organization_id, project_id);
        Ok(AuditEvent {
            id: AuditEventId::from_resource_id(self.next_id()?),
            organization_id,
            project_id,
            actor_type,
            actor_id: Some(actor_id),
            action: action.to_owned(),
            resource_type: "api_token".to_owned(),
            resource_id: ResourceId::from_uuid(token_id.as_uuid())
                .map_err(|_| ApiTokenApplicationError::IdGeneration)?,
            request_id,
            metadata,
            occurred_at: now,
        })
    }

    fn now(&self) -> Result<UtcTimestamp, ApiTokenApplicationError> {
        self.clock.now().map_err(map_application_error)
    }

    fn next_id(&self) -> Result<ResourceId, ApiTokenApplicationError> {
        self.ids.next_resource_id().map_err(map_application_error)
    }
}

fn token_projection(value: &NewApiToken) -> ApiToken {
    ApiToken {
        id: value.id,
        organization_id: value.organization_id,
        project_id: value.project_id,
        name: value.name.clone(),
        kind: value.kind,
        token_prefix: value.token_prefix.clone(),
        scopes: value.scopes.clone(),
        ip_networks: value.ip_networks.clone(),
        expires_at: value.expires_at,
        last_used_at: None,
        revoked_at: None,
        created_at: value.now,
        updated_at: value.now,
        version: 1,
    }
}

fn encode_create_replay(secret: &ApiTokenSecret, created_at: UtcTimestamp) -> Zeroizing<Vec<u8>> {
    let mut plaintext = Zeroizing::new(Vec::with_capacity(12 + secret.expose_once().len()));
    plaintext.extend_from_slice(CREATE_REPLAY_MAGIC);
    plaintext.extend_from_slice(&created_at.unix_micros().to_be_bytes());
    plaintext.extend_from_slice(secret.expose_once().as_bytes());
    plaintext
}

fn decode_create_replay(
    cipher: &ApiTokenReplayCipher,
    context: &ApiTokenIdempotencyContext,
    encrypted: &EncryptedApiTokenReplay,
) -> Result<(ApiTokenSecret, UtcTimestamp), ApiTokenApplicationError> {
    let plaintext = cipher.decrypt(context, encrypted)?;
    if plaintext.len() != 12 + PREFIX_LENGTH + SECRET_BYTES * 2
        || &plaintext[..4] != CREATE_REPLAY_MAGIC
    {
        return Err(ApiTokenApplicationError::ReplayEncryption);
    }
    let mut timestamp = [0_u8; 8];
    timestamp.copy_from_slice(&plaintext[4..12]);
    let secret = std::str::from_utf8(&plaintext[12..])
        .map_err(|_| ApiTokenApplicationError::ReplayEncryption)
        .and_then(|value| ApiTokenSecret::from_client_input(value.to_owned()))?;
    Ok((
        secret,
        UtcTimestamp::from_unix_micros(i64::from_be_bytes(timestamp)),
    ))
}

fn replay_token_projection(
    command: &ApiTokenCreateCommand,
    id: ApiTokenId,
    version: i64,
    secret: &ApiTokenSecret,
    created_at: UtcTimestamp,
) -> Result<ApiToken, ApiTokenApplicationError> {
    if version != 1 {
        return Err(invalid_repository_result());
    }
    Ok(ApiToken {
        id,
        organization_id: command.organization_id,
        project_id: command.project_id,
        name: command.name.clone(),
        kind: command.kind,
        token_prefix: prefix_from_secret(secret)?,
        scopes: command.scopes.clone(),
        ip_networks: command.ip_networks.clone(),
        expires_at: command.expires_at,
        last_used_at: None,
        revoked_at: None,
        created_at,
        updated_at: created_at,
        version,
    })
}

fn map_create_idempotency_error(error: ApiTokenCreateIdempotencyError) -> ApiTokenApplicationError {
    match error {
        ApiTokenCreateIdempotencyError::KeyReused => ApiTokenApplicationError::IdempotencyKeyReused,
        ApiTokenCreateIdempotencyError::Repository(error) => {
            ApiTokenApplicationError::Repository(error)
        }
    }
}

const fn invalid_repository_result() -> ApiTokenApplicationError {
    ApiTokenApplicationError::Repository(RepositoryError::UnknownInfrastructure)
}

fn ensure_target(token: &ApiToken, target: ApiTokenTarget) -> Result<(), ApiTokenApplicationError> {
    if token.id == target.id
        && token.organization_id == target.organization_id
        && token.project_id == target.project_id
    {
        Ok(())
    } else {
        Err(ApiTokenApplicationError::PermissionDenied)
    }
}

fn map_application_error(error: ApplicationError) -> ApiTokenApplicationError {
    match error {
        ApplicationError::Clock => ApiTokenApplicationError::Clock,
        ApplicationError::IdGeneration => ApiTokenApplicationError::IdGeneration,
        ApplicationError::Repository(error) => ApiTokenApplicationError::Repository(error),
        ApplicationError::Validation(_) | ApplicationError::Conflict => {
            ApiTokenApplicationError::InvalidMetadata
        }
    }
}

pub struct ApiTokenBearerAuthenticationService<'a, R, H, C> {
    repository: &'a R,
    hashing: &'a H,
    clock: &'a C,
}

impl<'a, R, H, C> ApiTokenBearerAuthenticationService<'a, R, H, C>
where
    R: ApiTokenRepository,
    H: ApiTokenHashing,
    C: Clock,
{
    #[must_use]
    pub const fn new(repository: &'a R, hashing: &'a H, clock: &'a C) -> Self {
        Self {
            repository,
            hashing,
            clock,
        }
    }

    pub async fn authenticate(
        &self,
        bearer_token: &str,
        source: IpAddr,
    ) -> Result<TokenActor, ApiTokenApplicationError> {
        let secret = ApiTokenSecret::from_client_input(bearer_token.to_owned())
            .map_err(|_| ApiTokenApplicationError::AuthenticationFailed)?;
        let prefix = prefix_from_secret(&secret)
            .map_err(|_| ApiTokenApplicationError::AuthenticationFailed)?;
        let stored = self
            .repository
            .api_token_by_prefix(&prefix)
            .await
            .map_err(map_bearer_repository_error)?;
        if stored.token.token_prefix != prefix
            || !self.hashing.verify(&secret, &stored.token_hash).await?
        {
            return Err(ApiTokenApplicationError::AuthenticationFailed);
        }
        let now = self.clock.now().map_err(map_application_error)?;
        let actor = authenticated_token_actor(&stored.token, now, source)?;
        self.repository
            .record_api_token_used(stored.token.id, now)
            .await
            .map_err(map_bearer_repository_error)?;
        Ok(actor)
    }
}

pub fn authorize_token_actor(
    actor: &TokenActor,
    organization_id: OrganizationId,
    project_id: Option<ProjectId>,
    required_scope: &ApiTokenScope,
) -> Result<(), ApiTokenApplicationError> {
    let project_allowed = match actor.project_id() {
        Some(scope) => project_id == Some(scope),
        None => true,
    };
    if actor.organization_id() == organization_id && project_allowed && actor.allows(required_scope)
    {
        Ok(())
    } else {
        Err(ApiTokenApplicationError::PermissionDenied)
    }
}

fn map_bearer_repository_error(error: RepositoryError) -> ApiTokenApplicationError {
    match error {
        RepositoryError::NotFound | RepositoryError::VersionConflict => {
            ApiTokenApplicationError::AuthenticationFailed
        }
        other => ApiTokenApplicationError::Repository(other),
    }
}

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
    IdempotencyKeyReused,
    ReplayEncryption,
    Clock,
    IdGeneration,
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
            Self::IdempotencyKeyReused => "API token idempotency key was reused",
            Self::Clock | Self::IdGeneration => "API token application operation failed",
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
