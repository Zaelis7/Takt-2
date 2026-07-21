#![forbid(unsafe_code)]

use std::{
    error::Error,
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{
        PasswordHash as ParsedPasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString,
        rand_core::OsRng,
    },
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use takt_domain::{
    AuditActorType, AuditEvent, AuditEventId, BootstrapAuditMetadata, LocalUser, Membership,
    MembershipId, OperationId, Organization, OrganizationId, Project, ProjectId, RecoveryToken,
    RecoveryTokenId, ResourceId, Role, SessionId, UserId, UtcTimestamp,
    session::{BrowserSession, CsrfProof, SessionPolicy, SessionWindow},
};
use uuid::Uuid;
use zeroize::Zeroizing;

pub const DEFAULT_ORGANIZATION_SLUG: &str = "default";
pub const DEFAULT_PROJECT_SLUG: &str = "default";
pub const BOOTSTRAP_AUDIT_ACTION: &str = "admin.bootstrap";

/// Normalizes a local username using a deliberately conservative ASCII policy.
pub fn normalize_local_username(value: &str) -> Result<String, ValidationError> {
    let normalized = value.trim().to_ascii_lowercase();
    let mut characters = normalized.chars();
    let first = characters.next().ok_or(ValidationError::InvalidUsername)?;
    if normalized.len() > 64
        || !first.is_ascii_alphanumeric()
        || !normalized.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
        || !normalized
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_alphanumeric())
    {
        return Err(ValidationError::InvalidUsername);
    }
    Ok(normalized)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    InvalidUsername,
    PasswordTooShort,
    PasswordTooLong,
    InvalidArgon2Configuration,
    PasswordHashFailed,
    InvalidPasswordHash,
    InvalidTokenDigest,
    InvalidOpaqueToken,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidUsername => {
                "username must be 1-64 ASCII letters, digits, '.', '_' or '-', with an alphanumeric first and last character"
            }
            Self::PasswordTooShort => "password must contain at least 12 characters",
            Self::PasswordTooLong => "password must not exceed 1024 bytes",
            Self::InvalidArgon2Configuration => "Argon2id configuration is invalid",
            Self::PasswordHashFailed => "password hashing failed",
            Self::InvalidPasswordHash => "stored password hash is invalid",
            Self::InvalidTokenDigest => "token digest must be a lowercase SHA-256 value",
            Self::InvalidOpaqueToken => "opaque token must be 32-512 visible ASCII bytes",
        };
        formatter.write_str(message)
    }
}

impl Error for ValidationError {}

/// Central Argon2id parameters. The production composition root always uses
/// [`Self::production`]; reduced parameters exist only for deterministic tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Argon2idConfig {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl Argon2idConfig {
    /// OWASP-aligned production baseline: 19 MiB, two iterations, one lane.
    #[must_use]
    pub const fn production() -> Self {
        Self {
            memory_kib: 19_456,
            iterations: 2,
            parallelism: 1,
        }
    }

    /// Reduced-cost parameters for automated tests. Production code does not
    /// select these parameters.
    #[must_use]
    pub const fn testing() -> Self {
        Self {
            memory_kib: 64,
            iterations: 1,
            parallelism: 1,
        }
    }

    fn params(self) -> Result<Params, ValidationError> {
        Params::new(self.memory_kib, self.iterations, self.parallelism, Some(32))
            .map_err(|_| ValidationError::InvalidArgon2Configuration)
    }
}

/// An Argon2id PHC string that redacts its `Debug` representation and zeroizes
/// its allocation on drop.
#[derive(Clone)]
pub struct PasswordHash(Zeroizing<String>);

