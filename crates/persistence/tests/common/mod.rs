#![forbid(unsafe_code)]

use std::{
    error::Error,
    sync::atomic::{AtomicI64, AtomicU64, Ordering},
};

use takt_application::api_token::{
    API_TOKEN_CREATED_AUDIT_ACTION, API_TOKEN_REVOKED_AUDIT_ACTION, API_TOKEN_UPDATED_AUDIT_ACTION,
    ApiTokenCreateIdempotencyError, ApiTokenCreateIdempotencyRepository,
    ApiTokenCreateIdempotencyResult, ApiTokenHash, ApiTokenIdempotencyContext,
    ApiTokenLifecycleRepository, ApiTokenListQuery, ApiTokenMutationIdempotencyError,
    ApiTokenMutationIdempotencyRepository, ApiTokenMutationIdempotencyResult, ApiTokenPatch,
    ApiTokenReplayCipher, ApiTokenStore, ApiTokenWriteMethod, CreateApiTokenIdempotencyPlan,
    CreateApiTokenPlan, NewApiToken, RevokeApiTokenPlan, StoredApiToken,
    UpdateApiTokenIdempotencyPlan, UpdateApiTokenPlan,
};
use takt_application::{
    ApplicationError, Argon2idConfig, AuditRepository, AuthenticationError, BootstrapService,
    BootstrapStatus, BrowserAuthenticationService, Clock, CompleteRecoveryPlan, CreateRecoveryPlan,
    CreateSessionPlan, IdGenerator, LocalUserRepository, MembershipRepository, NewAuditEvent,
    NewBrowserSession, NewLocalUser, NewMembership, NewOrganization, NewProject, NewRecoveryToken,
    OpaqueToken, OrganizationRepository, PasswordHash, PasswordHasher, PasswordHashing,
    ProjectRepository, RECOVERY_COMPLETED_AUDIT_ACTION, RECOVERY_ISSUED_AUDIT_ACTION,
    RecoveryRepository, RepositoryError, RevokeSessionPlan, SESSION_CREATED_AUDIT_ACTION,
    SESSION_REVOKED_AUDIT_ACTION, SessionRepository, TokenDigest, TokenGenerator, ValidationError,
};
use takt_domain::{
    ApiTokenId, AuditActorType, AuditEvent, AuditEventId, BootstrapAuditMetadata, MembershipId,
    OperationId, OrganizationId, ProjectId, RecoveryTokenId, ResourceId, Role, SessionId, UserId,
    UtcTimestamp,
    api_token::{ApiTokenKind, ApiTokenPrefix, ApiTokenScope, ApiTokenStatus, IpNetwork},
    session::{SessionPolicy, SessionWindow},
};
use takt_persistence::{Database, ReadinessError, SqlxRepository};

pub const TEST_NOW: UtcTimestamp = UtcTimestamp::from_unix_micros(1_784_445_600_123_456);
pub const TEST_PASSWORD: &str = "correct horse battery";
pub const TEST_RAW_SESSION_TOKEN: &str = "raw-session-token-must-never-be-stored";
pub const TEST_RAW_CSRF_TOKEN: &str = "raw-csrf-token-must-never-be-stored";
pub const TEST_RAW_RECOVERY_TOKEN: &str = "raw-recovery-token-must-never-be-stored";
pub const TEST_REPLACEMENT_PASSWORD: &str = "replacement horse battery";
pub const TEST_API_TOKEN_REPLAY_BODY: &[u8] = b"encrypted-create-response-marker";
pub const TEST_SESSION_TOKEN_SHA256: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const TEST_CSRF_TOKEN_SHA256: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub const TEST_RECOVERY_TOKEN_SHA256: &str =
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

pub fn assert_persisted_session_is_redacted(stored: &str, audit_metadata: &str) {
    let digest_count = stored.matches("sha256:").count();
    assert!(digest_count >= 2 && digest_count.is_multiple_of(2));
    for secret in [
        TEST_RAW_SESSION_TOKEN,
        TEST_RAW_CSRF_TOKEN,
        TEST_SESSION_TOKEN_SHA256,
        TEST_CSRF_TOKEN_SHA256,
    ] {
        assert!(!audit_metadata.contains(secret));
    }
    for suffix in 50_002..=50_009 {
        assert!(!stored.contains(&format!("{suffix:064x}")));
    }
    assert!(!stored.contains(TEST_RAW_SESSION_TOKEN) && !stored.contains(TEST_RAW_CSRF_TOKEN));
}

pub fn assert_persisted_recovery_is_redacted(stored: &str, audit_metadata: &str) {
    assert!(stored.contains("sha256:"));
    for secret in [
        TEST_RAW_RECOVERY_TOKEN,
        TEST_RECOVERY_TOKEN_SHA256,
        TEST_REPLACEMENT_PASSWORD,
    ] {
        assert!(!audit_metadata.contains(secret));
    }
    assert!(!stored.contains(TEST_RAW_RECOVERY_TOKEN));
}

pub fn assert_persisted_api_tokens_are_redacted(stored: &str, audit_metadata: &str) {
    assert!(stored.contains("$argon2id$"));
    assert!(!stored.contains(TEST_PASSWORD));
    assert!(!audit_metadata.contains(TEST_PASSWORD));
    assert!(!audit_metadata.contains("argon2"));
}

pub fn assert_replacement_password_hash(encoded: &str) -> Result<(), Box<dyn Error>> {
    let hash = PasswordHash::from_persistence(encoded.to_owned())?;
    assert!(
        PasswordHasher::new(Argon2idConfig::testing()).verify(TEST_REPLACEMENT_PASSWORD, &hash)?
    );
    Ok(())
}

pub struct FixedClock;

pub struct MutableClock(AtomicI64);

impl MutableClock {
    pub fn new(now: UtcTimestamp) -> Self {
        Self(AtomicI64::new(now.unix_micros()))
    }

    pub fn set(&self, now: UtcTimestamp) {
        self.0.store(now.unix_micros(), Ordering::Relaxed);
    }
}

impl Clock for MutableClock {
    fn now(&self) -> Result<UtcTimestamp, ApplicationError> {
        Ok(UtcTimestamp::from_unix_micros(
            self.0.load(Ordering::Relaxed),
        ))
    }
}

pub struct TestPasswordHasher(PasswordHasher);

impl TestPasswordHasher {
    pub fn new() -> Self {
        Self(PasswordHasher::new(
            takt_application::Argon2idConfig::testing(),
        ))
    }
}

#[async_trait::async_trait]
impl PasswordHashing for TestPasswordHasher {
    async fn hash(&self, password: &str) -> Result<PasswordHash, ValidationError> {
        self.0.hash(password)
    }

    async fn verify(&self, password: &str, hash: &PasswordHash) -> Result<bool, ValidationError> {
        self.0.verify(password, hash)
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Result<UtcTimestamp, ApplicationError> {
        Ok(TEST_NOW)
    }
}

pub struct SequenceIds(AtomicU64);

impl SequenceIds {
    pub const fn new(start: u64) -> Self {
        Self(AtomicU64::new(start))
    }
}

impl IdGenerator for SequenceIds {
    fn next_resource_id(&self) -> Result<ResourceId, ApplicationError> {
        let suffix = self.0.fetch_add(1, Ordering::Relaxed);
        ResourceId::parse(&format!("019b0000-0000-7000-8000-{suffix:012x}"))
            .map_err(|_| ApplicationError::IdGeneration)
    }
}

impl TokenGenerator for SequenceIds {
    fn generate(&self) -> Result<OpaqueToken, AuthenticationError> {
        let suffix = self.0.fetch_add(1, Ordering::Relaxed);
        OpaqueToken::from_client_input(format!("{suffix:064x}")).map_err(Into::into)
    }
}

pub fn resource_id(suffix: u64) -> Result<ResourceId, Box<dyn Error>> {
    Ok(ResourceId::parse(&format!(
        "019b0000-0000-7000-8000-{suffix:012x}"
    ))?)
}

// PRD-NFR-002: this exact suite is called for both concrete database engines.
pub async fn run_repository_contract(repository: &SqlxRepository) -> Result<(), Box<dyn Error>> {
    let organization_id = OrganizationId::from_resource_id(resource_id(1)?);
    let project_id = ProjectId::from_resource_id(resource_id(2)?);
    let user_id = UserId::from_resource_id(resource_id(3)?);
    let membership_id = MembershipId::from_resource_id(resource_id(4)?);
    let audit_id = AuditEventId::from_resource_id(resource_id(5)?);
    let request_id = OperationId::from_resource_id(resource_id(6)?);

    let organization = repository
        .create_organization(NewOrganization {
            id: organization_id,
            slug: "contract-org".to_owned(),
            name: "Contract Organization".to_owned(),
            now: TEST_NOW,
        })
        .await?;
    assert_eq!(organization.version, 1);
    assert_eq!(organization.created_at, TEST_NOW);
    assert_eq!(
        repository.organization_by_id(organization_id).await?,
        organization
    );
    assert_eq!(
        repository.organization_by_slug("contract-org").await?,
        organization
    );
    assert_eq!(
        repository
            .create_organization(NewOrganization {
                id: OrganizationId::from_resource_id(resource_id(7)?),
                slug: "contract-org".to_owned(),
                name: "Duplicate".to_owned(),
                now: TEST_NOW,
            })
            .await,
        Err(RepositoryError::AlreadyExists)
    );
    assert_eq!(
        repository
            .create_organization(NewOrganization {
                id: OrganizationId::from_resource_id(resource_id(18)?),
                slug: "overlong-name".to_owned(),
                name: "x".repeat(121),
                now: TEST_NOW,
            })
            .await,
        Err(RepositoryError::ConstraintViolation)
    );

    let update_time = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 1);
    let updated_organization = repository
        .update_organization_name(organization_id, 1, "Updated Organization", update_time)
        .await?;
    assert_eq!(updated_organization.name, "Updated Organization");
    assert_eq!(updated_organization.version, 2);
    assert_eq!(updated_organization.updated_at, update_time);
    assert_eq!(
        repository
            .update_organization_name(organization_id, 1, "Stale Update", update_time)
            .await,
        Err(RepositoryError::VersionConflict)
    );
    assert_eq!(
        repository
            .update_organization_name(
                OrganizationId::from_resource_id(resource_id(999)?),
                1,
                "Missing",
                update_time,
            )
            .await,
        Err(RepositoryError::NotFound)
    );

