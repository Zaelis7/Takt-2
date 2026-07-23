#![forbid(unsafe_code)]

use std::{
    error::Error,
    net::IpAddr,
    str::FromStr,
    sync::{
        Mutex,
        atomic::{AtomicI64, Ordering},
    },
};

use async_trait::async_trait;
use takt_application::api_token::{
    API_TOKEN_CREATED_AUDIT_ACTION, API_TOKEN_REVOKED_AUDIT_ACTION, API_TOKEN_UPDATED_AUDIT_ACTION,
    ApiTokenApplicationError, ApiTokenBearerAuthenticationService, ApiTokenCreateCommand,
    ApiTokenCreateIdempotencyError, ApiTokenCreateIdempotencyRepository,
    ApiTokenCreateIdempotencyResult, ApiTokenHasher, ApiTokenHashing, ApiTokenIdempotencyContext,
    ApiTokenIdempotencyInput, ApiTokenIdempotentCreateCommand, ApiTokenIdempotentWriteService,
    ApiTokenListQuery, ApiTokenManagementActor, ApiTokenManagementPermission,
    ApiTokenManagementService, ApiTokenPatch, ApiTokenReadActor, ApiTokenReadService,
    ApiTokenReplayCipher, ApiTokenRevokeCommand, ApiTokenSecret, ApiTokenSecretGenerator,
    ApiTokenStore, ApiTokenTarget, ApiTokenUpdateCommand, ApiTokenWriteActor, ApiTokenWriteMethod,
    CreateApiTokenIdempotencyPlan, CreateApiTokenPlan, EncryptedApiTokenReplay, RevokeApiTokenPlan,
    StoredApiToken, StoredApiTokenCreateReplay, TokenSecretGenerator, UpdateApiTokenPlan,
    authorize_token_actor, validate_new_api_token,
};
use takt_application::{ApplicationError, Argon2idConfig, Clock, RepositoryError, UuidV7Generator};
use takt_domain::{
    ApiTokenId, AuditActorId, AuditActorType, AuditEvent, AuditMetadata, MembershipId, OperationId,
    OrganizationId, ProjectId, ResourceId, UserId, UtcTimestamp,
    api_token::{
        ApiToken, ApiTokenKind, ApiTokenPrefix, ApiTokenScope, ApiTokenStatus, IpNetwork,
        TokenActor,
    },
};
use uuid::Uuid;

fn resource_id(value: &str) -> Result<ResourceId, Box<dyn Error>> {
    Ok(ResourceId::from_uuid(Uuid::parse_str(value)?)?)
}

const NOW: UtcTimestamp = UtcTimestamp::from_unix_micros(1_784_445_600_123_456);

struct TestClock(UtcTimestamp);

impl TestClock {
    const fn new(now: UtcTimestamp) -> Self {
        Self(now)
    }
}

impl Clock for TestClock {
    fn now(&self) -> Result<UtcTimestamp, ApplicationError> {
        Ok(self.0)
    }
}

struct MutableClock(AtomicI64);

impl MutableClock {
    const fn new(now: UtcTimestamp) -> Self {
        Self(AtomicI64::new(now.unix_micros()))
    }

    fn set(&self, now: UtcTimestamp) {
        self.0.store(now.unix_micros(), Ordering::SeqCst);
    }
}

impl Clock for MutableClock {
    fn now(&self) -> Result<UtcTimestamp, ApplicationError> {
        Ok(UtcTimestamp::from_unix_micros(
            self.0.load(Ordering::SeqCst),
        ))
    }
}

struct TestHashing(ApiTokenHasher);

impl TestHashing {
    fn new() -> Self {
        Self(ApiTokenHasher::new(Argon2idConfig::testing()))
    }
}

#[async_trait]
impl ApiTokenHashing for TestHashing {
    async fn hash(
        &self,
        secret: &ApiTokenSecret,
    ) -> Result<
        takt_application::api_token::ApiTokenHash,
        takt_application::api_token::ApiTokenApplicationError,
    > {
        self.0.hash(secret)
    }

    async fn verify(
        &self,
        secret: &ApiTokenSecret,
        hash: &takt_application::api_token::ApiTokenHash,
    ) -> Result<bool, takt_application::api_token::ApiTokenApplicationError> {
        self.0.verify(secret, hash)
    }
}

#[derive(Default)]
struct TestRepositoryState {
    stored: Option<StoredApiToken>,
    audits: Vec<AuditEvent>,
    create_idempotency: Option<(ApiTokenIdempotencyContext, StoredApiTokenCreateReplay)>,
}

#[derive(Default)]
struct TestRepository(Mutex<TestRepositoryState>);

impl TestRepository {
    fn audit_actions(&self) -> Vec<String> {
        self.0
            .lock()
            .expect("test repository lock")
            .audits
            .iter()
            .map(|event| event.action.clone())
            .collect()
    }

    fn mutate_token(&self, change: impl FnOnce(&mut ApiToken)) {
        if let Ok(mut state) = self.0.lock()
            && let Some(stored) = state.stored.as_mut()
        {
            change(&mut stored.token);
        }
    }