impl PasswordHash {
    pub fn from_persistence(value: String) -> Result<Self, ValidationError> {
        let parsed =
            ParsedPasswordHash::new(&value).map_err(|_| ValidationError::InvalidPasswordHash)?;
        if parsed.algorithm.as_str() != "argon2id" {
            return Err(ValidationError::InvalidPasswordHash);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    #[must_use]
    pub fn expose_for_persistence(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for PasswordHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PasswordHash([REDACTED])")
    }
}

/// SHA-256 digest of a high-entropy opaque token. The raw token is never
/// accepted by this type and its encoded digest is redacted from `Debug`.
#[derive(Clone)]
pub struct TokenDigest(Zeroizing<String>);

impl TokenDigest {
    /// Wraps exactly 32 digest bytes encoded as 64 lowercase hexadecimal characters.
    pub fn from_sha256_hex(value: &str) -> Result<Self, ValidationError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ValidationError::InvalidTokenDigest);
        }
        Ok(Self(Zeroizing::new(format!("sha256:{value}"))))
    }

    pub fn from_raw_token(value: &str) -> Result<Self, ValidationError> {
        OpaqueToken::validate(value)?;
        Self::from_sha256_hex(&format!("{:x}", Sha256::digest(value.as_bytes())))
    }

    pub fn from_persistence(value: String) -> Result<Self, ValidationError> {
        value
            .strip_prefix("sha256:")
            .ok_or(ValidationError::InvalidTokenDigest)
            .and_then(Self::from_sha256_hex)
    }

    #[must_use]
    pub fn constant_time_eq(&self, other: &Self) -> bool {
        self.0.as_bytes().ct_eq(other.0.as_bytes()).into()
    }

    /// Exposes the digest, never the token, only to a persistence adapter.
    #[must_use]
    pub fn expose_for_persistence(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for TokenDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TokenDigest([REDACTED])")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct OpaqueToken(Zeroizing<String>);

impl OpaqueToken {
    pub fn from_client_input(value: String) -> Result<Self, ValidationError> {
        Self::validate(&value)?;
        Ok(Self(Zeroizing::new(value)))
    }

    fn validate(value: &str) -> Result<(), ValidationError> {
        if !(32..=512).contains(&value.len()) || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(ValidationError::InvalidOpaqueToken);
        }
        Ok(())
    }

    #[must_use]
    pub fn expose_to_client(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for OpaqueToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpaqueToken([REDACTED])")
    }
}

pub trait TokenGenerator: Send + Sync {
    fn generate(&self) -> Result<OpaqueToken, AuthenticationError>;
}

pub struct SecureTokenGenerator;

impl TokenGenerator for SecureTokenGenerator {
    fn generate(&self) -> Result<OpaqueToken, AuthenticationError> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| AuthenticationError::TokenGeneration)?;
        let encoded = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        OpaqueToken::from_client_input(encoded).map_err(Into::into)
    }
}

/// Argon2id hashing and verification at the application boundary.
pub struct PasswordHasher {
    config: Argon2idConfig,
}

impl PasswordHasher {
    #[must_use]
    pub const fn new(config: Argon2idConfig) -> Self {
        Self { config }
    }

    pub fn hash(&self, password: &str) -> Result<PasswordHash, ValidationError> {
        validate_password(password)?;
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, self.config.params()?);
        let encoded = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| ValidationError::PasswordHashFailed)?
            .to_string();
        PasswordHash::from_persistence(encoded)
    }

    pub fn verify(&self, password: &str, hash: &PasswordHash) -> Result<bool, ValidationError> {
        validate_password(password)?;
        let parsed = ParsedPasswordHash::new(hash.expose_for_persistence())
            .map_err(|_| ValidationError::InvalidPasswordHash)?;
        let argon2 = Argon2::default();
        Ok(argon2.verify_password(password.as_bytes(), &parsed).is_ok())
    }
}

fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password.chars().count() < 12 {
        return Err(ValidationError::PasswordTooShort);
    }
    if password.len() > 1_024 {
        return Err(ValidationError::PasswordTooLong);
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct NewOrganization {
    pub id: OrganizationId,
    pub slug: String,
    pub name: String,
    pub now: UtcTimestamp,
}

#[derive(Clone, Debug)]
pub struct NewProject {
    pub id: ProjectId,
    pub organization_id: OrganizationId,
    pub slug: String,
    pub name: String,
    pub default_timezone: String,
    pub now: UtcTimestamp,
}

#[derive(Clone, Debug)]
pub struct NewLocalUser {
    pub id: UserId,
    pub normalized_username: String,
    pub display_name: String,
    pub password_hash: PasswordHash,
    pub now: UtcTimestamp,
}

#[derive(Clone, Debug)]
pub struct NewMembership {
    pub id: MembershipId,
    pub organization_id: OrganizationId,
    pub project_id: Option<ProjectId>,
    pub user_id: UserId,
    pub role: Role,
    pub now: UtcTimestamp,
}

#[derive(Clone, Debug)]
pub struct NewAuditEvent {
    pub event: AuditEvent,
}

pub const SESSION_CREATED_AUDIT_ACTION: &str = "auth.session.created";
pub const SESSION_REVOKED_AUDIT_ACTION: &str = "auth.session.revoked";
pub const RECOVERY_ISSUED_AUDIT_ACTION: &str = "auth.recovery.issued";
pub const RECOVERY_COMPLETED_AUDIT_ACTION: &str = "auth.recovery.completed";
pub const LOGIN_FAILED_AUDIT_ACTION: &str = "auth.login.failed";

#[derive(Clone, Debug)]
pub struct NewBrowserSession {
    pub id: SessionId,
    pub organization_id: OrganizationId,
    pub user_id: UserId,
    pub window: SessionWindow,
    pub token_digest: TokenDigest,
    pub csrf_digest: TokenDigest,
}

#[derive(Clone, Debug)]
pub struct CreateSessionPlan {
    pub session: NewBrowserSession,
    pub audit_event: NewAuditEvent,
}

#[derive(Clone, Debug)]
pub struct RevokeSessionPlan {
    pub session_id: SessionId,
    pub expected_version: i64,
    pub revoked_at: UtcTimestamp,
    pub audit_event: NewAuditEvent,
}

#[derive(Clone, Debug)]
pub struct NewRecoveryToken {
    pub id: RecoveryTokenId,
    pub organization_id: OrganizationId,
    pub user_id: UserId,
    pub token_digest: TokenDigest,
    pub expires_at: UtcTimestamp,
    pub now: UtcTimestamp,
}

#[derive(Clone, Debug)]
pub struct CreateRecoveryPlan {
    pub recovery: NewRecoveryToken,
    pub audit_event: NewAuditEvent,
}

#[derive(Clone, Debug)]
pub struct CompleteRecoveryPlan {
    pub token_digest: TokenDigest,
    pub replacement_password_hash: PasswordHash,
    pub completed_at: UtcTimestamp,
    pub audit_event: NewAuditEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryError {
    NotFound,
    AlreadyExists,
    VersionConflict,
    ConstraintViolation,
    DatabaseUnavailable,
    UnknownInfrastructure,
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotFound => "resource not found",
            Self::AlreadyExists => "resource already exists",
            Self::VersionConflict => "resource version conflict",
            Self::ConstraintViolation => "persistence constraint violated",
            Self::DatabaseUnavailable => "database unavailable",
            Self::UnknownInfrastructure => "unknown persistence infrastructure error",
        };
        formatter.write_str(message)
    }
}

impl Error for RepositoryError {}

#[async_trait]
pub trait OrganizationRepository: Send + Sync {
    async fn create_organization(
        &self,
        organization: NewOrganization,
    ) -> Result<Organization, RepositoryError>;
    async fn organization_by_id(&self, id: OrganizationId)
    -> Result<Organization, RepositoryError>;
    async fn organization_by_slug(&self, slug: &str) -> Result<Organization, RepositoryError>;
    async fn update_organization_name(
        &self,
        id: OrganizationId,
        expected_version: i64,
        name: &str,
        now: UtcTimestamp,
    ) -> Result<Organization, RepositoryError>;
}

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn create_project(&self, project: NewProject) -> Result<Project, RepositoryError>;
    async fn project_by_id(&self, id: ProjectId) -> Result<Project, RepositoryError>;
    async fn project_by_slug(
        &self,
        organization_id: OrganizationId,
        slug: &str,
    ) -> Result<Project, RepositoryError>;
}