    let second_organization_id = OrganizationId::from_resource_id(resource_id(8)?);
    repository
        .create_organization(NewOrganization {
            id: second_organization_id,
            slug: "second-contract-org".to_owned(),
            name: "Second Contract Organization".to_owned(),
            now: TEST_NOW,
        })
        .await?;

    let project = repository
        .create_project(NewProject {
            id: project_id,
            organization_id,
            slug: "contract-project".to_owned(),
            name: "Contract Project".to_owned(),
            default_timezone: "UTC".to_owned(),
            now: TEST_NOW,
        })
        .await?;
    assert_eq!(repository.project_by_id(project_id).await?, project);
    assert_eq!(
        repository
            .project_by_slug(organization_id, "contract-project")
            .await?,
        project
    );
    assert_eq!(
        repository
            .create_project(NewProject {
                id: ProjectId::from_resource_id(resource_id(11)?),
                organization_id,
                slug: "contract-project".to_owned(),
                name: "Duplicate Project".to_owned(),
                default_timezone: "UTC".to_owned(),
                now: TEST_NOW,
            })
            .await,
        Err(RepositoryError::AlreadyExists)
    );
    repository
        .create_project(NewProject {
            id: ProjectId::from_resource_id(resource_id(9)?),
            organization_id: second_organization_id,
            slug: "contract-project".to_owned(),
            name: "Allowed Project".to_owned(),
            default_timezone: "UTC".to_owned(),
            now: TEST_NOW,
        })
        .await?;
    assert_eq!(
        repository
            .create_project(NewProject {
                id: ProjectId::from_resource_id(resource_id(10)?),
                organization_id: OrganizationId::from_resource_id(resource_id(999)?),
                slug: "orphan-project".to_owned(),
                name: "Orphan Project".to_owned(),
                default_timezone: "UTC".to_owned(),
                now: TEST_NOW,
            })
            .await,
        Err(RepositoryError::ConstraintViolation)
    );

    let hasher = PasswordHasher::new(takt_application::Argon2idConfig::testing());
    let user = repository
        .create_local_user(NewLocalUser {
            id: user_id,
            normalized_username: "contract.admin".to_owned(),
            display_name: "Contract administrator".to_owned(),
            password_hash: hasher.hash(TEST_PASSWORD)?,
            now: TEST_NOW,
        })
        .await?;
    assert_eq!(
        repository.local_user_by_username("contract.admin").await?,
        user
    );
    assert_eq!(
        repository
            .create_local_user(NewLocalUser {
                id: UserId::from_resource_id(resource_id(12)?),
                normalized_username: "contract.admin".to_owned(),
                display_name: "Duplicate administrator".to_owned(),
                password_hash: hasher.hash(TEST_PASSWORD)?,
                now: TEST_NOW,
            })
            .await,
        Err(RepositoryError::AlreadyExists)
    );

    let membership = repository
        .create_membership(NewMembership {
            id: membership_id,
            organization_id,
            project_id: Some(project_id),
            user_id,
            role: Role::Owner,
            now: TEST_NOW,
        })
        .await?;
    assert_eq!(
        repository
            .membership_by_scope(organization_id, Some(project_id), user_id)
            .await?,
        membership
    );
    assert_eq!(
        repository
            .create_membership(NewMembership {
                id: MembershipId::from_resource_id(resource_id(13)?),
                organization_id,
                project_id: Some(project_id),
                user_id: UserId::from_resource_id(resource_id(999)?),
                role: Role::Viewer,
                now: TEST_NOW,
            })
            .await,
        Err(RepositoryError::ConstraintViolation)
    );

    let metadata = BootstrapAuditMetadata {
        organization_id,
        project_id,
        user_id,
        membership_id,
    };
    let audit = repository
        .append_audit_event(NewAuditEvent {
            event: AuditEvent {
                id: audit_id,
                organization_id,
                project_id: Some(project_id),
                actor_type: AuditActorType::LocalCli,
                actor_id: Some(user_id),
                action: "contract.created".to_owned(),
                resource_type: "project".to_owned(),
                resource_id: resource_id(2)?,
                request_id,
                metadata,
                occurred_at: TEST_NOW,
            },
        })
        .await?;
    assert_eq!(repository.audit_event_by_id(audit_id).await?, audit);
    let second_audit_id = AuditEventId::from_resource_id(resource_id(14)?);
    let second_audit = repository
        .append_audit_event(NewAuditEvent {
            event: AuditEvent {
                id: second_audit_id,
                organization_id,
                project_id: Some(project_id),
                actor_type: AuditActorType::LocalCli,
                actor_id: Some(user_id),
                action: "contract.updated".to_owned(),
                resource_type: "project".to_owned(),
                resource_id: resource_id(2)?,
                request_id: OperationId::from_resource_id(resource_id(15)?),
                metadata: BootstrapAuditMetadata {
                    organization_id,
                    project_id,
                    user_id,
                    membership_id,
                },
                occurred_at: TEST_NOW,
            },
        })
        .await?;
    assert_eq!(
        repository
            .audit_events_for_organization(organization_id)
            .await?,
        vec![audit, second_audit]
    );
    assert_eq!(
        repository.organization_by_slug("missing").await,
        Err(RepositoryError::NotFound)
    );

    let first_repository = repository.clone();
    let second_repository = repository.clone();
    let first_write = first_repository.create_organization(NewOrganization {
        id: OrganizationId::from_resource_id(resource_id(16)?),
        slug: "concurrent-contract-org".to_owned(),
        name: "Concurrent Organization A".to_owned(),
        now: TEST_NOW,
    });
    let second_write = second_repository.create_organization(NewOrganization {
        id: OrganizationId::from_resource_id(resource_id(17)?),
        slug: "concurrent-contract-org".to_owned(),
        name: "Concurrent Organization B".to_owned(),
        now: TEST_NOW,
    });
    let (first_result, second_result) = tokio::join!(first_write, second_write);
    assert!(
        first_result.is_ok() && second_result == Err(RepositoryError::AlreadyExists)
            || second_result.is_ok() && first_result == Err(RepositoryError::AlreadyExists)
    );
    assert!("invalid-role".parse::<Role>().is_err());
    Ok(())
}

fn session_audit_event(
    audit_id: AuditEventId,
    session_id: SessionId,
    action: &str,
    occurred_at: UtcTimestamp,
) -> Result<NewAuditEvent, Box<dyn Error>> {
    let organization_id = OrganizationId::from_resource_id(resource_id(1)?);
    let project_id = ProjectId::from_resource_id(resource_id(2)?);
    let user_id = UserId::from_resource_id(resource_id(3)?);
    let membership_id = MembershipId::from_resource_id(resource_id(4)?);
    Ok(NewAuditEvent {
        event: AuditEvent {
            id: audit_id,
            organization_id,
            project_id: Some(project_id),
            actor_type: AuditActorType::System,
            actor_id: Some(user_id),
            action: action.to_owned(),
            resource_type: "session".to_owned(),
            resource_id: ResourceId::from_uuid(session_id.as_uuid())?,
            request_id: OperationId::from_resource_id(resource_id(39_999)?),
            metadata: BootstrapAuditMetadata {
                organization_id,
                project_id,
                user_id,
                membership_id,
            },
            occurred_at,
        },
    })
}

