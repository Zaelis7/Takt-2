#![forbid(unsafe_code)]

use std::{
    error::Error,
    sync::atomic::{AtomicU64, Ordering},
};

use takt_application::{
    ApplicationError, AuditRepository, BootstrapService, BootstrapStatus, Clock, IdGenerator,
    LocalUserRepository, MembershipRepository, NewAuditEvent, NewLocalUser, NewMembership,
    NewOrganization, NewProject, OrganizationRepository, PasswordHasher, ProjectRepository,
    RepositoryError,
};
use takt_domain::{
    AuditActorType, AuditEvent, AuditEventId, BootstrapAuditMetadata, MembershipId, OperationId,
    OrganizationId, ProjectId, ResourceId, Role, UserId, UtcTimestamp,
};
use takt_persistence::SqlxRepository;

pub const TEST_NOW: UtcTimestamp = UtcTimestamp::from_unix_micros(1_784_445_600_123_456);
pub const TEST_PASSWORD: &str = "correct horse battery";

pub struct FixedClock;

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
    assert_eq!(
        repository.organization_by_slug("missing").await,
        Err(RepositoryError::NotFound)
    );
    Ok(())
}

pub async fn run_bootstrap_contract(
    repository: &SqlxRepository,
) -> Result<takt_application::BootstrapOutput, Box<dyn Error>> {
    let hasher = PasswordHasher::new(takt_application::Argon2idConfig::testing());
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