#[async_trait]
pub trait LocalUserRepository: Send + Sync {
    async fn create_local_user(
        &self,
        user: NewLocalUser,
    ) -> Result<takt_domain::LocalUser, RepositoryError>;
    async fn local_user_by_username(
        &self,
        normalized_username: &str,
    ) -> Result<takt_domain::LocalUser, RepositoryError>;
}

#[async_trait]
pub trait MembershipRepository: Send + Sync {
    async fn create_membership(
        &self,
        membership: NewMembership,
    ) -> Result<Membership, RepositoryError>;
    async fn membership_by_scope(
        &self,
        organization_id: OrganizationId,
        project_id: Option<ProjectId>,
        user_id: UserId,
    ) -> Result<Membership, RepositoryError>;
}

#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn append_audit_event(&self, event: NewAuditEvent)
    -> Result<AuditEvent, RepositoryError>;
    async fn audit_event_by_id(&self, id: AuditEventId) -> Result<AuditEvent, RepositoryError>;
    async fn audit_events_for_organization(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<AuditEvent>, RepositoryError>;
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create_session(
        &self,
        plan: CreateSessionPlan,
    ) -> Result<BrowserSession, RepositoryError>;
    async fn session_by_token_digest(
        &self,
        token_digest: &TokenDigest,
    ) -> Result<BrowserSession, RepositoryError>;
    async fn session_by_token_and_csrf_digests(
        &self,
        token_digest: &TokenDigest,
        csrf_digest: &TokenDigest,
    ) -> Result<BrowserSession, RepositoryError>;
    async fn refresh_session(
        &self,
        id: SessionId,
        expected_version: i64,
        window: SessionWindow,
    ) -> Result<BrowserSession, RepositoryError>;
    async fn refresh_session_and_rotate_csrf(
        &self,
        id: SessionId,
        expected_version: i64,
        window: SessionWindow,
        csrf_digest: TokenDigest,
    ) -> Result<BrowserSession, RepositoryError>;
    async fn csrf_digest_by_session_id(
        &self,
        id: SessionId,
    ) -> Result<TokenDigest, RepositoryError>;
    async fn revoke_session(
        &self,
        plan: RevokeSessionPlan,
    ) -> Result<BrowserSession, RepositoryError>;
}

#[async_trait]
pub trait RecoveryRepository: Send + Sync {
    async fn create_recovery_token(
        &self,
        plan: CreateRecoveryPlan,
    ) -> Result<RecoveryToken, RepositoryError>;
    async fn recovery_token_by_digest(
        &self,
        token_digest: &TokenDigest,
    ) -> Result<RecoveryToken, RepositoryError>;
    async fn complete_recovery(
        &self,
        plan: CompleteRecoveryPlan,
    ) -> Result<RecoveryToken, RepositoryError>;
}

#[async_trait]
pub trait AuthenticationRepository: SessionRepository + AuditRepository {
    async fn local_authentication_context(
        &self,
    ) -> Result<LocalAuthenticationContext, RepositoryError>;
}

#[derive(Clone, Debug)]
pub struct LocalAuthenticationContext {
    pub organization_id: OrganizationId,
    pub project_id: ProjectId,
    pub membership_id: MembershipId,
    pub role: Role,
    pub user: LocalUser,
    pub password_hash: PasswordHash,
}