fn new_session(
    id_suffix: u64,
    token_hex: &str,
    csrf_hex: &str,
) -> Result<NewBrowserSession, Box<dyn Error>> {
    Ok(NewBrowserSession {
        id: SessionId::from_resource_id(resource_id(id_suffix)?),
        organization_id: OrganizationId::from_resource_id(resource_id(1)?),
        user_id: UserId::from_resource_id(resource_id(3)?),
        window: SessionWindow::issue(TEST_NOW, SessionPolicy::default())?,
        token_digest: TokenDigest::from_sha256_hex(token_hex)?,
        csrf_digest: TokenDigest::from_sha256_hex(csrf_hex)?,
    })
}

// PRD-IAM-001 / PRD-IAM-004 / PRD-IAM-005 / PRD-DATA-001 / PRD-DATA-004 /
// PRD-NFR-002 / PRD-NFR-005:
// this exact contract runs against PostgreSQL and SQLite.
pub async fn run_session_repository_contract(
    repository: &SqlxRepository,
) -> Result<(), Box<dyn Error>> {
    let token_digest = TokenDigest::from_sha256_hex(TEST_SESSION_TOKEN_SHA256)?;
    let csrf_digest = TokenDigest::from_sha256_hex(TEST_CSRF_TOKEN_SHA256)?;
    assert!(!format!("{token_digest:?}").contains(TEST_SESSION_TOKEN_SHA256));
    assert!(!format!("{csrf_digest:?}").contains(TEST_CSRF_TOKEN_SHA256));

    let session = new_session(30_000, TEST_SESSION_TOKEN_SHA256, TEST_CSRF_TOKEN_SHA256)?;
    let session_id = session.id;
    let create_audit_id = AuditEventId::from_resource_id(resource_id(30_001)?);
    let created = repository
        .create_session(CreateSessionPlan {
            session,
            audit_event: session_audit_event(
                create_audit_id,
                session_id,
                SESSION_CREATED_AUDIT_ACTION,
                TEST_NOW,
            )?,
        })
        .await?;
    assert_eq!(created.id, session_id);
    assert_eq!(created.version, 1);
    assert_eq!(created.revoked_at, None);
    assert_eq!(
        repository.session_by_token_digest(&token_digest).await?,
        created
    );
    assert_eq!(
        repository
            .session_by_token_and_csrf_digests(&token_digest, &csrf_digest)
            .await?,
        created
    );
    let wrong_csrf = TokenDigest::from_sha256_hex(
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    )?;
    assert_eq!(
        repository
            .session_by_token_and_csrf_digests(&token_digest, &wrong_csrf)
            .await,
        Err(RepositoryError::NotFound)
    );

    let activity_at = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 3_600_000_000);
    let refreshed_window = created
        .window
        .record_activity(activity_at, SessionPolicy::default())?;
    let refreshed = repository
        .refresh_session(session_id, 1, refreshed_window)
        .await?;
    assert_eq!(refreshed.version, 2);
    assert_eq!(refreshed.window.last_activity_at(), activity_at);
    assert_eq!(
        repository
            .refresh_session(session_id, 1, refreshed_window)
            .await,
        Err(RepositoryError::VersionConflict)
    );
    assert_eq!(
        repository
            .refresh_session(session_id, 2, created.window)
            .await,
        Err(RepositoryError::VersionConflict),
        "activity timestamps must never move backwards"
    );

    let expired_attempt_at = refreshed.window.expires_at();
    let expired_window = SessionWindow::from_persistence(
        refreshed.window.issued_at(),
        expired_attempt_at,
        UtcTimestamp::from_unix_micros(expired_attempt_at.unix_micros() + 1),
        refreshed.window.absolute_expires_at(),
    )?;
    assert_eq!(
        repository
            .refresh_session(session_id, 2, expired_window)
            .await,
        Err(RepositoryError::VersionConflict),
        "an expired stored session must not be revived"
    );
    let missing_session_id = SessionId::from_resource_id(resource_id(30_099)?);
    assert_eq!(
        repository
            .refresh_session(missing_session_id, 1, refreshed_window)
            .await,
        Err(RepositoryError::NotFound)
    );

    let revoked_at = UtcTimestamp::from_unix_micros(activity_at.unix_micros() + 1);
    assert_eq!(
        repository
            .revoke_session(RevokeSessionPlan {
                session_id,
                expected_version: 2,
                revoked_at,
                audit_event: session_audit_event(
                    AuditEventId::from_resource_id(resource_id(30_005)?),
                    session_id,
                    SESSION_CREATED_AUDIT_ACTION,
                    revoked_at,
                )?,
            })
            .await,
        Err(RepositoryError::ConstraintViolation),
        "revoke must reject an incoherent audit action before writing"
    );
    let revoked = repository
        .revoke_session(RevokeSessionPlan {
            session_id,
            expected_version: 2,
            revoked_at,
            audit_event: session_audit_event(
                AuditEventId::from_resource_id(resource_id(30_002)?),
                session_id,
                SESSION_REVOKED_AUDIT_ACTION,
                revoked_at,
            )?,
        })
        .await?;
    assert_eq!(revoked.revoked_at, Some(revoked_at));
    assert_eq!(revoked.version, 3);
    let post_revoke_window = revoked.window.record_activity(
        UtcTimestamp::from_unix_micros(revoked_at.unix_micros() + 1),
        SessionPolicy::default(),
    )?;
    assert_eq!(
        repository
            .refresh_session(session_id, 3, post_revoke_window)
            .await,
        Err(RepositoryError::VersionConflict),
        "a revoked session must not be refreshed"
    );
    assert_eq!(
        repository
            .revoke_session(RevokeSessionPlan {
                session_id,
                expected_version: 3,
                revoked_at,
                audit_event: session_audit_event(
                    AuditEventId::from_resource_id(resource_id(30_003)?),
                    session_id,
                    SESSION_REVOKED_AUDIT_ACTION,
                    revoked_at,
                )?,
            })
            .await,
        Err(RepositoryError::VersionConflict)
    );
    assert_eq!(
        repository
            .revoke_session(RevokeSessionPlan {
                session_id: missing_session_id,
                expected_version: 1,
                revoked_at,
                audit_event: session_audit_event(
                    AuditEventId::from_resource_id(resource_id(30_004)?),
                    missing_session_id,
                    SESSION_REVOKED_AUDIT_ACTION,
                    revoked_at,
                )?,
            })
            .await,
        Err(RepositoryError::NotFound)
    );

    let concurrent_session = new_session(
        30_020,
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        "1111111111111111111111111111111111111111111111111111111111111111",
    )?;
    let concurrent_id = concurrent_session.id;
    let concurrent_token = concurrent_session.token_digest.clone();
    let concurrent_window = concurrent_session.window;
    repository
        .create_session(CreateSessionPlan {
            session: concurrent_session,
            audit_event: session_audit_event(
                AuditEventId::from_resource_id(resource_id(30_021)?),
                concurrent_id,
                SESSION_CREATED_AUDIT_ACTION,
                TEST_NOW,
            )?,
        })
        .await?;
    let left_window = concurrent_window.record_activity(
        UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 1_000),
        SessionPolicy::default(),
    )?;
    let right_window = concurrent_window.record_activity(
        UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 2_000),
        SessionPolicy::default(),
    )?;
    let (left, right) = tokio::join!(
        repository.refresh_session(concurrent_id, 1, left_window),
        repository.refresh_session(concurrent_id, 1, right_window)
    );
    assert!(
        matches!((&left, &right), (Ok(session), Err(RepositoryError::VersionConflict)) if session.version == 2)
            || matches!((&left, &right), (Err(RepositoryError::VersionConflict), Ok(session)) if session.version == 2)
    );
    assert_eq!(
        repository
            .session_by_token_digest(&concurrent_token)
            .await?
            .version,
        2
    );

    let rollback_revoke = new_session(
        30_030,
        "2222222222222222222222222222222222222222222222222222222222222222",
        "3333333333333333333333333333333333333333333333333333333333333333",
    )?;
    let rollback_revoke_id = rollback_revoke.id;
    let rollback_revoke_token = rollback_revoke.token_digest.clone();
    let rollback_audit_id = AuditEventId::from_resource_id(resource_id(30_031)?);
    repository
        .create_session(CreateSessionPlan {
            session: rollback_revoke,
            audit_event: session_audit_event(
                rollback_audit_id,
                rollback_revoke_id,
                SESSION_CREATED_AUDIT_ACTION,
                TEST_NOW,
            )?,
        })
        .await?;
    assert_eq!(
        repository
            .revoke_session(RevokeSessionPlan {
                session_id: rollback_revoke_id,
                expected_version: 1,
                revoked_at,
                audit_event: session_audit_event(
                    rollback_audit_id,
                    rollback_revoke_id,
                    SESSION_REVOKED_AUDIT_ACTION,
                    revoked_at,
                )?,
            })
            .await,
        Err(RepositoryError::AlreadyExists)
    );
    let active_after_rollback = repository
        .session_by_token_digest(&rollback_revoke_token)
        .await?;
    assert_eq!(active_after_rollback.revoked_at, None);
    assert_eq!(active_after_rollback.version, 1);

    let rolled_back_create = new_session(
        30_010,
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
    )?;
    let rolled_back_token = rolled_back_create.token_digest.clone();
    let rolled_back_id = rolled_back_create.id;
    assert_eq!(
        repository
            .create_session(CreateSessionPlan {
                session: rolled_back_create,
                audit_event: session_audit_event(
                    create_audit_id,
                    rolled_back_id,
                    SESSION_CREATED_AUDIT_ACTION,
                    TEST_NOW,
                )?,
            })
            .await,
        Err(RepositoryError::AlreadyExists)
    );
    assert_eq!(
        repository.session_by_token_digest(&rolled_back_token).await,
        Err(RepositoryError::NotFound),
        "a duplicate audit id must roll back the preceding session insert"
    );

    let session_audits = repository
        .audit_events_for_organization(OrganizationId::from_resource_id(resource_id(1)?))
        .await?
        .into_iter()
        .filter(|event| event.resource_type == "session")
        .collect::<Vec<_>>();
    assert_eq!(session_audits.len(), 4);
    assert!(session_audits.iter().all(|event| {
        !format!("{event:?}").contains(TEST_RAW_SESSION_TOKEN)
            && !format!("{event:?}").contains(TEST_RAW_CSRF_TOKEN)
            && !format!("{event:?}").contains(TEST_SESSION_TOKEN_SHA256)
            && !format!("{event:?}").contains(TEST_CSRF_TOKEN_SHA256)
    }));
    Ok(())
}