    fn last_used_at(&self) -> Option<UtcTimestamp> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.stored.as_ref()?.token.last_used_at)
    }

    fn audits(&self) -> Vec<AuditEvent> {
        self.0
            .lock()
            .map(|state| state.audits.clone())
            .unwrap_or_default()
    }
}

fn projection(plan: &CreateApiTokenPlan) -> ApiToken {
    ApiToken {
        id: plan.token.id,
        organization_id: plan.token.organization_id,
        project_id: plan.token.project_id,
        name: plan.token.name.clone(),
        kind: plan.token.kind,
        token_prefix: plan.token.token_prefix.clone(),
        scopes: plan.token.scopes.clone(),
        ip_networks: plan.token.ip_networks.clone(),
        expires_at: plan.token.expires_at,
        last_used_at: None,
        revoked_at: None,
        created_at: plan.token.now,
        updated_at: plan.token.now,
        version: 1,
    }
}

#[async_trait]
impl ApiTokenStore for TestRepository {
    async fn create_api_token(
        &self,
        plan: CreateApiTokenPlan,
    ) -> Result<ApiToken, RepositoryError> {
        let token = projection(&plan);
        let mut state = self
            .0
            .lock()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
        if state.stored.is_some() {
            return Err(RepositoryError::AlreadyExists);
        }
        state.audits.push(plan.audit_event.event);
        state.stored = Some(StoredApiToken {
            token: token.clone(),
            token_hash: plan.token.token_hash,
        });
        Ok(token)
    }

    async fn api_token_by_id(&self, id: ApiTokenId) -> Result<ApiToken, RepositoryError> {
        self.0
            .lock()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?
            .stored
            .as_ref()
            .filter(|stored| stored.token.id == id)
            .map(|stored| stored.token.clone())
            .ok_or(RepositoryError::NotFound)
    }

    async fn api_token_by_prefix(
        &self,
        prefix: &ApiTokenPrefix,
    ) -> Result<StoredApiToken, RepositoryError> {
        self.0
            .lock()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?
            .stored
            .as_ref()
            .filter(|stored| &stored.token.token_prefix == prefix)
            .cloned()
            .ok_or(RepositoryError::NotFound)
    }

    async fn list_api_tokens(
        &self,
        _query: ApiTokenListQuery,
    ) -> Result<Vec<ApiToken>, RepositoryError> {
        let state = self
            .0
            .lock()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
        Ok(state
            .stored
            .iter()
            .map(|stored| stored.token.clone())
            .collect())
    }
}

#[async_trait]
impl takt_application::api_token::ApiTokenLifecycleRepository for TestRepository {
    async fn update_api_token(
        &self,
        plan: UpdateApiTokenPlan,
    ) -> Result<ApiToken, RepositoryError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
        let stored = state.stored.as_mut().ok_or(RepositoryError::NotFound)?;
        if stored.token.id != plan.id || stored.token.version != plan.expected_version {
            return Err(RepositoryError::VersionConflict);
        }
        if let Some(name) = plan.patch.name {
            stored.token.name = name;
        }
        stored.token.updated_at = plan.now;
        stored.token.version += 1;
        let token = stored.token.clone();
        state.audits.push(plan.audit_event.event);
        Ok(token)
    }

    async fn revoke_api_token(
        &self,
        plan: RevokeApiTokenPlan,
    ) -> Result<ApiToken, RepositoryError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
        let stored = state.stored.as_mut().ok_or(RepositoryError::NotFound)?;
        if stored.token.id != plan.id || stored.token.version != plan.expected_version {
            return Err(RepositoryError::VersionConflict);
        }
        stored.token.revoked_at = Some(plan.now);
        stored.token.updated_at = plan.now;
        stored.token.version += 1;
        let token = stored.token.clone();
        state.audits.push(plan.audit_event.event);
        Ok(token)
    }

    async fn record_api_token_used(
        &self,
        id: ApiTokenId,
        now: UtcTimestamp,
    ) -> Result<(), RepositoryError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
        let token = &mut state
            .stored
            .as_mut()
            .filter(|stored| stored.token.id == id)
            .ok_or(RepositoryError::NotFound)?
            .token;
        if token.status(now) != ApiTokenStatus::Active
            || token.updated_at > now
            || token.last_used_at.is_some_and(|last_used| last_used >= now)
        {
            return Err(RepositoryError::VersionConflict);
        }
        token.last_used_at = Some(now);
        token.updated_at = now;
        token.version += 1;
        Ok(())
    }
}

fn same_idempotency_scope(
    stored: &ApiTokenIdempotencyContext,
    incoming: &ApiTokenIdempotencyContext,
) -> bool {
    stored.actor_type() == incoming.actor_type()
        && stored.actor_id() == incoming.actor_id()
        && stored.method() == incoming.method()
        && stored.path() == incoming.path()
        && stored.key() == incoming.key()
}