#[derive(Clone, Debug)]
pub struct BootstrapPlan {
    pub organization: NewOrganization,
    pub project: NewProject,
    pub user: NewLocalUser,
    pub membership: NewMembership,
    pub audit_event: NewAuditEvent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapResources {
    pub organization: Organization,
    pub project: Project,
    pub user: takt_domain::LocalUser,
    pub membership: Membership,
    pub audit_event: AuditEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapStatus {
    Created,
    AlreadyPresent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapOutput {
    pub status: BootstrapStatus,
    pub resources: BootstrapResources,
}

#[derive(Clone, Debug)]
pub struct ExistingBootstrap {
    pub resources: BootstrapResources,
    pub password_hash: PasswordHash,
}

#[derive(Clone, Debug)]
pub enum BootstrapStoreResult {
    Created(BootstrapResources),
    Existing(ExistingBootstrap),
    Conflict,
}

#[async_trait]
pub trait BootstrapRepository: Send + Sync {
    async fn bootstrap(&self, plan: BootstrapPlan)
    -> Result<BootstrapStoreResult, RepositoryError>;
}

pub trait Clock: Send + Sync {
    fn now(&self) -> Result<UtcTimestamp, ApplicationError>;
}

pub trait IdGenerator: Send + Sync {
    fn next_resource_id(&self) -> Result<ResourceId, ApplicationError>;
}

/// Port for CPU-intensive password operations. Runtime adapters must execute
/// these operations away from asynchronous executor worker threads.
#[async_trait]
pub trait PasswordHashing: Send + Sync {
    async fn hash(&self, password: &str) -> Result<PasswordHash, ValidationError>;
    async fn verify(&self, password: &str, hash: &PasswordHash) -> Result<bool, ValidationError>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Result<UtcTimestamp, ApplicationError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ApplicationError::Clock)?;
        let micros = i64::try_from(duration.as_micros()).map_err(|_| ApplicationError::Clock)?;
        Ok(UtcTimestamp::from_unix_micros(micros))
    }
}

pub struct UuidV7Generator;

impl IdGenerator for UuidV7Generator {
    fn next_resource_id(&self) -> Result<ResourceId, ApplicationError> {
        ResourceId::from_uuid(Uuid::now_v7()).map_err(|_| ApplicationError::IdGeneration)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationError {
    Validation(ValidationError),
    Conflict,
    Repository(RepositoryError),
    Clock,
    IdGeneration,
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(formatter),
            Self::Conflict => formatter.write_str("bootstrap data conflicts with existing state"),
            Self::Repository(error) => error.fmt(formatter),
            Self::Clock => formatter.write_str("system clock cannot produce a UTC timestamp"),
            Self::IdGeneration => formatter.write_str("UUIDv7 generation failed"),
        }
    }
}

impl Error for ApplicationError {}

impl From<ValidationError> for ApplicationError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}

impl From<RepositoryError> for ApplicationError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationError {
    Validation(ValidationError),
    InvalidCredentials,
    Unauthenticated,
    CsrfFailed,
    Repository(RepositoryError),
    Clock,
    IdGeneration,
    TokenGeneration,
}

impl fmt::Display for AuthenticationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(formatter),
            Self::InvalidCredentials => formatter.write_str("authentication failed"),
            Self::Unauthenticated => formatter.write_str("authentication failed"),
            Self::CsrfFailed => formatter.write_str("CSRF verification failed"),
            Self::Repository(error) => error.fmt(formatter),
            Self::Clock => formatter.write_str("system clock cannot produce a UTC timestamp"),
            Self::IdGeneration => formatter.write_str("UUIDv7 generation failed"),
            Self::TokenGeneration => formatter.write_str("secure token generation failed"),
        }
    }
}

impl Error for AuthenticationError {}

impl From<ValidationError> for AuthenticationError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}

impl From<RepositoryError> for AuthenticationError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