fn recovery_audit_event(
    audit_id: AuditEventId,
    recovery_id: RecoveryTokenId,
    action: &str,
    occurred_at: UtcTimestamp,
) -> Result<NewAuditEvent, Box<dyn Error>> {
    let organization_id = OrganizationId::from_resource_id(resource_id(1)?);
    let project_id = ProjectId::from_resource_id(resource_id(2)?);
    let user_id = UserId::from_resource_id(resource_id(3)?);
    Ok(NewAuditEvent {
        event: AuditEvent {
            id: audit_id,
            organization_id,
            project_id: Some(project_id),
            actor_type: AuditActorType::System,
            actor_id: Some(user_id),
            action: action.to_owned(),
            resource_type: "recovery_token".to_owned(),
            resource_id: ResourceId::from_uuid(recovery_id.as_uuid())?,
            request_id: OperationId::from_resource_id(resource_id(49_999)?),
            metadata: BootstrapAuditMetadata {
                organization_id,
                project_id,
                user_id,
                membership_id: MembershipId::from_resource_id(resource_id(4)?),
            },
            occurred_at,
        },
    })
}

fn new_recovery_token(
    id_suffix: u64,
    digest_hex: &str,
    expires_at: UtcTimestamp,
) -> Result<NewRecoveryToken, Box<dyn Error>> {
    Ok(NewRecoveryToken {
        id: RecoveryTokenId::from_resource_id(resource_id(id_suffix)?),
        organization_id: OrganizationId::from_resource_id(resource_id(1)?),
        user_id: UserId::from_resource_id(resource_id(3)?),
        token_digest: TokenDigest::from_sha256_hex(digest_hex)?,
        expires_at,
        now: TEST_NOW,
    })
}

fn replacement_password_hash() -> Result<PasswordHash, Box<dyn Error>> {
    Ok(PasswordHasher::new(Argon2idConfig::testing()).hash(TEST_REPLACEMENT_PASSWORD)?)
}

fn create_recovery_plan(
    recovery: NewRecoveryToken,
    audit_id: AuditEventId,
) -> Result<CreateRecoveryPlan, Box<dyn Error>> {
    Ok(CreateRecoveryPlan {
        audit_event: recovery_audit_event(
            audit_id,
            recovery.id,
            RECOVERY_ISSUED_AUDIT_ACTION,
            recovery.now,
        )?,
        recovery,
    })
}

fn complete_recovery_plan(
    token_digest: TokenDigest,
    recovery_id: RecoveryTokenId,
    audit_id: AuditEventId,
    completed_at: UtcTimestamp,
    action: &str,
) -> Result<CompleteRecoveryPlan, Box<dyn Error>> {
    Ok(CompleteRecoveryPlan {
        token_digest,
        replacement_password_hash: replacement_password_hash()?,
        completed_at,
        audit_event: recovery_audit_event(audit_id, recovery_id, action, completed_at)?,
    })
}

// PRD-IAM-001 / PRD-IAM-004 / PRD-IAM-005 / PRD-DATA-001 / PRD-DATA-002 /
// PRD-DATA-004 / PRD-NFR-002 / PRD-NFR-005:
// this exact recovery contract runs against PostgreSQL and SQLite.
pub async fn run_recovery_repository_contract(
    repository: &SqlxRepository,
) -> Result<(), Box<dyn Error>> {
    let expiry = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 900_000_000);
    let digest = TokenDigest::from_sha256_hex(TEST_RECOVERY_TOKEN_SHA256)?;
    assert!(!format!("{digest:?}").contains(TEST_RECOVERY_TOKEN_SHA256));

    let recovery = new_recovery_token(40_000, TEST_RECOVERY_TOKEN_SHA256, expiry)?;
    let recovery_id = recovery.id;
    let issue_audit_id = AuditEventId::from_resource_id(resource_id(40_001)?);
    let created = repository
        .create_recovery_token(create_recovery_plan(recovery, issue_audit_id)?)
        .await?;
    assert_eq!(created.id, recovery_id);
    assert_eq!(created.version, 1);
    assert_eq!(created.consumed_at, None);
    assert_eq!(repository.recovery_token_by_digest(&digest).await?, created);

    let completed_at = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 60_000_000);
    let completed = repository
        .complete_recovery(complete_recovery_plan(
            digest.clone(),
            recovery_id,
            AuditEventId::from_resource_id(resource_id(40_002)?),
            completed_at,
            RECOVERY_COMPLETED_AUDIT_ACTION,
        )?)
        .await?;
    assert_eq!(completed.consumed_at, Some(completed_at));
    assert_eq!(completed.version, 2);
    for session_hex in ["f".repeat(64), "2".repeat(64)] {
        assert_eq!(
            repository
                .session_by_token_digest(&TokenDigest::from_sha256_hex(&session_hex)?)
                .await?
                .revoked_at,
            Some(completed_at)
        );
    }
    assert_eq!(
        repository
            .complete_recovery(complete_recovery_plan(
                digest,
                recovery_id,
                AuditEventId::from_resource_id(resource_id(40_003)?),
                completed_at,
                RECOVERY_COMPLETED_AUDIT_ACTION,
            )?)
            .await,
        Err(RepositoryError::VersionConflict),
        "a consumed token must reject replay"
    );

    let expired_digest = TokenDigest::from_sha256_hex(
        "8888888888888888888888888888888888888888888888888888888888888888",
    )?;
    let expired_at = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 1);
    let expired = new_recovery_token(40_020, &"8".repeat(64), expired_at)?;
    let expired_id = expired.id;
    repository
        .create_recovery_token(create_recovery_plan(
            expired,
            AuditEventId::from_resource_id(resource_id(40_021)?),
        )?)
        .await?;
    assert_eq!(
        repository
            .complete_recovery(complete_recovery_plan(
                expired_digest.clone(),
                expired_id,
                AuditEventId::from_resource_id(resource_id(40_022)?),
                expired_at,
                RECOVERY_COMPLETED_AUDIT_ACTION,
            )?)
            .await,
        Err(RepositoryError::VersionConflict)
    );
    assert_eq!(
        repository
            .recovery_token_by_digest(&expired_digest)
            .await?
            .consumed_at,
        None
    );

    let rollback_digest = TokenDigest::from_sha256_hex(
        "9999999999999999999999999999999999999999999999999999999999999999",
    )?;
    let rollback = new_recovery_token(40_030, &"9".repeat(64), expiry)?;
    let rollback_id = rollback.id;
    repository
        .create_recovery_token(create_recovery_plan(
            rollback,
            AuditEventId::from_resource_id(resource_id(40_031)?),
        )?)
        .await?;
    let rollback_session = new_session(40_032, &"0".repeat(64), &"ab".repeat(32))?;
    let rollback_session_token = rollback_session.token_digest.clone();
    repository
        .create_session(CreateSessionPlan {
            audit_event: session_audit_event(
                AuditEventId::from_resource_id(resource_id(40_033)?),
                rollback_session.id,
                SESSION_CREATED_AUDIT_ACTION,
                TEST_NOW,
            )?,
            session: rollback_session,
        })
        .await?;
    assert_eq!(
        repository
            .complete_recovery(complete_recovery_plan(
                rollback_digest.clone(),
                rollback_id,
                issue_audit_id,
                completed_at,
                RECOVERY_COMPLETED_AUDIT_ACTION,
            )?)
            .await,
        Err(RepositoryError::AlreadyExists)
    );
    assert_eq!(
        repository
            .recovery_token_by_digest(&rollback_digest)
            .await?
            .consumed_at,
        None,
        "token consumption must roll back when the audit insert fails"
    );
    assert_eq!(
        repository
            .session_by_token_digest(&rollback_session_token)
            .await?
            .revoked_at,
        None,
        "session revocation must roll back with the failed completion audit"
    );

    let concurrent_digest = TokenDigest::from_sha256_hex(&"cd".repeat(32))?;
    let concurrent_recovery = new_recovery_token(40_040, &"cd".repeat(32), expiry)?;
    let concurrent_id = concurrent_recovery.id;
    repository
        .create_recovery_token(create_recovery_plan(
            concurrent_recovery,
            AuditEventId::from_resource_id(resource_id(40_041)?),
        )?)
        .await?;
    let (left, right) = tokio::join!(
        repository.complete_recovery(complete_recovery_plan(
            concurrent_digest.clone(),
            concurrent_id,
            AuditEventId::from_resource_id(resource_id(40_042)?),
            completed_at,
            RECOVERY_COMPLETED_AUDIT_ACTION,
        )?),
        repository.complete_recovery(complete_recovery_plan(
            concurrent_digest.clone(),
            concurrent_id,
            AuditEventId::from_resource_id(resource_id(40_043)?),
            completed_at,
            RECOVERY_COMPLETED_AUDIT_ACTION,
        )?)
    );
    assert!(
        matches!((&left, &right), (Ok(result), Err(RepositoryError::VersionConflict)) if result.version == 2)
            || matches!((&left, &right), (Err(RepositoryError::VersionConflict), Ok(result)) if result.version == 2),
        "exactly one concurrent recovery completion must succeed"
    );

    Ok(())
}

