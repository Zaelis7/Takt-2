#![forbid(unsafe_code)]

use std::{
    error::Error,
    sync::atomic::{AtomicU64, Ordering},
};

use takt_application::{
    ApplicationError, AuditRepository, BootstrapService, BootstrapStatus, Clock, CreateSessionPlan,
    IdGenerator, LocalUserRepository, MembershipRepository, NewAuditEvent, NewBrowserSession,
    NewLocalUser, NewMembership, NewOrganization, NewProject, OrganizationRepository, PasswordHash,
    PasswordHasher, PasswordHashing, ProjectRepository, RepositoryError, RevokeSessionPlan,
    SESSION_CREATED_AUDIT_ACTION, SESSION_REVOKED_AUDIT_ACTION, SessionRepository, TokenDigest,
    ValidationError,
};
use takt_domain::{
    AuditActorType, AuditEvent, AuditEventId, BootstrapAuditMetadata, MembershipId, OperationId,
    OrganizationId, ProjectId, ResourceId, Role, SessionId, UserId, UtcTimestamp,
    session::{SessionPolicy, SessionWindow},
};
use takt_persistence::{Database, ReadinessError, SqlxRepository};

pub const TEST_NOW: UtcTimestamp = UtcTimestamp::from_unix_micros(1_784_445_600_123_456);
pub const TEST_PASSWORD: &str = "correct horse battery";
pub const TEST_RAW_SESSION_TOKEN: &str = "raw-session-token-must-never-be-stored";
pub const TEST_RAW_CSRF_TOKEN: &str = "raw-csrf-token-must-never-be-stored";
pub const TEST_SESSION_TOKEN_SHA256: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const TEST_CSRF_TOKEN_SHA256: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

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
    assert!(!stored.contains(TEST_RAW_SESSION_TOKEN) && !stored.contains(TEST_RAW_CSRF_TOKEN));
}

pub struct FixedClock;

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