impl From<ApplicationError> for AuthenticationError {
    fn from(value: ApplicationError) -> Self {
        match value {
            ApplicationError::Validation(error) => Self::Validation(error),
            ApplicationError::Repository(error) => Self::Repository(error),
            ApplicationError::Clock => Self::Clock,
            ApplicationError::IdGeneration => Self::IdGeneration,
            ApplicationError::Conflict => Self::Repository(RepositoryError::VersionConflict),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserAuthentication {
    pub session: BrowserSession,
    pub user: LocalUser,
    pub role: Role,
    pub csrf_token: OpaqueToken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserLogin {
    pub authentication: BrowserAuthentication,
    pub session_token: OpaqueToken,
}

pub struct BrowserAuthenticationService<'a, R, H, C, I, T> {
    repository: &'a R,
    password_hasher: &'a H,
    clock: &'a C,
    ids: &'a I,
    tokens: &'a T,
    dummy_password_hash: PasswordHash,
    policy: SessionPolicy,
}

impl<'a, R, H, C, I, T> BrowserAuthenticationService<'a, R, H, C, I, T>
where
    R: AuthenticationRepository,
    H: PasswordHashing,
    C: Clock,
    I: IdGenerator,
    T: TokenGenerator,
{
    #[must_use]
    pub const fn new(
        repository: &'a R,
        password_hasher: &'a H,
        clock: &'a C,
        ids: &'a I,
        tokens: &'a T,
        dummy_password_hash: PasswordHash,
        policy: SessionPolicy,
    ) -> Self {
        Self {
            repository,
            password_hasher,
            clock,
            ids,
            tokens,
            dummy_password_hash,
            policy,
        }
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        request_id: OperationId,
    ) -> Result<BrowserLogin, AuthenticationError> {
        let username = normalize_local_username(username)?;
        let context = self.repository.local_authentication_context().await?;
        let known_user = context.user.normalized_username == username;
        let password_hash = if known_user {
            &context.password_hash
        } else {
            &self.dummy_password_hash
        };
        let verified = self.password_hasher.verify(password, password_hash).await?;
        let now = self.clock.now()?;
        if !known_user || !verified {
            let audit_id = AuditEventId::from_resource_id(self.ids.next_resource_id()?);
            self.repository
                .append_audit_event(authentication_audit(
                    &context,
                    audit_id,
                    request_id,
                    LOGIN_FAILED_AUDIT_ACTION,
                    "authentication",
                    context.organization_id.as_uuid(),
                    None,
                    now,
                )?)
                .await?;
            return Err(AuthenticationError::InvalidCredentials);
        }

        let session_token = self.tokens.generate()?;
        let csrf_token = self.tokens.generate()?;
        let session_id = SessionId::from_resource_id(self.ids.next_resource_id()?);
        let audit_id = AuditEventId::from_resource_id(self.ids.next_resource_id()?);
        let session = self
            .repository
            .create_session(CreateSessionPlan {
                session: NewBrowserSession {
                    id: session_id,
                    organization_id: context.organization_id,
                    user_id: context.user.id,
                    window: SessionWindow::issue(now, self.policy)
                        .map_err(|_| AuthenticationError::Clock)?,
                    token_digest: TokenDigest::from_raw_token(session_token.expose_to_client())?,
                    csrf_digest: TokenDigest::from_raw_token(csrf_token.expose_to_client())?,
                },
                audit_event: authentication_audit(
                    &context,
                    audit_id,
                    request_id,
                    SESSION_CREATED_AUDIT_ACTION,
                    "session",
                    session_id.as_uuid(),
                    Some(context.user.id),
                    now,
                )?,
            })
            .await?;
        Ok(BrowserLogin {
            authentication: browser_authentication(&context, session, csrf_token)?,
            session_token,
        })
    }

    pub async fn current_session(
        &self,
        session_token: &str,
    ) -> Result<BrowserAuthentication, AuthenticationError> {
        let digest = TokenDigest::from_raw_token(session_token)
            .map_err(|_| AuthenticationError::Unauthenticated)?;
        let session = self
            .repository
            .session_by_token_digest(&digest)
            .await
            .map_err(authentication_repository_error)?;
        let now = self.clock.now()?;
        ensure_active(&session, now)?;
        let context = self.repository.local_authentication_context().await?;
        ensure_context(&context, &session)?;
        let csrf_token = self.tokens.generate()?;
        let window = session
            .window
            .record_activity(now, self.policy)
            .map_err(|_| AuthenticationError::Unauthenticated)?;
        let refreshed = self
            .repository
            .refresh_session_and_rotate_csrf(
                session.id,
                session.version,
                window,
                TokenDigest::from_raw_token(csrf_token.expose_to_client())?,
            )
            .await
            .map_err(authentication_repository_error)?;
        browser_authentication(&context, refreshed, csrf_token)
    }

    pub async fn logout(
        &self,
        session_token: &str,
        csrf_token: &str,
        request_id: OperationId,
    ) -> Result<(), AuthenticationError> {
        let token_digest = TokenDigest::from_raw_token(session_token)
            .map_err(|_| AuthenticationError::Unauthenticated)?;
        let session = self
            .repository
            .session_by_token_digest(&token_digest)
            .await
            .map_err(authentication_repository_error)?;
        let now = self.clock.now()?;
        ensure_active(&session, now)?;
        let supplied_csrf =
            TokenDigest::from_raw_token(csrf_token).map_err(|_| AuthenticationError::CsrfFailed)?;
        let stored_csrf = self
            .repository
            .csrf_digest_by_session_id(session.id)
            .await
            .map_err(authentication_repository_error)?;
        let proof = if stored_csrf.constant_time_eq(&supplied_csrf) {
            CsrfProof::VerifiedForCurrentSession
        } else {
            CsrfProof::InvalidOrFromAnotherSession
        };
        session
            .window
            .authorize_browser_write(now, proof)
            .map_err(|_| AuthenticationError::CsrfFailed)?;
        let context = self.repository.local_authentication_context().await?;
        ensure_context(&context, &session)?;
        let audit_id = AuditEventId::from_resource_id(self.ids.next_resource_id()?);
        self.repository
            .revoke_session(RevokeSessionPlan {
                session_id: session.id,
                expected_version: session.version,
                revoked_at: now,
                audit_event: authentication_audit(
                    &context,
                    audit_id,
                    request_id,
                    SESSION_REVOKED_AUDIT_ACTION,
                    "session",
                    session.id.as_uuid(),
                    Some(session.user_id),
                    now,
                )?,
            })
            .await
            .map_err(authentication_repository_error)?;
        Ok(())
    }
}

fn ensure_active(session: &BrowserSession, now: UtcTimestamp) -> Result<(), AuthenticationError> {
    if session.revoked_at.is_some() || session.window.expiry_at(now).is_some() {
        Err(AuthenticationError::Unauthenticated)
    } else {
        Ok(())
    }
}

fn ensure_context(
    context: &LocalAuthenticationContext,
    session: &BrowserSession,
) -> Result<(), AuthenticationError> {
    if context.organization_id == session.organization_id && context.user.id == session.user_id {
        Ok(())
    } else {
        Err(AuthenticationError::Unauthenticated)
    }
}

fn browser_authentication(
    context: &LocalAuthenticationContext,
    session: BrowserSession,
    csrf_token: OpaqueToken,
) -> Result<BrowserAuthentication, AuthenticationError> {
    ensure_context(context, &session)?;
    Ok(BrowserAuthentication {
        session,
        user: context.user.clone(),
        role: context.role,
        csrf_token,
    })
}

fn authentication_repository_error(error: RepositoryError) -> AuthenticationError {
    match error {
        RepositoryError::NotFound | RepositoryError::VersionConflict => {
            AuthenticationError::Unauthenticated
        }
        other => AuthenticationError::Repository(other),
    }
}

#[allow(clippy::too_many_arguments)]
fn authentication_audit(
    context: &LocalAuthenticationContext,
    id: AuditEventId,
    request_id: OperationId,
    action: &str,
    resource_type: &str,
    resource_id: uuid::Uuid,
    actor_id: Option<UserId>,
    occurred_at: UtcTimestamp,
) -> Result<NewAuditEvent, AuthenticationError> {
    Ok(NewAuditEvent {
        event: AuditEvent {
            id,
            organization_id: context.organization_id,
            project_id: Some(context.project_id),
            actor_type: AuditActorType::System,
            actor_id,
            action: action.to_owned(),
            resource_type: resource_type.to_owned(),
            resource_id: ResourceId::from_uuid(resource_id)
                .map_err(|_| AuthenticationError::IdGeneration)?,
            request_id,
            metadata: BootstrapAuditMetadata {
                organization_id: context.organization_id,
                project_id: context.project_id,
                user_id: context.user.id,
                membership_id: context.membership_id,
            },
            occurred_at,
        },
    })
}

pub struct BootstrapService<'a, R, H, C, I> {
    repository: &'a R,
    password_hasher: &'a H,
    clock: &'a C,
    ids: &'a I,
}

impl<'a, R, H, C, I> BootstrapService<'a, R, H, C, I>
where
    R: BootstrapRepository,
    H: PasswordHashing,
    C: Clock,
    I: IdGenerator,
{
    #[must_use]
    pub const fn new(repository: &'a R, password_hasher: &'a H, clock: &'a C, ids: &'a I) -> Self {
        Self {
            repository,
            password_hasher,
            clock,
            ids,
        }
    }

    pub async fn execute(
        &self,
        username: &str,
        password: &str,
    ) -> Result<BootstrapOutput, ApplicationError> {
        let normalized_username = normalize_local_username(username)?;
        let password_hash = self.password_hasher.hash(password).await?;
        let now = self.clock.now()?;
        let organization_id = OrganizationId::from_resource_id(self.ids.next_resource_id()?);
        let project_id = ProjectId::from_resource_id(self.ids.next_resource_id()?);
        let user_id = UserId::from_resource_id(self.ids.next_resource_id()?);
        let membership_id = MembershipId::from_resource_id(self.ids.next_resource_id()?);
        let audit_id = AuditEventId::from_resource_id(self.ids.next_resource_id()?);
        let operation_id = OperationId::from_resource_id(self.ids.next_resource_id()?);

        let organization = NewOrganization {
            id: organization_id,
            slug: DEFAULT_ORGANIZATION_SLUG.to_owned(),
            name: "Default".to_owned(),
            now,
        };
        let project = NewProject {
            id: project_id,
            organization_id,
            slug: DEFAULT_PROJECT_SLUG.to_owned(),
            name: "Default".to_owned(),
            default_timezone: "UTC".to_owned(),
            now,
        };
        let user = NewLocalUser {
            id: user_id,
            normalized_username: normalized_username.clone(),
            display_name: "Local administrator".to_owned(),
            password_hash,
            now,
        };
        let membership = NewMembership {
            id: membership_id,
            organization_id,
            project_id: None,
            user_id,
            role: Role::Owner,
            now,
        };
        let audit_event = NewAuditEvent {
            event: AuditEvent {
                id: audit_id,
                organization_id,
                project_id: Some(project_id),
                actor_type: AuditActorType::LocalCli,
                actor_id: Some(user_id),
                action: BOOTSTRAP_AUDIT_ACTION.to_owned(),
                resource_type: "organization".to_owned(),
                resource_id: ResourceId::from_uuid(organization_id.as_uuid())
                    .map_err(|_| ApplicationError::Clock)?,
                request_id: operation_id,
                metadata: BootstrapAuditMetadata {
                    organization_id,
                    project_id,
                    user_id,
                    membership_id,
                },
                occurred_at: now,
            },
        };

        match self
            .repository
            .bootstrap(BootstrapPlan {
                organization,
                project,
                user,
                membership,
                audit_event,
            })
            .await?
        {
            BootstrapStoreResult::Created(resources) => Ok(BootstrapOutput {
                status: BootstrapStatus::Created,
                resources,
            }),
            BootstrapStoreResult::Existing(existing) => {
                if existing.resources.user.normalized_username == normalized_username
                    && self
                        .password_hasher
                        .verify(password, &existing.password_hash)
                        .await?
                {
                    Ok(BootstrapOutput {
                        status: BootstrapStatus::AlreadyPresent,
                        resources: existing.resources,
                    })
                } else {
                    Err(ApplicationError::Conflict)
                }
            }
            BootstrapStoreResult::Conflict => Err(ApplicationError::Conflict),
        }
    }
}