fn api_token_audit_event(
    audit_id: AuditEventId,
    token_id: ApiTokenId,
    action: &str,
    occurred_at: UtcTimestamp,
) -> Result<NewAuditEvent, Box<dyn Error>> {
    let organization_id = OrganizationId::from_resource_id(resource_id(1)?);
    let project_id = ProjectId::from_resource_id(resource_id(2)?);
    let user_id = UserId::from_resource_id(resource_id(3)?);
    Ok(NewAuditEvent {
        event: AuditEvent {
            id: audit_id,
            organization_id,
            project_id: Some(project_id),
            actor_type: AuditActorType::System,
            actor_id: Some(user_id),
            action: action.to_owned(),
            resource_type: "api_token".to_owned(),
            resource_id: ResourceId::from_uuid(token_id.as_uuid())?,
            request_id: OperationId::from_resource_id(resource_id(69_999)?),
            metadata: BootstrapAuditMetadata {
                organization_id,
                project_id,
                user_id,
                membership_id: MembershipId::from_resource_id(resource_id(4)?),
            },
            occurred_at,
        },
    })
}

fn new_api_token(
    id_suffix: u64,
    prefix: &str,
    name: &str,
    now: UtcTimestamp,
) -> Result<NewApiToken, Box<dyn Error>> {
    let password_hash = PasswordHasher::new(Argon2idConfig::testing()).hash(TEST_PASSWORD)?;
    Ok(NewApiToken {
        id: ApiTokenId::from_resource_id(resource_id(id_suffix)?),
        organization_id: OrganizationId::from_resource_id(resource_id(1)?),
        project_id: Some(ProjectId::from_resource_id(resource_id(2)?)),
        name: name.to_owned(),
        kind: ApiTokenKind::Personal,
        token_prefix: prefix.parse::<ApiTokenPrefix>()?,
        token_hash: ApiTokenHash::from_persistence(
            password_hash.expose_for_persistence().to_owned(),
        )?,
        scopes: vec!["monitors:read".parse::<ApiTokenScope>()?],
        ip_networks: vec!["192.0.2.0/24".parse::<IpNetwork>()?],
        expires_at: Some(UtcTimestamp::from_unix_micros(
            TEST_NOW.unix_micros() + 3_600_000_000,
        )),
        now,
    })
}

fn idempotent_create_plan(
    id_suffix: u64,
    prefix: &str,
    name: &str,
    audit_suffix: u64,
    key: &str,
    request_hash: [u8; 32],
    now: UtcTimestamp,
) -> Result<CreateApiTokenIdempotencyPlan, Box<dyn Error>> {
    let mut token = new_api_token(id_suffix, prefix, name, now)?;
    token.expires_at = Some(UtcTimestamp::from_unix_micros(
        now.unix_micros() + 3_600_000_000,
    ));
    let context = ApiTokenIdempotencyContext::new(
        AuditActorType::System,
        resource_id(3)?,
        ApiTokenWriteMethod::Post,
        "/api/v1/api-tokens".to_owned(),
        key.to_owned(),
        request_hash,
        now,
    )?;
    let encrypted_replay =
        ApiTokenReplayCipher::new(1, [0x42; 32])?.encrypt(&context, TEST_API_TOKEN_REPLAY_BODY)?;
    Ok(CreateApiTokenIdempotencyPlan {
        create: CreateApiTokenPlan {
            audit_event: api_token_audit_event(
                AuditEventId::from_resource_id(resource_id(audit_suffix)?),
                token.id,
                API_TOKEN_CREATED_AUDIT_ACTION,
                now,
            )?,
            token,
        },
        context,
        encrypted_replay,
    })
}

fn mutation_context(
    id: ApiTokenId,
    method: ApiTokenWriteMethod,
    key: &str,
    request_hash: [u8; 32],
    now: UtcTimestamp,
) -> Result<ApiTokenIdempotencyContext, Box<dyn Error>> {
    ApiTokenIdempotencyContext::new(
        AuditActorType::System,
        resource_id(3)?,
        method,
        format!("/api/v1/api-tokens/{id}"),
        key.to_owned(),
        request_hash,
        now,
    )
    .map_err(Into::into)
}

fn idempotent_update_plan(
    id: ApiTokenId,
    expected_version: i64,
    name: &str,
    audit_suffix: u64,
    key: &str,
    request_hash: [u8; 32],
    now: UtcTimestamp,
) -> Result<UpdateApiTokenIdempotencyPlan, Box<dyn Error>> {
    Ok(UpdateApiTokenIdempotencyPlan {
        update: UpdateApiTokenPlan {
            id,
            expected_version,
            patch: ApiTokenPatch {
                name: Some(name.to_owned()),
                expires_at: None,
                ip_networks: None,
            },
            now,
            audit_event: api_token_audit_event(
                AuditEventId::from_resource_id(resource_id(audit_suffix)?),
                id,
                API_TOKEN_UPDATED_AUDIT_ACTION,
                now,
            )?,
        },
        context: mutation_context(id, ApiTokenWriteMethod::Patch, key, request_hash, now)?,
    })
}

async fn create_mutation_token(
    repository: &SqlxRepository,
    id_suffix: u64,
    prefix: &str,
    name: &str,
    audit_suffix: u64,
) -> Result<ApiTokenId, Box<dyn Error>> {
    let mut token = new_api_token(id_suffix, prefix, name, TEST_NOW)?;
    token.expires_at = None;
    let id = token.id;
    repository
        .create_api_token(CreateApiTokenPlan {
            token,
            audit_event: api_token_audit_event(
                AuditEventId::from_resource_id(resource_id(audit_suffix)?),
                id,
                API_TOKEN_CREATED_AUDIT_ACTION,
                TEST_NOW,
            )?,
        })
        .await?;
    Ok(id)
}