#[async_trait]
impl ApiTokenCreateIdempotencyRepository for TestRepository {
    async fn create_api_token_idempotent(
        &self,
        plan: CreateApiTokenIdempotencyPlan,
    ) -> Result<ApiTokenCreateIdempotencyResult, ApiTokenCreateIdempotencyError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
        if let Some((context, replay)) = &state.create_idempotency
            && same_idempotency_scope(context, &plan.context)
        {
            if context.request_hash() != plan.context.request_hash() {
                return Err(ApiTokenCreateIdempotencyError::KeyReused);
            }
            return Ok(ApiTokenCreateIdempotencyResult::Replay(replay.clone()));
        }
        if state.stored.is_some() {
            return Err(RepositoryError::AlreadyExists.into());
        }
        validate_new_api_token(&plan.create.token)
            .map_err(|_| RepositoryError::ConstraintViolation)?;
        let token = projection(&plan.create);
        let replay = StoredApiTokenCreateReplay {
            api_token_id: token.id,
            result_version: token.version,
            encrypted_replay: plan.encrypted_replay,
        };
        state.audits.push(plan.create.audit_event.event);
        state.stored = Some(StoredApiToken {
            token: token.clone(),
            token_hash: plan.create.token.token_hash,
        });
        state.create_idempotency = Some((plan.context, replay.clone()));
        Ok(ApiTokenCreateIdempotencyResult::Created {
            api_token: Box::new(token),
            replay,
        })
    }

    async fn purge_expired_api_token_idempotency(
        &self,
        _now: UtcTimestamp,
        _limit: u16,
    ) -> Result<u64, RepositoryError> {
        Ok(0)
    }
}

fn operation_id(value: &str) -> Result<OperationId, Box<dyn Error>> {
    Ok(OperationId::from_resource_id(resource_id(value)?))
}

fn management_actor(
    organization_id: OrganizationId,
    project_scope: Option<ProjectId>,
    audit_project_id: ProjectId,
    permissions: Vec<ApiTokenManagementPermission>,
) -> Result<ApiTokenManagementActor, Box<dyn Error>> {
    Ok(ApiTokenManagementActor::new(
        organization_id,
        project_scope,
        audit_project_id,
        UserId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000003")?),
        MembershipId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000004")?),
        permissions,
    )?)
}

