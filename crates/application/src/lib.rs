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
use takt_domain::{
    AuditActorType, AuditEvent, AuditEventId, BootstrapAuditMetadata, Membership, MembershipId,
    OperationId, Organization, OrganizationId, Project, ProjectId, ResourceId, Role, SessionId,
    UserId, UtcTimestamp,
    session::{BrowserSession, SessionWindow},
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