// PRD-IAM-001 / PRD-IAM-004 / PRD-IAM-005 / PRD-DATA-001 / PRD-DATA-002 /
// PRD-DATA-004 / PRD-NFR-002 / PRD-NFR-005: both engines execute this contract.
pub async fn run_api_token_repository_contract(
    repository: &SqlxRepository,
) -> Result<(), Box<dyn Error>> {
    let first = new_api_token(60_000, "takt_0011223344556677", "first", TEST_NOW)?;
    let first_id = first.id;
    let first_prefix = first.token_prefix.clone();
    let created = repository
        .create_api_token(CreateApiTokenPlan {
            token: first,
            audit_event: api_token_audit_event(
                AuditEventId::from_resource_id(resource_id(60_001)?),
                first_id,
                API_TOKEN_CREATED_AUDIT_ACTION,
                TEST_NOW,
            )?,
        })
        .await?;
    assert_eq!(created.version, 1);
    assert_eq!(created.name, "first");
    assert_eq!(repository.api_token_by_id(first_id).await?, created);
    let StoredApiToken { token, token_hash } =
        repository.api_token_by_prefix(&first_prefix).await?;
    assert_eq!(token, created);
    assert_eq!(format!("{token_hash:?}"), "ApiTokenHash([REDACTED])");

    let later = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 1);
    let second = new_api_token(60_010, "takt_8899aabbccddeeff", "second", later)?;
    let second_id = second.id;
    repository
        .create_api_token(CreateApiTokenPlan {
            token: second,
            audit_event: api_token_audit_event(
                AuditEventId::from_resource_id(resource_id(60_011)?),
                second_id,
                API_TOKEN_CREATED_AUDIT_ACTION,
                later,
            )?,
        })
        .await?;
    let page = repository
        .list_api_tokens(ApiTokenListQuery {
            organization_id: OrganizationId::from_resource_id(resource_id(1)?),
            project_id: None,
            kind: Some(ApiTokenKind::Personal),
            status: Some(ApiTokenStatus::Active),
            scope: Some("monitors:read".parse::<ApiTokenScope>()?),
            before: None,
            limit: 200,
            now: TEST_NOW,
        })
        .await?;
    assert_eq!(
        page.iter().map(|token| token.id).collect::<Vec<_>>(),
        vec![second_id, first_id]
    );
    let next = repository
        .list_api_tokens(ApiTokenListQuery {
            organization_id: OrganizationId::from_resource_id(resource_id(1)?),
            project_id: None,
            kind: None,
            status: None,
            scope: None,
            before: Some((later, second_id)),
            limit: 1,
            now: TEST_NOW,
        })
        .await?;
    assert_eq!(
        next.iter().map(|token| token.id).collect::<Vec<_>>(),
        vec![first_id]
    );

    let rollback = new_api_token(60_020, "takt_1020304050607080", "rollback", TEST_NOW)?;
    let rollback_id = rollback.id;
    assert_eq!(
        repository
            .create_api_token(CreateApiTokenPlan {
                token: rollback,
                audit_event: api_token_audit_event(
                    AuditEventId::from_resource_id(resource_id(60_001)?),
                    rollback_id,
                    API_TOKEN_CREATED_AUDIT_ACTION,
                    TEST_NOW,
                )?,
            })
            .await,
        Err(RepositoryError::AlreadyExists)
    );
    assert_eq!(
        repository.api_token_by_id(rollback_id).await,
        Err(RepositoryError::NotFound),
        "token insert must roll back with duplicate audit ID"
    );
    Ok(())
}

// PRD-API-003 / PRD-IAM-001 / PRD-IAM-005 / PRD-DATA-001 / PRD-DATA-002 /
// PRD-DATA-004 / PRD-NFR-002 / PRD-NFR-005: both engines execute this exact
// atomic Create/idempotency/replay contract.
pub async fn run_api_token_create_idempotency_contract(
    repository: &SqlxRepository,
) -> Result<(), Box<dyn Error>> {
    let organization_id = OrganizationId::from_resource_id(resource_id(1)?);
    let audit_before = repository
        .audit_events_for_organization(organization_id)
        .await?
        .len();
    let plan = idempotent_create_plan(
        61_000,
        "takt_1111222233334444",
        "idempotent",
        61_001,
        "create-key-primary",
        [0x11; 32],
        TEST_NOW,
    )?;
    let first = repository.create_api_token_idempotent(plan.clone()).await?;
    let first_replay = match first {
        ApiTokenCreateIdempotencyResult::Created { api_token, replay } => {
            assert_eq!((api_token.id, api_token.version), (plan.create.token.id, 1));
            replay
        }
        ApiTokenCreateIdempotencyResult::Replay(_) => {
            return Err("the first idempotent create unexpectedly replayed".into());
        }
    };
    let repeated = repository.create_api_token_idempotent(plan.clone()).await?;
    let repeated_replay = match repeated {
        ApiTokenCreateIdempotencyResult::Replay(replay) => replay,
        ApiTokenCreateIdempotencyResult::Created { .. } => {
            return Err("an identical request created the token twice".into());
        }
    };
    assert_eq!(first_replay, repeated_replay);
    assert_eq!(
        ApiTokenReplayCipher::new(1, [0x42; 32])?
            .decrypt(&plan.context, &repeated_replay.encrypted_replay)?
            .as_slice(),
        TEST_API_TOKEN_REPLAY_BODY
    );
    assert_eq!(
        repository
            .audit_events_for_organization(organization_id)
            .await?
            .len(),
        audit_before + 1,
        "an identical replay must not append a second audit event"
    );

    let conflicting = idempotent_create_plan(
        61_010,
        "takt_5555666677778888",
        "must-not-exist",
        61_011,
        "create-key-primary",
        [0x22; 32],
        TEST_NOW,
    )?;
    assert_eq!(
        repository
            .create_api_token_idempotent(conflicting.clone())
            .await,
        Err(ApiTokenCreateIdempotencyError::KeyReused)
    );
    assert_eq!(
        repository
            .api_token_by_id(conflicting.create.token.id)
            .await,
        Err(RepositoryError::NotFound)
    );
    assert_eq!(
        repository
            .audit_events_for_organization(organization_id)
            .await?
            .len(),
        audit_before + 1,
        "a request-hash conflict must have no audit effect"
    );

    let rollback = idempotent_create_plan(
        61_020,
        "takt_9999aaaabbbbcccc",
        "rollback",
        60_001,
        "create-key-rollback",
        [0x33; 32],
        TEST_NOW,
    )?;
    assert_eq!(
        repository
            .create_api_token_idempotent(rollback.clone())
            .await,
        Err(ApiTokenCreateIdempotencyError::Repository(
            RepositoryError::AlreadyExists
        ))
    );
    assert_eq!(
        repository.api_token_by_id(rollback.create.token.id).await,
        Err(RepositoryError::NotFound),
        "token and idempotency reservation must roll back with the audit failure"
    );
    let retry = idempotent_create_plan(
        61_020,
        "takt_9999aaaabbbbcccc",
        "rollback",
        61_021,
        "create-key-rollback",
        [0x33; 32],
        TEST_NOW,
    )?;
    assert!(matches!(
        repository.create_api_token_idempotent(retry).await?,
        ApiTokenCreateIdempotencyResult::Created { .. }
    ));

    let concurrent = idempotent_create_plan(
        61_030,
        "takt_ddddeeeeffff0000",
        "concurrent",
        61_031,
        "create-key-concurrent",
        [0x44; 32],
        TEST_NOW,
    )?;
    let (left, right) = tokio::join!(
        repository.create_api_token_idempotent(concurrent.clone()),
        repository.create_api_token_idempotent(concurrent)
    );
    assert!(
        matches!(
            (&left, &right),
            (
                Ok(ApiTokenCreateIdempotencyResult::Created { .. }),
                Ok(ApiTokenCreateIdempotencyResult::Replay(_))
            )
        ) || matches!(
            (&left, &right),
            (
                Ok(ApiTokenCreateIdempotencyResult::Replay(_)),
                Ok(ApiTokenCreateIdempotencyResult::Created { .. })
            )
        ),
        "exactly one concurrent request must create and the other must replay: {left:?} / {right:?}"
    );

    let expiring = idempotent_create_plan(
        61_040,
        "takt_0102030405060708",
        "expiring",
        61_041,
        "create-key-expiring",
        [0x55; 32],
        TEST_NOW,
    )?;
    assert!(matches!(
        repository.create_api_token_idempotent(expiring).await?,
        ApiTokenCreateIdempotencyResult::Created { .. }
    ));
    let after_window = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 86_400_000_000);
    let replacement = idempotent_create_plan(
        61_050,
        "takt_1112131415161718",
        "after-window",
        61_051,
        "create-key-expiring",
        [0x55; 32],
        after_window,
    )?;
    assert!(matches!(
        repository.create_api_token_idempotent(replacement).await?,
        ApiTokenCreateIdempotencyResult::Created { .. }
    ));
    assert!(
        repository
            .purge_expired_api_token_idempotency(after_window, 200)
            .await?
            >= 3,
        "bounded cleanup must remove expired replay rows"
    );
    assert_eq!(
        repository
            .purge_expired_api_token_idempotency(after_window, 0)
            .await,
        Err(RepositoryError::ConstraintViolation)
    );
    Ok(())
}