// PRD-IAM-001 / PRD-IAM-004 / PRD-IAM-005: API-token administration is an
// application use case, not an HTTP- or repository-only authorization check.
#[tokio::test]
async fn prd_iam_001_api_token_crud_enforces_permission_context_and_audit()
-> Result<(), Box<dyn Error>> {
    let organization_id =
        OrganizationId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000101")?);
    let project_id =
        ProjectId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000102")?);
    let foreign_project_id =
        ProjectId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000103")?);
    let repository = TestRepository::default();
    let hashing = TestHashing::new();
    let clock = TestClock::new(NOW);
    let ids = UuidV7Generator;
    let secrets = ApiTokenSecretGenerator;
    let service = ApiTokenManagementService::new(&repository, &hashing, &clock, &ids, &secrets);
    let actor = management_actor(
        organization_id,
        Some(project_id),
        project_id,
        vec![
            ApiTokenManagementPermission::Read,
            ApiTokenManagementPermission::Write,
        ],
    )?;
    let create_request_id = operation_id("019b3cf0-0000-7000-8000-000000000104")?;
    let create = ApiTokenCreateCommand {
        organization_id,
        project_id: Some(project_id),
        name: "monitor reader".to_owned(),
        kind: ApiTokenKind::Personal,
        scopes: vec![ApiTokenScope::from_str("monitors:read")?],
        ip_networks: vec![IpNetwork::from_str("192.0.2.0/24")?],
        expires_at: Some(UtcTimestamp::from_unix_micros(
            NOW.unix_micros() + 1_000_000,
        )),
        request_id: create_request_id,
    };

    let created = service.create(&actor, create.clone()).await?;
    assert_eq!(created.api_token.version, 1);
    assert_eq!(created.audit_event.action, API_TOKEN_CREATED_AUDIT_ACTION);
    assert_eq!(created.audit_event.organization_id, organization_id);
    assert_eq!(created.audit_event.project_id, Some(project_id));
    assert_eq!(created.audit_event.request_id, create_request_id);
    assert!(!format!("{created:?}").contains(created.token.expose_once()));
    let target = ApiTokenTarget {
        id: created.api_token.id,
        organization_id,
        project_id: Some(project_id),
    };
    assert_eq!(service.get(&actor, target).await?, created.api_token);
    let list_query = ApiTokenListQuery {
        organization_id,
        project_id: Some(project_id),
        kind: None,
        status: Some(ApiTokenStatus::Active),
        scope: Some(ApiTokenScope::from_str("monitors:read")?),
        before: None,
        limit: 50,
        now: NOW,
    };
    assert_eq!(service.list(&actor, list_query.clone()).await?.len(), 1);

    let foreign_actor = management_actor(
        organization_id,
        Some(foreign_project_id),
        foreign_project_id,
        vec![
            ApiTokenManagementPermission::Read,
            ApiTokenManagementPermission::Write,
        ],
    )?;
    assert_eq!(
        service.get(&foreign_actor, target).await,
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    assert_eq!(
        service.list(&foreign_actor, list_query).await,
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    assert_eq!(
        service
            .get(
                &actor,
                ApiTokenTarget {
                    organization_id: OrganizationId::from_resource_id(resource_id(
                        "019b3cf0-0000-7000-8000-000000000107",
                    )?),
                    ..target
                },
            )
            .await,
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    let read_only = management_actor(
        organization_id,
        Some(project_id),
        project_id,
        vec![ApiTokenManagementPermission::Read],
    )?;
    assert!(matches!(
        service.create(&read_only, create).await,
        Err(ApiTokenApplicationError::PermissionDenied)
    ));

    let update = ApiTokenUpdateCommand {
        target,
        expected_version: 1,
        patch: ApiTokenPatch {
            name: Some("renamed reader".to_owned()),
            expires_at: None,
            ip_networks: None,
        },
        request_id: operation_id("019b3cf0-0000-7000-8000-000000000105")?,
    };
    assert_eq!(
        service.update(&foreign_actor, update.clone()).await,
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    let updated = service.update(&actor, update).await?;
    assert_eq!(
        (updated.name.as_str(), updated.version),
        ("renamed reader", 2)
    );
    let revoke = ApiTokenRevokeCommand {
        target,
        expected_version: 2,
        request_id: operation_id("019b3cf0-0000-7000-8000-000000000106")?,
    };
    assert_eq!(
        service.revoke(&read_only, revoke.clone()).await,
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    let revoked = service.revoke(&actor, revoke).await?;
    assert_eq!((revoked.revoked_at, revoked.version), (Some(NOW), 3));
    assert_eq!(
        repository.audit_actions(),
        vec![
            API_TOKEN_CREATED_AUDIT_ACTION,
            API_TOKEN_UPDATED_AUDIT_ACTION,
            API_TOKEN_REVOKED_AUDIT_ACTION,
        ]
    );
    Ok(())
}

fn idempotency(key: &str, request_hash: u8) -> ApiTokenIdempotencyInput {
    ApiTokenIdempotencyInput {
        key: key.to_owned(),
        request_hash: [request_hash; 32],
    }
}

// PRD-API-003 / PRD-API-005 / PRD-IAM-001 / PRD-IAM-004 / PRD-IAM-005:
// Browser and Bearer writes share the same actor-bound idempotency, context,
// audit and redaction boundary before HTTP is introduced.
#[tokio::test]
async fn prd_api_003_idempotent_api_token_create_binds_actor_context_and_secrets()
-> Result<(), Box<dyn Error>> {
    let organization_id =
        OrganizationId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000501")?);
    let project_id =
        ProjectId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000502")?);
    let foreign_project_id =
        ProjectId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000503")?);
    let repository = TestRepository::default();
    let hashing = TestHashing::new();
    let clock = MutableClock::new(NOW);
    let ids = UuidV7Generator;
    let secrets = ApiTokenSecretGenerator;
    let cipher = ApiTokenReplayCipher::new(1, [0x42; 32])?;
    let service =
        ApiTokenIdempotentWriteService::new(&repository, &hashing, &clock, &ids, &secrets, &cipher);
    let browser = ApiTokenWriteActor::from_browser_management(management_actor(
        organization_id,
        Some(project_id),
        project_id,
        vec![ApiTokenManagementPermission::Write],
    )?);
    let create = ApiTokenIdempotentCreateCommand {
        create: ApiTokenCreateCommand {
            organization_id,
            project_id: Some(project_id),
            name: "automation writer".to_owned(),
            kind: ApiTokenKind::Service,
            scopes: vec![ApiTokenScope::from_str("api_tokens:write")?],
            ip_networks: vec![IpNetwork::from_str("192.0.2.0/24")?],
            expires_at: Some(UtcTimestamp::from_unix_micros(NOW.unix_micros() + 500_000)),
            request_id: operation_id("019b3cf0-0000-7000-8000-000000000504")?,
        },
        idempotency: idempotency("create-key-0501", 0x11),
    };

    let browser_read_only = ApiTokenWriteActor::from_browser_management(management_actor(
        organization_id,
        Some(project_id),
        project_id,
        vec![ApiTokenManagementPermission::Read],
    )?);
    assert_eq!(
        service.create(&browser_read_only, create.clone()).await,
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    assert!(repository.audits().is_empty());

    let created = service.create(&browser, create.clone()).await?;
    clock.set(UtcTimestamp::from_unix_micros(
        NOW.unix_micros() + 1_000_000,
    ));
    let replayed = service.create(&browser, create.clone()).await?;
    assert!(!created.replayed);
    assert!(replayed.replayed);
    assert_eq!(created.api_token, replayed.api_token);
    assert_eq!(created.token.expose_once(), replayed.token.expose_once());
    assert!(!format!("{created:?}{replayed:?}").contains(created.token.expose_once()));
    assert_eq!(repository.audits().len(), 1);
    assert_eq!(repository.audits()[0].actor_type, AuditActorType::System);
    assert!(matches!(
        repository.audits()[0].actor_id,
        Some(AuditActorId::User(_))
    ));
    assert!(matches!(
        repository.audits()[0].metadata,
        AuditMetadata::LocalIdentity(_)
    ));

    let mut conflicting_create = create;
    conflicting_create.idempotency.request_hash = [0x12; 32];
    assert_eq!(
        service.create(&browser, conflicting_create).await,
        Err(ApiTokenApplicationError::IdempotencyKeyReused)
    );
    assert_eq!(repository.audits().len(), 1);

    let bearer_token_id =
        ApiTokenId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000506")?);
    let read_only = TokenActor::new(
        bearer_token_id,
        organization_id,
        Some(project_id),
        vec![ApiTokenScope::from_str("api_tokens:read")?],
    )?;
    assert_eq!(
        ApiTokenWriteActor::from_token_actor(&read_only),
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    let bearer = ApiTokenWriteActor::from_token_actor(&TokenActor::new(
        bearer_token_id,
        organization_id,
        Some(project_id),
        vec![ApiTokenScope::from_str("api_tokens:write")?],
    )?)?;
    let bearer_repository = TestRepository::default();
    let bearer_service = ApiTokenIdempotentWriteService::new(
        &bearer_repository,
        &hashing,
        &clock,
        &ids,
        &secrets,
        &cipher,
    );
    let bearer_create = ApiTokenIdempotentCreateCommand {
        create: ApiTokenCreateCommand {
            organization_id,
            project_id: Some(project_id),
            name: "bearer-created writer".to_owned(),
            kind: ApiTokenKind::Service,
            scopes: vec![ApiTokenScope::from_str("monitors:read")?],
            ip_networks: Vec::new(),
            expires_at: None,
            request_id: operation_id("019b3cf0-0000-7000-8000-000000000507")?,
        },
        idempotency: idempotency("create-key-0502", 0x31),
    };
    let mut foreign = bearer_create.clone();
    foreign.create.project_id = Some(foreign_project_id);
    assert_eq!(
        bearer_service.create(&bearer, foreign).await,
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    assert!(bearer_repository.audits().is_empty());

    let bearer_created = bearer_service
        .create(&bearer, bearer_create.clone())
        .await?;
    let bearer_replay = bearer_service.create(&bearer, bearer_create).await?;
    assert!(!bearer_created.replayed);
    assert!(bearer_replay.replayed);
    assert_eq!(
        bearer_created.token.expose_once(),
        bearer_replay.token.expose_once()
    );
    let audits = bearer_repository.audits();
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].actor_type, AuditActorType::ApiToken);
    assert_eq!(
        audits[0].actor_id,
        Some(AuditActorId::ApiToken(bearer_token_id))
    );
    assert!(matches!(
        audits[0].metadata,
        AuditMetadata::ApiToken(ref metadata)
            if metadata.api_token_id == bearer_token_id
    ));
    assert!(!format!("{audits:?}").contains(bearer_created.token.expose_once()));
    Ok(())
}

// PRD-IAM-001 / PRD-IAM-004: Bearer authentication verifies the slow hash and
// applies token status, source-IP, organization, project and exact-scope limits.
#[tokio::test]
async fn prd_iam_001_bearer_authentication_fails_closed_and_records_use()
-> Result<(), Box<dyn Error>> {
    let organization_id =
        OrganizationId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000201")?);
    let project_id =
        ProjectId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000202")?);
    let foreign_project_id =
        ProjectId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000203")?);
    let foreign_organization_id =
        OrganizationId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000205")?);
    let repository = TestRepository::default();
    let hashing = TestHashing::new();
    let clock = TestClock::new(NOW);
    let ids = UuidV7Generator;
    let secrets = ApiTokenSecretGenerator;
    let management = ApiTokenManagementService::new(&repository, &hashing, &clock, &ids, &secrets);
    let manager = management_actor(
        organization_id,
        Some(project_id),
        project_id,
        vec![ApiTokenManagementPermission::Write],
    )?;
    let created = management
        .create(
            &manager,
            ApiTokenCreateCommand {
                organization_id,
                project_id: Some(project_id),
                name: "monitor reader".to_owned(),
                kind: ApiTokenKind::Personal,
                scopes: vec![ApiTokenScope::from_str("monitors:read")?],
                ip_networks: vec![IpNetwork::from_str("192.0.2.0/24")?],
                expires_at: Some(UtcTimestamp::from_unix_micros(
                    NOW.unix_micros() + 1_000_000,
                )),
                request_id: operation_id("019b3cf0-0000-7000-8000-000000000204")?,
            },
        )
        .await?;
    let authentication = ApiTokenBearerAuthenticationService::new(&repository, &hashing, &clock);
    let allowed_source = IpAddr::from_str("192.0.2.10")?;
    let denied_source = IpAddr::from_str("198.51.100.10")?;

    assert_eq!(
        authentication
            .authenticate("not-a-token", allowed_source)
            .await,
        Err(ApiTokenApplicationError::AuthenticationFailed)
    );
    let unknown = secrets.generate()?;
    assert_eq!(
        authentication
            .authenticate(unknown.expose_once(), allowed_source)
            .await,
        Err(ApiTokenApplicationError::AuthenticationFailed)
    );
    let mut wrong_secret = created.token.expose_once().to_owned();
    let replacement = if wrong_secret.ends_with('0') {
        '1'
    } else {
        '0'
    };
    wrong_secret.pop();
    wrong_secret.push(replacement);
    assert_eq!(
        authentication
            .authenticate(&wrong_secret, allowed_source)
            .await,
        Err(ApiTokenApplicationError::AuthenticationFailed)
    );
    assert_eq!(
        authentication
            .authenticate(created.token.expose_once(), denied_source)
            .await,
        Err(ApiTokenApplicationError::AuthenticationFailed)
    );
    repository.mutate_token(|token| token.expires_at = Some(NOW));
    assert_eq!(
        authentication
            .authenticate(created.token.expose_once(), allowed_source)
            .await,
        Err(ApiTokenApplicationError::AuthenticationFailed)
    );
    repository.mutate_token(|token| {
        token.expires_at = None;
        token.revoked_at = Some(NOW);
    });
    assert_eq!(
        authentication
            .authenticate(created.token.expose_once(), allowed_source)
            .await,
        Err(ApiTokenApplicationError::AuthenticationFailed)
    );
    repository.mutate_token(|token| token.revoked_at = None);

    let actor = authentication
        .authenticate(created.token.expose_once(), allowed_source)
        .await?;
    let read = ApiTokenScope::from_str("monitors:read")?;
    assert_eq!(actor.token_id(), created.api_token.id);
    assert_eq!(repository.last_used_at(), Some(NOW));
    assert_eq!(
        authorize_token_actor(&actor, organization_id, Some(project_id), &read),
        Ok(())
    );
    for (target_project, required) in [
        (Some(project_id), ApiTokenScope::from_str("monitors:write")?),
        (Some(project_id), ApiTokenScope::from_str("checks:execute")?),
        (Some(foreign_project_id), read.clone()),
    ] {
        assert_eq!(
            authorize_token_actor(&actor, organization_id, target_project, &required),
            Err(ApiTokenApplicationError::PermissionDenied)
        );
    }
    assert_eq!(
        authorize_token_actor(&actor, foreign_organization_id, Some(project_id), &read,),
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    assert!(!format!("{actor:?}").contains(created.token.expose_once()));
    Ok(())
}

// PRD-API-004 / PRD-IAM-001 / PRD-IAM-004: the production read adapter must
// compose an exact api_tokens:read actor with context-checked List/Get use cases.
#[tokio::test]
async fn prd_iam_004_api_token_reads_require_exact_scope_and_context() -> Result<(), Box<dyn Error>>
{
    let organization_id =
        OrganizationId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000301")?);
    let project_id =
        ProjectId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000302")?);
    let foreign_project_id =
        ProjectId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000303")?);
    let repository = TestRepository::default();
    let hashing = TestHashing::new();
    let clock = TestClock::new(NOW);
    let ids = UuidV7Generator;
    let secrets = ApiTokenSecretGenerator;
    let management = ApiTokenManagementService::new(&repository, &hashing, &clock, &ids, &secrets);
    let manager = management_actor(
        organization_id,
        Some(project_id),
        project_id,
        vec![ApiTokenManagementPermission::Write],
    )?;
    let created = management
        .create(
            &manager,
            ApiTokenCreateCommand {
                organization_id,
                project_id: Some(project_id),
                name: "token metadata reader".to_owned(),
                kind: ApiTokenKind::Service,
                scopes: vec![ApiTokenScope::from_str("api_tokens:read")?],
                ip_networks: Vec::new(),
                expires_at: None,
                request_id: operation_id("019b3cf0-0000-7000-8000-000000000304")?,
            },
        )
        .await?;
    let token_actor = TokenActor::new(
        created.api_token.id,
        organization_id,
        Some(project_id),
        vec![ApiTokenScope::from_str("api_tokens:read")?],
    )?;
    let actor = ApiTokenReadActor::from_token_actor(&token_actor)?;
    let service = ApiTokenReadService::new(&repository, &clock);
    let query = ApiTokenListQuery {
        organization_id,
        project_id: Some(project_id),
        kind: None,
        status: None,
        scope: None,
        before: None,
        limit: 50,
        now: UtcTimestamp::from_unix_micros(0),
    };

    let page = service.list(&actor, query.clone()).await?;
    assert_eq!(page.items.len(), 1);
    assert!(!page.has_more);
    assert_eq!(page.items[0].token.id, created.api_token.id);
    assert_eq!(page.items[0].status, ApiTokenStatus::Active);
    assert_eq!(
        service.get(&actor, created.api_token.id).await?.token.id,
        created.api_token.id
    );

    let monitor_reader = TokenActor::new(
        created.api_token.id,
        organization_id,
        Some(project_id),
        vec![ApiTokenScope::from_str("monitors:read")?],
    )?;
    assert_eq!(
        ApiTokenReadActor::from_token_actor(&monitor_reader),
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    assert_eq!(
        service
            .list(
                &actor,
                ApiTokenListQuery {
                    project_id: Some(foreign_project_id),
                    ..query
                },
            )
            .await,
        Err(ApiTokenApplicationError::PermissionDenied)
    );
    Ok(())
}

#[test]
fn token_actor_grants_only_exact_scopes() -> Result<(), Box<dyn Error>> {
    let actor = TokenActor::new(
        ApiTokenId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000001")?),
        OrganizationId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000002")?),
        None,
        vec![ApiTokenScope::from_str("monitors:read")?],
    )?;

    assert!(actor.allows(&ApiTokenScope::from_str("monitors:read")?));
    assert!(!actor.allows(&ApiTokenScope::from_str("monitors:write")?));
    assert!(!actor.allows(&ApiTokenScope::from_str("checks:execute")?));
    assert!(ApiTokenScope::from_str("monitors:*").is_err());
    assert!(format!("{actor:?}").contains("monitors:read"));
    Ok(())
}

#[test]
fn cidrs_are_canonical_and_enforce_the_source_address() -> Result<(), Box<dyn Error>> {
    let v4 = IpNetwork::from_str("192.0.2.0/24")?;
    let v6 = IpNetwork::from_str("2001:db8::/32")?;
    assert!(v4.contains(IpAddr::from_str("192.0.2.7")?));
    assert!(!v4.contains(IpAddr::from_str("192.0.3.7")?));
    assert!(v6.contains(IpAddr::from_str("2001:db8::7")?));
    assert!(IpNetwork::from_str("192.0.2.7/24").is_err());
    assert!(IpNetwork::from_str("2001:db8::1/32").is_err());
    Ok(())
}

#[test]
fn status_and_ip_restrictions_are_evaluated_without_secrets() -> Result<(), Box<dyn Error>> {
    let token = ApiToken {
        id: ApiTokenId::from_resource_id(resource_id("019b3cf0-0000-7000-8000-000000000011")?),
        organization_id: OrganizationId::from_resource_id(resource_id(
            "019b3cf0-0000-7000-8000-000000000012",
        )?),
        project_id: None,
        name: "read-only".to_owned(),
        kind: ApiTokenKind::Personal,
        token_prefix: ApiTokenPrefix::from_str("takt_0011223344556677")?,
        scopes: vec![ApiTokenScope::from_str("monitors:read")?],
        ip_networks: vec![IpNetwork::from_str("192.0.2.0/24")?],
        expires_at: Some(UtcTimestamp::from_unix_micros(200)),
        last_used_at: None,
        revoked_at: None,
        created_at: UtcTimestamp::from_unix_micros(100),
        updated_at: UtcTimestamp::from_unix_micros(100),
        version: 1,
    };
    assert!(token.authorizes_source(
        UtcTimestamp::from_unix_micros(199),
        IpAddr::from_str("192.0.2.99")?
    ));
    assert!(!token.authorizes_source(
        UtcTimestamp::from_unix_micros(200),
        IpAddr::from_str("192.0.2.99")?
    ));
    assert!(!token.authorizes_source(
        UtcTimestamp::from_unix_micros(199),
        IpAddr::from_str("198.51.100.1")?
    ));
    Ok(())
}

#[test]
fn generated_tokens_and_slow_hashes_are_redacted() -> Result<(), Box<dyn Error>> {
    let generator = ApiTokenSecretGenerator;
    let first = generator.generate()?;
    let second = generator.generate()?;
    assert_ne!(first.expose_once(), second.expose_once());
    assert!(first.expose_once().starts_with(first.lookup_prefix()));
    assert_eq!(first.lookup_prefix().len(), 21);
    assert_eq!(first.secret_entropy_bits(), 256);
    assert_eq!(format!("{first:?}"), "ApiTokenSecret([REDACTED])");

    let hasher = ApiTokenHasher::new(Argon2idConfig::testing());
    let hash = hasher.hash(&first)?;
    assert_eq!(format!("{hash:?}"), "ApiTokenHash([REDACTED])");
    assert!(hash.expose_for_persistence().starts_with("$argon2id$"));
    assert!(!hash.expose_for_persistence().contains(first.expose_once()));
    assert!(hasher.verify(&first, &hash)?);
    let wrong = ApiTokenSecret::from_client_input(second.expose_once().to_owned())?;
    assert!(!hasher.verify(&wrong, &hash)?);
    Ok(())
}

#[test]
fn replay_encryption_binds_all_idempotency_dimensions_and_detects_tampering()
-> Result<(), Box<dyn Error>> {
    let actor = resource_id("019b3cf0-0000-7000-8000-000000000021")?;
    let now = UtcTimestamp::from_unix_micros(1_784_445_600_123_456);
    let context = ApiTokenIdempotencyContext::new(
        AuditActorType::System,
        actor,
        ApiTokenWriteMethod::Post,
        "/api/v1/api-tokens".to_owned(),
        "create-key-0001".to_owned(),
        [0x11; 32],
        now,
    )?;
    assert_eq!(
        context.expires_at().unix_micros() - now.unix_micros(),
        86_400_000_000
    );

    let cipher = ApiTokenReplayCipher::new(7, [0xa5; 32])?;
    let plaintext = br#"{"token":"fixture-replay-marker"}"#;
    let encrypted = cipher.encrypt(&context, plaintext)?;
    let encrypted_again = cipher.encrypt(&context, plaintext)?;
    assert_ne!(encrypted.ciphertext(), plaintext);
    assert_ne!(encrypted.nonce(), encrypted_again.nonce());
    assert_eq!(cipher.decrypt(&context, &encrypted)?.as_slice(), plaintext);
    let debug_output = format!("{context:?}{encrypted:?}{cipher:?}");
    assert!(!debug_output.contains("create-key-0001"));
    assert!(!debug_output.contains("fixture-replay-marker"));

    let mut tampered_bytes = encrypted.ciphertext().to_vec();
    tampered_bytes[0] ^= 1;
    let tampered = EncryptedApiTokenReplay::from_persistence(
        encrypted.key_version(),
        encrypted.nonce().to_vec(),
        tampered_bytes,
    )?;
    assert!(cipher.decrypt(&context, &tampered).is_err());

    for changed in [
        ApiTokenIdempotencyContext::new(
            AuditActorType::System,
            resource_id("019b3cf0-0000-7000-8000-000000000022")?,
            ApiTokenWriteMethod::Post,
            "/api/v1/api-tokens".to_owned(),
            "create-key-0001".to_owned(),
            [0x11; 32],
            now,
        )?,
        ApiTokenIdempotencyContext::new(
            AuditActorType::LocalCli,
            actor,
            ApiTokenWriteMethod::Post,
            "/api/v1/api-tokens".to_owned(),
            "create-key-0001".to_owned(),
            [0x11; 32],
            now,
        )?,
        ApiTokenIdempotencyContext::new(
            AuditActorType::System,
            actor,
            ApiTokenWriteMethod::Patch,
            "/api/v1/api-tokens/019b3cf0-0000-7000-8000-000000000099".to_owned(),
            "create-key-0001".to_owned(),
            [0x11; 32],
            now,
        )?,
        ApiTokenIdempotencyContext::new(
            AuditActorType::System,
            actor,
            ApiTokenWriteMethod::Post,
            "/api/v1/api-tokens".to_owned(),
            "create-key-0002".to_owned(),
            [0x11; 32],
            now,
        )?,
        ApiTokenIdempotencyContext::new(
            AuditActorType::System,
            actor,
            ApiTokenWriteMethod::Post,
            "/api/v1/api-tokens".to_owned(),
            "create-key-0001".to_owned(),
            [0x22; 32],
            now,
        )?,
    ] {
        assert!(cipher.decrypt(&changed, &encrypted).is_err());
    }

    let mutation_context = ApiTokenIdempotencyContext::new(
        AuditActorType::System,
        actor,
        ApiTokenWriteMethod::Patch,
        "/api/v1/api-tokens/019b3cf0-0000-7000-8000-000000000099".to_owned(),
        "mutation-key-0001".to_owned(),
        [0x33; 32],
        now,
    )?;
    let mutation_encrypted = cipher.encrypt(&mutation_context, plaintext)?;
    for changed in [
        ApiTokenIdempotencyContext::new(
            AuditActorType::System,
            actor,
            ApiTokenWriteMethod::Delete,
            mutation_context.path().to_owned(),
            "mutation-key-0001".to_owned(),
            [0x33; 32],
            now,
        )?,
        ApiTokenIdempotencyContext::new(
            AuditActorType::System,
            actor,
            ApiTokenWriteMethod::Patch,
            "/api/v1/api-tokens/019b3cf0-0000-7000-8000-000000000098".to_owned(),
            "mutation-key-0001".to_owned(),
            [0x33; 32],
            now,
        )?,
    ] {
        assert!(cipher.decrypt(&changed, &mutation_encrypted).is_err());
    }
    let wrong_key_version = EncryptedApiTokenReplay::from_persistence(
        encrypted.key_version() + 1,
        encrypted.nonce().to_vec(),
        encrypted.ciphertext().to_vec(),
    )?;
    assert!(cipher.decrypt(&context, &wrong_key_version).is_err());
    assert!(
        ApiTokenIdempotencyContext::new(
            AuditActorType::System,
            actor,
            ApiTokenWriteMethod::Post,
            "/api/v1/api-tokens".to_owned(),
            "short".to_owned(),
            [0; 32],
            now,
        )
        .is_err()
    );
    Ok(())
}