// PRD-API-003 / PRD-IAM-005 / PRD-DATA-001 / PRD-DATA-002 / PRD-DATA-004 /
// PRD-NFR-002 / PRD-NFR-005: both engines execute this exact atomic Patch
// idempotency, replay, conflict, rollback, expiry and concurrency contract.
pub async fn run_api_token_patch_idempotency_contract(
    repository: &SqlxRepository,
) -> Result<(), Box<dyn Error>> {
    let organization_id = OrganizationId::from_resource_id(resource_id(1)?);
    let primary_id = create_mutation_token(
        repository,
        62_000,
        "takt_1011121314151617",
        "patch-primary",
        62_001,
    )
    .await?;
    let audit_before = repository
        .audit_events_for_organization(organization_id)
        .await?
        .len();
    let patch_time = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 100);
    let primary = idempotent_update_plan(
        primary_id,
        1,
        "patch-applied",
        62_002,
        "patch-key-primary",
        [0x11; 32],
        patch_time,
    )?;
    let stored_result = match repository
        .update_api_token_idempotent(primary.clone())
        .await?
    {
        ApiTokenMutationIdempotencyResult::Mutated { api_token, result } => {
            assert_eq!(
                (api_token.name.as_str(), api_token.version),
                ("patch-applied", 2)
            );
            result
        }
        ApiTokenMutationIdempotencyResult::Replay(_) => {
            return Err("the first idempotent patch unexpectedly replayed".into());
        }
    };
    assert_eq!(
        repository.update_api_token_idempotent(primary).await?,
        ApiTokenMutationIdempotencyResult::Replay(stored_result)
    );
    let conflicting = idempotent_update_plan(
        primary_id,
        1,
        "must-not-apply",
        62_003,
        "patch-key-primary",
        [0x22; 32],
        patch_time,
    )?;
    assert_eq!(
        repository.update_api_token_idempotent(conflicting).await,
        Err(ApiTokenMutationIdempotencyError::KeyReused)
    );
    assert_eq!(
        repository.api_token_by_id(primary_id).await?.name,
        "patch-applied"
    );
    assert_eq!(
        repository
            .audit_events_for_organization(organization_id)
            .await?
            .len(),
        audit_before + 1,
        "patch replay and hash conflict must not append audit events"
    );

    let rollback_id = create_mutation_token(
        repository,
        62_010,
        "takt_2021222324252627",
        "patch-rollback",
        62_011,
    )
    .await?;
    let rollback_time = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 200);
    let rollback = idempotent_update_plan(
        rollback_id,
        1,
        "must-roll-back",
        62_001,
        "patch-key-rollback",
        [0x33; 32],
        rollback_time,
    )?;
    assert_eq!(
        repository.update_api_token_idempotent(rollback).await,
        Err(ApiTokenMutationIdempotencyError::Repository(
            RepositoryError::AlreadyExists
        ))
    );
    let rolled_back = repository.api_token_by_id(rollback_id).await?;
    assert_eq!(
        (rolled_back.name.as_str(), rolled_back.version),
        ("patch-rollback", 1),
        "patch and reservation must roll back with the failed audit"
    );
    assert!(matches!(
        repository
            .update_api_token_idempotent(idempotent_update_plan(
                rollback_id,
                1,
                "patch-after-rollback",
                62_012,
                "patch-key-rollback",
                [0x33; 32],
                rollback_time,
            )?)
            .await?,
        ApiTokenMutationIdempotencyResult::Mutated { .. }
    ));

    let concurrent_id = create_mutation_token(
        repository,
        62_020,
        "takt_3031323334353637",
        "patch-concurrent",
        62_021,
    )
    .await?;
    let concurrent = idempotent_update_plan(
        concurrent_id,
        1,
        "patch-winner",
        62_022,
        "patch-key-concurrent",
        [0x44; 32],
        UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 300),
    )?;
    let (left, right) = tokio::join!(
        repository.update_api_token_idempotent(concurrent.clone()),
        repository.update_api_token_idempotent(concurrent)
    );
    assert!(
        matches!(
            (&left, &right),
            (
                Ok(ApiTokenMutationIdempotencyResult::Mutated { .. }),
                Ok(ApiTokenMutationIdempotencyResult::Replay(_))
            )
        ) || matches!(
            (&left, &right),
            (
                Ok(ApiTokenMutationIdempotencyResult::Replay(_)),
                Ok(ApiTokenMutationIdempotencyResult::Mutated { .. })
            )
        ),
        "exactly one concurrent patch must mutate and the other replay: {left:?} / {right:?}"
    );
    assert_eq!(repository.api_token_by_id(concurrent_id).await?.version, 2);

    let expiring_id = create_mutation_token(
        repository,
        62_030,
        "takt_4041424344454647",
        "patch-expiring",
        62_031,
    )
    .await?;
    let expiring_time = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 400);
    assert!(matches!(
        repository
            .update_api_token_idempotent(idempotent_update_plan(
                expiring_id,
                1,
                "patch-before-expiry",
                62_032,
                "patch-key-expiring",
                [0x55; 32],
                expiring_time,
            )?)
            .await?,
        ApiTokenMutationIdempotencyResult::Mutated { .. }
    ));
    let after_window = UtcTimestamp::from_unix_micros(expiring_time.unix_micros() + 86_400_000_000);
    assert!(matches!(
        repository
            .update_api_token_idempotent(idempotent_update_plan(
                expiring_id,
                2,
                "patch-after-expiry",
                62_033,
                "patch-key-expiring",
                [0x66; 32],
                after_window,
            )?)
            .await?,
        ApiTokenMutationIdempotencyResult::Mutated { .. }
    ));

    Ok(())
}

pub async fn run_api_token_lifecycle_contract(
    repository: &SqlxRepository,
) -> Result<(), Box<dyn Error>> {
    let first_id = ApiTokenId::from_resource_id(resource_id(60_000)?);
    let second_id = ApiTokenId::from_resource_id(resource_id(60_010)?);
    let updated_at = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 2);
    let updated = repository
        .update_api_token(UpdateApiTokenPlan {
            id: first_id,
            expected_version: 1,
            patch: ApiTokenPatch {
                name: Some("renamed".to_owned()),
                expires_at: None,
                ip_networks: Some(vec![]),
            },
            now: updated_at,
            audit_event: api_token_audit_event(
                AuditEventId::from_resource_id(resource_id(60_002)?),
                first_id,
                API_TOKEN_UPDATED_AUDIT_ACTION,
                updated_at,
            )?,
        })
        .await?;
    assert_eq!((updated.name.as_str(), updated.version), ("renamed", 2));
    assert!(updated.ip_networks.is_empty());
    assert_eq!(
        repository
            .update_api_token(UpdateApiTokenPlan {
                id: first_id,
                expected_version: 1,
                patch: ApiTokenPatch {
                    name: Some("stale".to_owned()),
                    expires_at: None,
                    ip_networks: None,
                },
                now: updated_at,
                audit_event: api_token_audit_event(
                    AuditEventId::from_resource_id(resource_id(60_003)?),
                    first_id,
                    API_TOKEN_UPDATED_AUDIT_ACTION,
                    updated_at,
                )?,
            })
            .await,
        Err(RepositoryError::VersionConflict)
    );

    let rollback_time = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 3);
    assert_eq!(
        repository
            .update_api_token(UpdateApiTokenPlan {
                id: second_id,
                expected_version: 1,
                patch: ApiTokenPatch {
                    name: Some("must-roll-back".to_owned()),
                    expires_at: None,
                    ip_networks: None,
                },
                now: rollback_time,
                audit_event: api_token_audit_event(
                    AuditEventId::from_resource_id(resource_id(60_001)?),
                    second_id,
                    API_TOKEN_UPDATED_AUDIT_ACTION,
                    rollback_time,
                )?,
            })
            .await,
        Err(RepositoryError::AlreadyExists)
    );
    assert_eq!(repository.api_token_by_id(second_id).await?.name, "second");

    repository
        .record_api_token_used(second_id, rollback_time)
        .await?;
    let used = repository.api_token_by_id(second_id).await?;
    assert_eq!((used.last_used_at, used.version), (Some(rollback_time), 2));
    assert_eq!(
        repository
            .record_api_token_used(second_id, rollback_time)
            .await,
        Err(RepositoryError::VersionConflict)
    );
    assert_eq!(
        repository
            .revoke_api_token(RevokeApiTokenPlan {
                id: second_id,
                expected_version: 2,
                now: rollback_time,
                audit_event: api_token_audit_event(
                    AuditEventId::from_resource_id(resource_id(60_001)?),
                    second_id,
                    API_TOKEN_REVOKED_AUDIT_ACTION,
                    rollback_time,
                )?,
            })
            .await,
        Err(RepositoryError::AlreadyExists)
    );
    assert_eq!(
        repository.api_token_by_id(second_id).await?.revoked_at,
        None
    );

    let revoked_at = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 4);
    let revoked = repository
        .revoke_api_token(RevokeApiTokenPlan {
            id: first_id,
            expected_version: 2,
            now: revoked_at,
            audit_event: api_token_audit_event(
                AuditEventId::from_resource_id(resource_id(60_004)?),
                first_id,
                API_TOKEN_REVOKED_AUDIT_ACTION,
                revoked_at,
            )?,
        })
        .await?;
    assert_eq!(
        (revoked.status(revoked_at), revoked.version),
        (ApiTokenStatus::Revoked, 3)
    );
    assert_eq!(
        repository.record_api_token_used(first_id, revoked_at).await,
        Err(RepositoryError::VersionConflict)
    );
    let replay_at = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 5);
    assert_eq!(
        repository
            .revoke_api_token(RevokeApiTokenPlan {
                id: first_id,
                expected_version: 3,
                now: replay_at,
                audit_event: api_token_audit_event(
                    AuditEventId::from_resource_id(resource_id(60_005)?),
                    first_id,
                    API_TOKEN_REVOKED_AUDIT_ACTION,
                    replay_at,
                )?,
            })
            .await,
        Err(RepositoryError::VersionConflict)
    );
    let after_expiry = UtcTimestamp::from_unix_micros(TEST_NOW.unix_micros() + 3_600_000_001);
    assert_eq!(
        repository
            .record_api_token_used(second_id, after_expiry)
            .await,
        Err(RepositoryError::VersionConflict)
    );
    assert_eq!(
        repository
            .update_api_token(UpdateApiTokenPlan {
                id: second_id,
                expected_version: 2,
                patch: ApiTokenPatch {
                    name: Some("expired-must-not-change".to_owned()),
                    expires_at: Some(None),
                    ip_networks: None,
                },
                now: after_expiry,
                audit_event: api_token_audit_event(
                    AuditEventId::from_resource_id(resource_id(60_006)?),
                    second_id,
                    API_TOKEN_UPDATED_AUDIT_ACTION,
                    after_expiry,
                )?,
            })
            .await,
        Err(RepositoryError::VersionConflict),
        "an expired token must not be patched back into an active state"
    );
    assert_eq!(
        repository
            .revoke_api_token(RevokeApiTokenPlan {
                id: second_id,
                expected_version: 2,
                now: after_expiry,
                audit_event: api_token_audit_event(
                    AuditEventId::from_resource_id(resource_id(60_007)?),
                    second_id,
                    API_TOKEN_REVOKED_AUDIT_ACTION,
                    after_expiry,
                )?,
            })
            .await,
        Err(RepositoryError::VersionConflict),
        "an expired token must not be mutated during revoke"
    );
    assert_eq!(
        repository
            .audit_events_for_organization(OrganizationId::from_resource_id(resource_id(1)?))
            .await?
            .iter()
            .filter(|event| event.resource_type == "api_token")
            .count(),
        4,
        "failed, stale and replayed writes must not append audit events"
    );
    Ok(())
}

// PRD-IAM-001 / PRD-IAM-004 / PRD-IAM-005: both engines execute this exact
// framework-free authentication, rotation, CSRF, expiry and audit contract.
pub async fn run_browser_authentication_contract(
    repository: &SqlxRepository,
) -> Result<(), Box<dyn Error>> {
    let hasher = TestPasswordHasher::new();
    let dummy = hasher.hash("dummy credential password").await?;
    let ids = SequenceIds::new(50_000);
    let clock = MutableClock::new(TEST_NOW);
    let service = BrowserAuthenticationService::new(
        repository,
        &hasher,
        &clock,
        &ids,
        &ids,
        dummy,
        SessionPolicy::default(),
    );
    let request_id = OperationId::from_resource_id(resource_id(50_100)?);
    let username = "contract.admin";
    let password = TEST_REPLACEMENT_PASSWORD;
    assert_eq!(
        service.login("missing.user", password, request_id).await,
        Err(AuthenticationError::InvalidCredentials)
    );
    assert_eq!(
        service
            .login(username, "wrong horse battery", request_id)
            .await,
        Err(AuthenticationError::InvalidCredentials)
    );

    let login = service.login(username, password, request_id).await?;
    let session_token = login.session_token.expose_to_client().to_owned();
    let login_csrf = login
        .authentication
        .csrf_token
        .expose_to_client()
        .to_owned();
    assert_ne!(session_token, login_csrf);

    let current = service.current_session(&session_token).await?;
    let current_csrf = current.csrf_token.expose_to_client().to_owned();
    assert_ne!(login_csrf, current_csrf);
    assert_eq!(
        service
            .logout(&session_token, &login_csrf, request_id)
            .await,
        Err(AuthenticationError::CsrfFailed)
    );
    assert_eq!(
        repository
            .session_by_token_digest(&TokenDigest::from_raw_token(&session_token)?)
            .await?
            .revoked_at,
        None
    );
    service
        .logout(&session_token, &current_csrf, request_id)
        .await?;
    assert_eq!(
        service.current_session(&session_token).await,
        Err(AuthenticationError::Unauthenticated)
    );

    let rotated = service.login(username, password, request_id).await?;
    assert_ne!(rotated.session_token.expose_to_client(), session_token);
    clock.set(rotated.authentication.session.window.expires_at());
    assert_eq!(
        service
            .current_session(rotated.session_token.expose_to_client())
            .await,
        Err(AuthenticationError::Unauthenticated)
    );
    let organization_id = rotated.authentication.session.organization_id;
    let audit = format!(
        "{:?}",
        repository
            .audit_events_for_organization(organization_id)
            .await?
    );
    for secret in [
        TEST_REPLACEMENT_PASSWORD,
        session_token.as_str(),
        login_csrf.as_str(),
        current_csrf.as_str(),
    ] {
        assert!(!audit.contains(secret));
    }
    Ok(())
}

pub async fn run_bootstrap_contract(
    repository: &SqlxRepository,
) -> Result<takt_application::BootstrapOutput, Box<dyn Error>> {
    let hasher = TestPasswordHasher::new();
    let clock = FixedClock;
    let ids = SequenceIds::new(100);
    let service = BootstrapService::new(repository, &hasher, &clock, &ids);

    let first = service.execute("  Admin  ", TEST_PASSWORD).await?;
    assert_eq!(first.status, BootstrapStatus::Created);
    assert_eq!(first.resources.organization.slug, "default");
    assert_eq!(first.resources.project.slug, "default");
    assert_eq!(first.resources.user.normalized_username, "admin");
    assert_eq!(first.resources.membership.role, Role::Owner);
    assert_eq!(first.resources.membership.project_id, None);
    assert_eq!(
        first.resources.audit_event.actor_type,
        AuditActorType::LocalCli
    );
    assert_eq!(first.resources.audit_event.occurred_at, TEST_NOW);

    let repeated = service.execute("admin", TEST_PASSWORD).await?;
    assert_eq!(repeated.status, BootstrapStatus::AlreadyPresent);
    assert_eq!(repeated.resources, first.resources);

    assert_eq!(
        service.execute("another-admin", TEST_PASSWORD).await,
        Err(ApplicationError::Conflict)
    );
    assert_eq!(
        service.execute("admin", "different safe password").await,
        Err(ApplicationError::Conflict)
    );
    Ok(first)
}

// PRD-DATA-001 / PRD-NFR-002: both engines expose the same controlled-shutdown
// behavior through the shared repository boundary.
pub async fn run_shutdown_contract(database: Database) -> Result<(), Box<dyn Error>> {
    let repository = SqlxRepository::new(database.clone());
    database.close().await?;
    assert_eq!(
        database.readiness().await,
        Err(ReadinessError::SchemaNotReady)
    );
    assert_eq!(
        repository.organization_by_slug("after-shutdown").await,
        Err(RepositoryError::DatabaseUnavailable)
    );
    Ok(())
}
