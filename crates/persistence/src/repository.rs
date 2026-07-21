use std::str::FromStr;

use async_trait::async_trait;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row, SqliteConnection, postgres::PgRow, sqlite::SqliteRow};
use takt_application::{
    AuditRepository, AuthenticationRepository, BootstrapPlan, BootstrapRepository,
    BootstrapResources, BootstrapStoreResult, CompleteRecoveryPlan, CreateRecoveryPlan,
    CreateSessionPlan, ExistingBootstrap, LocalAuthenticationContext, LocalUserRepository,
    MembershipRepository, NewAuditEvent, NewLocalUser, NewMembership, NewOrganization, NewProject,
    OrganizationRepository, PasswordHash, ProjectRepository, RECOVERY_COMPLETED_AUDIT_ACTION,
    RECOVERY_ISSUED_AUDIT_ACTION, RecoveryRepository, RepositoryError, RevokeSessionPlan,
    SESSION_CREATED_AUDIT_ACTION, SESSION_REVOKED_AUDIT_ACTION, SessionRepository, TokenDigest,
};
use takt_domain::{
    AuditActorType, AuditEvent, AuditEventId, BootstrapAuditMetadata, LocalUser, Membership,
    MembershipId, OperationId, Organization, OrganizationId, Project, ProjectId, RecoveryToken,
    RecoveryTokenId, ResourceId, Role, SessionId, UserId, UtcTimestamp,
    session::{BrowserSession, SessionWindow},
};
use time::OffsetDateTime;

use crate::database::{Database, DatabasePool};

#[derive(Clone)]
pub struct SqlxRepository {
    database: Database,
}

impl SqlxRepository {
    #[must_use]
    pub const fn new(database: Database) -> Self {
        Self { database }
    }

    #[must_use]
    pub const fn database(&self) -> &Database {
        &self.database
    }

    async fn session_by_id(&self, id: SessionId) -> Result<BrowserSession, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => session_from_postgres(
                &sqlx::query(
                    "SELECT id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version FROM sessions WHERE id = $1",
                )
                .bind(id.as_uuid())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => session_from_sqlite(
                &sqlx::query(
                    "SELECT id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version FROM sessions WHERE id = ?1",
                )
                .bind(id.to_string())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }
}

#[async_trait]
impl OrganizationRepository for SqlxRepository {
    async fn create_organization(
        &self,
        organization: NewOrganization,
    ) -> Result<Organization, RepositoryError> {
        validate_bounded_text(&organization.slug, 63)?;
        validate_bounded_text(&organization.name, 120)?;
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let now = postgres_time(organization.now)?;
                let row = sqlx::query(
                    "INSERT INTO organizations (id, slug, name, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $4, 1) RETURNING id, slug, name, created_at, updated_at, version",
                )
                .bind(organization.id.as_uuid())
                .bind(&organization.slug)
                .bind(&organization.name)
                .bind(now)
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?;
                organization_from_postgres(&row)
            }
            DatabasePool::Sqlite(pool) => {
                let row = sqlx::query(
                    "INSERT INTO organizations (id, slug, name, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?4, 1) RETURNING id, slug, name, created_at, updated_at, version",
                )
                .bind(organization.id.to_string())
                .bind(&organization.slug)
                .bind(&organization.name)
                .bind(organization.now.unix_micros())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?;
                organization_from_sqlite(&row)
            }
        }
    }

    async fn organization_by_id(
        &self,
        id: OrganizationId,
    ) -> Result<Organization, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => organization_from_postgres(
                &sqlx::query(
                    "SELECT id, slug, name, created_at, updated_at, version FROM organizations WHERE id = $1",
                )
                .bind(id.as_uuid())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => organization_from_sqlite(
                &sqlx::query(
                    "SELECT id, slug, name, created_at, updated_at, version FROM organizations WHERE id = ?1",
                )
                .bind(id.to_string())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn organization_by_slug(&self, slug: &str) -> Result<Organization, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => organization_from_postgres(
                &sqlx::query(
                    "SELECT id, slug, name, created_at, updated_at, version FROM organizations WHERE slug = $1",
                )
                .bind(slug)
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => organization_from_sqlite(
                &sqlx::query(
                    "SELECT id, slug, name, created_at, updated_at, version FROM organizations WHERE slug = ?1",
                )
                .bind(slug)
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn update_organization_name(
        &self,
        id: OrganizationId,
        expected_version: i64,
        name: &str,
        now: UtcTimestamp,
    ) -> Result<Organization, RepositoryError> {
        validate_bounded_text(name, 120)?;
        let updated = match &self.database.pool {
            DatabasePool::PostgreSql(pool) => sqlx::query(
                "UPDATE organizations SET name = $1, updated_at = $2, version = version + 1 WHERE id = $3 AND version = $4 RETURNING id, slug, name, created_at, updated_at, version",
            )
            .bind(name)
            .bind(postgres_time(now)?)
            .bind(id.as_uuid())
            .bind(expected_version)
            .fetch_optional(pool)
            .await
            .map_err(map_sqlx_error)?
            .map(|row| organization_from_postgres(&row))
            .transpose()?,
            DatabasePool::Sqlite(pool) => sqlx::query(
                "UPDATE organizations SET name = ?1, updated_at = ?2, version = version + 1 WHERE id = ?3 AND version = ?4 RETURNING id, slug, name, created_at, updated_at, version",
            )
            .bind(name)
            .bind(now.unix_micros())
            .bind(id.to_string())
            .bind(expected_version)
            .fetch_optional(pool)
            .await
            .map_err(map_sqlx_error)?
            .map(|row| organization_from_sqlite(&row))
            .transpose()?,
        };
        if let Some(organization) = updated {
            return Ok(organization);
        }
        match self.organization_by_id(id).await {
            Ok(_) => Err(RepositoryError::VersionConflict),
            Err(RepositoryError::NotFound) => Err(RepositoryError::NotFound),
            Err(error) => Err(error),
        }
    }
}

#[async_trait]
impl ProjectRepository for SqlxRepository {
    async fn create_project(&self, project: NewProject) -> Result<Project, RepositoryError> {
        validate_bounded_text(&project.slug, 63)?;
        validate_bounded_text(&project.name, 120)?;
        validate_bounded_text(&project.default_timezone, 100)?;
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => project_from_postgres(
                &sqlx::query(
                    "INSERT INTO projects (id, organization_id, slug, name, default_timezone, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $5, $6, $6, 1) RETURNING id, organization_id, slug, name, default_timezone, created_at, updated_at, version",
                )
                .bind(project.id.as_uuid())
                .bind(project.organization_id.as_uuid())
                .bind(&project.slug)
                .bind(&project.name)
                .bind(&project.default_timezone)
                .bind(postgres_time(project.now)?)
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => project_from_sqlite(
                &sqlx::query(
                    "INSERT INTO projects (id, organization_id, slug, name, default_timezone, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 1) RETURNING id, organization_id, slug, name, default_timezone, created_at, updated_at, version",
                )
                .bind(project.id.to_string())
                .bind(project.organization_id.to_string())
                .bind(&project.slug)
                .bind(&project.name)
                .bind(&project.default_timezone)
                .bind(project.now.unix_micros())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn project_by_id(&self, id: ProjectId) -> Result<Project, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => project_from_postgres(
                &sqlx::query(
                    "SELECT id, organization_id, slug, name, default_timezone, created_at, updated_at, version FROM projects WHERE id = $1",
                )
                .bind(id.as_uuid())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => project_from_sqlite(
                &sqlx::query(
                    "SELECT id, organization_id, slug, name, default_timezone, created_at, updated_at, version FROM projects WHERE id = ?1",
                )
                .bind(id.to_string())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn project_by_slug(
        &self,
        organization_id: OrganizationId,
        slug: &str,
    ) -> Result<Project, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => project_from_postgres(
                &sqlx::query(
                    "SELECT id, organization_id, slug, name, default_timezone, created_at, updated_at, version FROM projects WHERE organization_id = $1 AND slug = $2",
                )
                .bind(organization_id.as_uuid())
                .bind(slug)
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => project_from_sqlite(
                &sqlx::query(
                    "SELECT id, organization_id, slug, name, default_timezone, created_at, updated_at, version FROM projects WHERE organization_id = ?1 AND slug = ?2",
                )
                .bind(organization_id.to_string())
                .bind(slug)
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }
}

#[async_trait]
impl LocalUserRepository for SqlxRepository {
    async fn create_local_user(&self, user: NewLocalUser) -> Result<LocalUser, RepositoryError> {
        validate_bounded_text(&user.normalized_username, 64)?;
        validate_bounded_text(&user.display_name, 120)?;
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(
                    "INSERT INTO users (id, normalized_username, display_name, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $4, 1) RETURNING id, normalized_username, display_name, created_at, updated_at, version",
                )
                .bind(user.id.as_uuid())
                .bind(&user.normalized_username)
                .bind(&user.display_name)
                .bind(postgres_time(user.now)?)
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                sqlx::query(
                    "INSERT INTO local_credentials (user_id, password_hash, created_at, updated_at, version) VALUES ($1, $2, $3, $3, 1)",
                )
                .bind(user.id.as_uuid())
                .bind(user.password_hash.expose_for_persistence())
                .bind(postgres_time(user.now)?)
                .execute(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                let result = local_user_from_postgres(&row)?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(result)
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(
                    "INSERT INTO users (id, normalized_username, display_name, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?4, 1) RETURNING id, normalized_username, display_name, created_at, updated_at, version",
                )
                .bind(user.id.to_string())
                .bind(&user.normalized_username)
                .bind(&user.display_name)
                .bind(user.now.unix_micros())
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                sqlx::query(
                    "INSERT INTO local_credentials (user_id, password_hash, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?3, 1)",
                )
                .bind(user.id.to_string())
                .bind(user.password_hash.expose_for_persistence())
                .bind(user.now.unix_micros())
                .execute(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                let result = local_user_from_sqlite(&row)?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(result)
            }
        }
    }

    async fn local_user_by_username(
        &self,
        normalized_username: &str,
    ) -> Result<LocalUser, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => local_user_from_postgres(
                &sqlx::query(
                    "SELECT id, normalized_username, display_name, created_at, updated_at, version FROM users WHERE normalized_username = $1",
                )
                .bind(normalized_username)
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => local_user_from_sqlite(
                &sqlx::query(
                    "SELECT id, normalized_username, display_name, created_at, updated_at, version FROM users WHERE normalized_username = ?1",
                )
                .bind(normalized_username)
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }
}

#[async_trait]
impl MembershipRepository for SqlxRepository {
    async fn create_membership(
        &self,
        membership: NewMembership,
    ) -> Result<Membership, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => membership_from_postgres(
                &sqlx::query(
                    "INSERT INTO memberships (id, organization_id, project_id, user_id, role, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $5, $6, $6, 1) RETURNING id, organization_id, project_id, user_id, role, created_at, updated_at, version",
                )
                .bind(membership.id.as_uuid())
                .bind(membership.organization_id.as_uuid())
                .bind(membership.project_id.map(ProjectId::as_uuid))
                .bind(membership.user_id.as_uuid())
                .bind(membership.role.as_str())
                .bind(postgres_time(membership.now)?)
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => membership_from_sqlite(
                &sqlx::query(
                    "INSERT INTO memberships (id, organization_id, project_id, user_id, role, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 1) RETURNING id, organization_id, project_id, user_id, role, created_at, updated_at, version",
                )
                .bind(membership.id.to_string())
                .bind(membership.organization_id.to_string())
                .bind(membership.project_id.map(|id| id.to_string()))
                .bind(membership.user_id.to_string())
                .bind(membership.role.as_str())
                .bind(membership.now.unix_micros())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn membership_by_scope(
        &self,
        organization_id: OrganizationId,
        project_id: Option<ProjectId>,
        user_id: UserId,
    ) -> Result<Membership, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => membership_from_postgres(
                &sqlx::query(
                    "SELECT id, organization_id, project_id, user_id, role, created_at, updated_at, version FROM memberships WHERE organization_id = $1 AND project_id IS NOT DISTINCT FROM $2 AND user_id = $3",
                )
                .bind(organization_id.as_uuid())
                .bind(project_id.map(ProjectId::as_uuid))
                .bind(user_id.as_uuid())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => membership_from_sqlite(
                &sqlx::query(
                    "SELECT id, organization_id, project_id, user_id, role, created_at, updated_at, version FROM memberships WHERE organization_id = ?1 AND project_id IS ?2 AND user_id = ?3",
                )
                .bind(organization_id.to_string())
                .bind(project_id.map(|id| id.to_string()))
                .bind(user_id.to_string())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }
}

#[async_trait]
impl AuditRepository for SqlxRepository {
    async fn append_audit_event(
        &self,
        event: NewAuditEvent,
    ) -> Result<AuditEvent, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut connection = pool.acquire().await.map_err(map_sqlx_error)?;
                insert_audit_postgres(&mut connection, &event).await
            }
            DatabasePool::Sqlite(pool) => {
                let mut connection = pool.acquire().await.map_err(map_sqlx_error)?;
                insert_audit_sqlite(&mut connection, &event).await
            }
        }
    }

    async fn audit_event_by_id(&self, id: AuditEventId) -> Result<AuditEvent, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => audit_from_postgres(
                &sqlx::query(
                    "SELECT id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at FROM audit_events WHERE id = $1",
                )
                .bind(id.as_uuid())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => audit_from_sqlite(
                &sqlx::query(
                    "SELECT id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at FROM audit_events WHERE id = ?1",
                )
                .bind(id.to_string())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn audit_events_for_organization(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<AuditEvent>, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => sqlx::query(
                "SELECT id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at FROM audit_events WHERE organization_id = $1 ORDER BY occurred_at ASC, id ASC",
            )
            .bind(organization_id.as_uuid())
            .fetch_all(pool)
            .await
            .map_err(map_sqlx_error)?
            .iter()
            .map(audit_from_postgres)
            .collect(),
            DatabasePool::Sqlite(pool) => sqlx::query(
                "SELECT id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at FROM audit_events WHERE organization_id = ?1 ORDER BY occurred_at ASC, id ASC",
            )
            .bind(organization_id.to_string())
            .fetch_all(pool)
            .await
            .map_err(map_sqlx_error)?
            .iter()
            .map(audit_from_sqlite)
            .collect(),
        }
    }
}

#[async_trait]
impl SessionRepository for SqlxRepository {
    async fn create_session(
        &self,
        plan: CreateSessionPlan,
    ) -> Result<BrowserSession, RepositoryError> {
        validate_session_audit(
            plan.session.id,
            plan.session.organization_id,
            plan.session.user_id,
            SESSION_CREATED_AUDIT_ACTION,
            plan.session.window.issued_at(),
            &plan.audit_event,
        )?;
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(
                    "INSERT INTO sessions (id, organization_id, user_id, token_digest, csrf_digest, issued_at, last_activity_at, expires_at, absolute_expires_at, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $6, $6, 1) RETURNING id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version",
                )
                .bind(plan.session.id.as_uuid())
                .bind(plan.session.organization_id.as_uuid())
                .bind(plan.session.user_id.as_uuid())
                .bind(plan.session.token_digest.expose_for_persistence())
                .bind(plan.session.csrf_digest.expose_for_persistence())
                .bind(postgres_time(plan.session.window.issued_at())?)
                .bind(postgres_time(plan.session.window.last_activity_at())?)
                .bind(postgres_time(plan.session.window.expires_at())?)
                .bind(postgres_time(plan.session.window.absolute_expires_at())?)
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                let session = session_from_postgres(&row)?;
                insert_audit_postgres(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(session)
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(
                    "INSERT INTO sessions (id, organization_id, user_id, token_digest, csrf_digest, issued_at, last_activity_at, expires_at, absolute_expires_at, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?6, ?6, 1) RETURNING id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version",
                )
                .bind(plan.session.id.to_string())
                .bind(plan.session.organization_id.to_string())
                .bind(plan.session.user_id.to_string())
                .bind(plan.session.token_digest.expose_for_persistence())
                .bind(plan.session.csrf_digest.expose_for_persistence())
                .bind(plan.session.window.issued_at().unix_micros())
                .bind(plan.session.window.last_activity_at().unix_micros())
                .bind(plan.session.window.expires_at().unix_micros())
                .bind(plan.session.window.absolute_expires_at().unix_micros())
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                let session = session_from_sqlite(&row)?;
                insert_audit_sqlite(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(session)
            }
        }
    }

    async fn session_by_token_digest(
        &self,
        token_digest: &TokenDigest,
    ) -> Result<BrowserSession, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => session_from_postgres(
                &sqlx::query(
                    "SELECT id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version FROM sessions WHERE token_digest = $1",
                )
                .bind(token_digest.expose_for_persistence())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => session_from_sqlite(
                &sqlx::query(
                    "SELECT id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version FROM sessions WHERE token_digest = ?1",
                )
                .bind(token_digest.expose_for_persistence())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn session_by_token_and_csrf_digests(
        &self,
        token_digest: &TokenDigest,
        csrf_digest: &TokenDigest,
    ) -> Result<BrowserSession, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => session_from_postgres(
                &sqlx::query(
                    "SELECT id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version FROM sessions WHERE token_digest = $1 AND csrf_digest = $2",
                )
                .bind(token_digest.expose_for_persistence())
                .bind(csrf_digest.expose_for_persistence())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => session_from_sqlite(
                &sqlx::query(
                    "SELECT id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version FROM sessions WHERE token_digest = ?1 AND csrf_digest = ?2",
                )
                .bind(token_digest.expose_for_persistence())
                .bind(csrf_digest.expose_for_persistence())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn refresh_session(
        &self,
        id: SessionId,
        expected_version: i64,
        window: SessionWindow,
    ) -> Result<BrowserSession, RepositoryError> {
        update_session_activity(self, id, expected_version, window, None).await
    }

    async fn refresh_session_and_rotate_csrf(
        &self,
        id: SessionId,
        expected_version: i64,
        window: SessionWindow,
        csrf_digest: TokenDigest,
    ) -> Result<BrowserSession, RepositoryError> {
        update_session_activity(self, id, expected_version, window, Some(&csrf_digest)).await
    }

    async fn csrf_digest_by_session_id(
        &self,
        id: SessionId,
    ) -> Result<TokenDigest, RepositoryError> {
        let stored: String = match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                sqlx::query_scalar("SELECT csrf_digest FROM sessions WHERE id = $1")
                    .bind(id.as_uuid())
                    .fetch_one(pool)
                    .await
                    .map_err(map_sqlx_error)?
            }
            DatabasePool::Sqlite(pool) => {
                sqlx::query_scalar("SELECT csrf_digest FROM sessions WHERE id = ?1")
                    .bind(id.to_string())
                    .fetch_one(pool)
                    .await
                    .map_err(map_sqlx_error)?
            }
        };
        TokenDigest::from_persistence(stored).map_err(|_| RepositoryError::UnknownInfrastructure)
    }

    async fn revoke_session(
        &self,
        plan: RevokeSessionPlan,
    ) -> Result<BrowserSession, RepositoryError> {
        let existing = self.session_by_id(plan.session_id).await?;
        validate_session_audit(
            plan.session_id,
            existing.organization_id,
            existing.user_id,
            SESSION_REVOKED_AUDIT_ACTION,
            plan.revoked_at,
            &plan.audit_event,
        )?;
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(
                    "UPDATE sessions SET revoked_at = $1, updated_at = $1, version = version + 1 WHERE id = $2 AND version = $3 AND revoked_at IS NULL AND updated_at <= $1 RETURNING id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version",
                )
                .bind(postgres_time(plan.revoked_at)?)
                .bind(plan.session_id.as_uuid())
                .bind(plan.expected_version)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                let Some(row) = row else {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return Err(RepositoryError::VersionConflict);
                };
                let session = session_from_postgres(&row)?;
                insert_audit_postgres(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(session)
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(
                    "UPDATE sessions SET revoked_at = ?1, updated_at = ?1, version = version + 1 WHERE id = ?2 AND version = ?3 AND revoked_at IS NULL AND updated_at <= ?1 RETURNING id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version",
                )
                .bind(plan.revoked_at.unix_micros())
                .bind(plan.session_id.to_string())
                .bind(plan.expected_version)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                let Some(row) = row else {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return Err(RepositoryError::VersionConflict);
                };
                let session = session_from_sqlite(&row)?;
                insert_audit_sqlite(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(session)
            }
        }
    }
}

async fn update_session_activity(
    repository: &SqlxRepository,
    id: SessionId,
    expected_version: i64,
    window: SessionWindow,
    csrf_digest: Option<&TokenDigest>,
) -> Result<BrowserSession, RepositoryError> {
    let updated = match &repository.database.pool {
        DatabasePool::PostgreSql(pool) => sqlx::query(
            "UPDATE sessions SET last_activity_at = $1, expires_at = $2, csrf_digest = COALESCE($7, csrf_digest), updated_at = $1, version = version + 1 WHERE id = $3 AND version = $4 AND revoked_at IS NULL AND last_activity_at <= $1 AND expires_at > $1 AND absolute_expires_at > $1 AND issued_at = $5 AND absolute_expires_at = $6 RETURNING id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version",
        )
        .bind(postgres_time(window.last_activity_at())?)
        .bind(postgres_time(window.expires_at())?)
        .bind(id.as_uuid())
        .bind(expected_version)
        .bind(postgres_time(window.issued_at())?)
        .bind(postgres_time(window.absolute_expires_at())?)
        .bind(csrf_digest.map(TokenDigest::expose_for_persistence))
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx_error)?
        .map(|row| session_from_postgres(&row))
        .transpose()?,
        DatabasePool::Sqlite(pool) => sqlx::query(
            "UPDATE sessions SET last_activity_at = ?1, expires_at = ?2, csrf_digest = COALESCE(?7, csrf_digest), updated_at = ?1, version = version + 1 WHERE id = ?3 AND version = ?4 AND revoked_at IS NULL AND last_activity_at <= ?1 AND expires_at > ?1 AND absolute_expires_at > ?1 AND issued_at = ?5 AND absolute_expires_at = ?6 RETURNING id, organization_id, user_id, issued_at, last_activity_at, expires_at, absolute_expires_at, revoked_at, created_at, updated_at, version",
        )
        .bind(window.last_activity_at().unix_micros())
        .bind(window.expires_at().unix_micros())
        .bind(id.to_string())
        .bind(expected_version)
        .bind(window.issued_at().unix_micros())
        .bind(window.absolute_expires_at().unix_micros())
        .bind(csrf_digest.map(TokenDigest::expose_for_persistence))
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx_error)?
        .map(|row| session_from_sqlite(&row))
        .transpose()?,
    };
    classify_session_update(repository, id, updated).await
}

async fn classify_session_update(
    repository: &SqlxRepository,
    id: SessionId,
    updated: Option<BrowserSession>,
) -> Result<BrowserSession, RepositoryError> {
    if let Some(session) = updated {
        return Ok(session);
    }
    match repository.session_by_id(id).await {
        Ok(_) => Err(RepositoryError::VersionConflict),
        Err(RepositoryError::NotFound) => Err(RepositoryError::NotFound),
        Err(error) => Err(error),
    }
}

#[async_trait]
impl RecoveryRepository for SqlxRepository {
    async fn create_recovery_token(
        &self,
        plan: CreateRecoveryPlan,
    ) -> Result<RecoveryToken, RepositoryError> {
        validate_recovery_audit(
            plan.recovery.id,
            plan.recovery.organization_id,
            plan.recovery.user_id,
            RECOVERY_ISSUED_AUDIT_ACTION,
            plan.recovery.now,
            &plan.audit_event,
        )?;
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let recovery = recovery_from_postgres(
                    &sqlx::query(
                        "INSERT INTO recovery_tokens (id, organization_id, user_id, token_digest, expires_at, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $5, $6, $6, 1) RETURNING id, organization_id, user_id, expires_at, consumed_at, created_at, updated_at, version",
                    )
                    .bind(plan.recovery.id.as_uuid())
                    .bind(plan.recovery.organization_id.as_uuid())
                    .bind(plan.recovery.user_id.as_uuid())
                    .bind(plan.recovery.token_digest.expose_for_persistence())
                    .bind(postgres_time(plan.recovery.expires_at)?)
                    .bind(postgres_time(plan.recovery.now)?)
                    .fetch_one(&mut *transaction)
                    .await
                    .map_err(map_sqlx_error)?,
                )?;
                insert_audit_postgres(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(recovery)
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let recovery = recovery_from_sqlite(
                    &sqlx::query(
                        "INSERT INTO recovery_tokens (id, organization_id, user_id, token_digest, expires_at, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 1) RETURNING id, organization_id, user_id, expires_at, consumed_at, created_at, updated_at, version",
                    )
                    .bind(plan.recovery.id.to_string())
                    .bind(plan.recovery.organization_id.to_string())
                    .bind(plan.recovery.user_id.to_string())
                    .bind(plan.recovery.token_digest.expose_for_persistence())
                    .bind(plan.recovery.expires_at.unix_micros())
                    .bind(plan.recovery.now.unix_micros())
                    .fetch_one(&mut *transaction)
                    .await
                    .map_err(map_sqlx_error)?,
                )?;
                insert_audit_sqlite(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(recovery)
            }
        }
    }

    async fn recovery_token_by_digest(
        &self,
        token_digest: &TokenDigest,
    ) -> Result<RecoveryToken, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => recovery_from_postgres(
                &sqlx::query(
                    "SELECT id, organization_id, user_id, expires_at, consumed_at, created_at, updated_at, version FROM recovery_tokens WHERE token_digest = $1",
                )
                .bind(token_digest.expose_for_persistence())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => recovery_from_sqlite(
                &sqlx::query(
                    "SELECT id, organization_id, user_id, expires_at, consumed_at, created_at, updated_at, version FROM recovery_tokens WHERE token_digest = ?1",
                )
                .bind(token_digest.expose_for_persistence())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn complete_recovery(
        &self,
        plan: CompleteRecoveryPlan,
    ) -> Result<RecoveryToken, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(
                    "UPDATE recovery_tokens SET consumed_at = $1, updated_at = $1, version = version + 1 WHERE token_digest = $2 AND consumed_at IS NULL AND created_at <= $1 AND expires_at > $1 RETURNING id, organization_id, user_id, expires_at, consumed_at, created_at, updated_at, version",
                )
                .bind(postgres_time(plan.completed_at)?)
                .bind(plan.token_digest.expose_for_persistence())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                let Some(row) = row else {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return classify_recovery_update(self, &plan.token_digest).await;
                };
                let recovery = recovery_from_postgres(&row)?;
                if let Err(error) = validate_recovery_audit(
                    recovery.id,
                    recovery.organization_id,
                    recovery.user_id,
                    RECOVERY_COMPLETED_AUDIT_ACTION,
                    plan.completed_at,
                    &plan.audit_event,
                ) {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return Err(error);
                }
                let credential = sqlx::query(
                    "UPDATE local_credentials SET password_hash = $1, updated_at = $2, version = version + 1 WHERE user_id = $3 AND updated_at <= $2",
                )
                .bind(plan.replacement_password_hash.expose_for_persistence())
                .bind(postgres_time(plan.completed_at)?)
                .bind(recovery.user_id.as_uuid())
                .execute(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                if credential.rows_affected() != 1 {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return Err(RepositoryError::VersionConflict);
                }
                sqlx::query(
                    "UPDATE sessions SET revoked_at = $1, updated_at = $1, version = version + 1 WHERE organization_id = $2 AND user_id = $3 AND revoked_at IS NULL AND updated_at <= $1",
                )
                .bind(postgres_time(plan.completed_at)?)
                .bind(recovery.organization_id.as_uuid())
                .bind(recovery.user_id.as_uuid())
                .execute(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                insert_audit_postgres(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(recovery)
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(
                    "UPDATE recovery_tokens SET consumed_at = ?1, updated_at = ?1, version = version + 1 WHERE token_digest = ?2 AND consumed_at IS NULL AND created_at <= ?1 AND expires_at > ?1 RETURNING id, organization_id, user_id, expires_at, consumed_at, created_at, updated_at, version",
                )
                .bind(plan.completed_at.unix_micros())
                .bind(plan.token_digest.expose_for_persistence())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                let Some(row) = row else {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return classify_recovery_update(self, &plan.token_digest).await;
                };
                let recovery = recovery_from_sqlite(&row)?;
                if let Err(error) = validate_recovery_audit(
                    recovery.id,
                    recovery.organization_id,
                    recovery.user_id,
                    RECOVERY_COMPLETED_AUDIT_ACTION,
                    plan.completed_at,
                    &plan.audit_event,
                ) {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return Err(error);
                }
                let credential = sqlx::query(
                    "UPDATE local_credentials SET password_hash = ?1, updated_at = ?2, version = version + 1 WHERE user_id = ?3 AND updated_at <= ?2",
                )
                .bind(plan.replacement_password_hash.expose_for_persistence())
                .bind(plan.completed_at.unix_micros())
                .bind(recovery.user_id.to_string())
                .execute(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                if credential.rows_affected() != 1 {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return Err(RepositoryError::VersionConflict);
                }
                sqlx::query(
                    "UPDATE sessions SET revoked_at = ?1, updated_at = ?1, version = version + 1 WHERE organization_id = ?2 AND user_id = ?3 AND revoked_at IS NULL AND updated_at <= ?1",
                )
                .bind(plan.completed_at.unix_micros())
                .bind(recovery.organization_id.to_string())
                .bind(recovery.user_id.to_string())
                .execute(&mut *transaction)
                .await
                .map_err(map_sqlx_error)?;
                insert_audit_sqlite(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(recovery)
            }
        }
    }
}

async fn classify_recovery_update(
    repository: &SqlxRepository,
    token_digest: &TokenDigest,
) -> Result<RecoveryToken, RepositoryError> {
    match repository.recovery_token_by_digest(token_digest).await {
        Ok(_) => Err(RepositoryError::VersionConflict),
        Err(RepositoryError::NotFound) => Err(RepositoryError::NotFound),
        Err(error) => Err(error),
    }
}

#[async_trait]
impl AuthenticationRepository for SqlxRepository {
    async fn local_authentication_context(
        &self,
    ) -> Result<LocalAuthenticationContext, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => local_authentication_from_postgres(
                &sqlx::query(
                    "SELECT m.organization_id, p.id AS project_id, m.id AS membership_id, m.role, u.id, u.normalized_username, u.display_name, u.created_at, u.updated_at, u.version, c.password_hash FROM users u JOIN local_credentials c ON c.user_id = u.id JOIN memberships m ON m.user_id = u.id JOIN projects p ON p.organization_id = m.organization_id AND (p.id = m.project_id OR (m.project_id IS NULL AND p.slug = 'default')) ORDER BY u.created_at, u.id, p.created_at LIMIT 1",
                )
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => local_authentication_from_sqlite(
                &sqlx::query(
                    "SELECT m.organization_id, p.id AS project_id, m.id AS membership_id, m.role, u.id, u.normalized_username, u.display_name, u.created_at, u.updated_at, u.version, c.password_hash FROM users u JOIN local_credentials c ON c.user_id = u.id JOIN memberships m ON m.user_id = u.id JOIN projects p ON p.organization_id = m.organization_id AND (p.id = m.project_id OR (m.project_id IS NULL AND p.slug = 'default')) ORDER BY u.created_at, u.id, p.created_at LIMIT 1",
                )
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)?,
            ),
        }
    }
}

#[async_trait]
impl BootstrapRepository for SqlxRepository {
    async fn bootstrap(
        &self,
        plan: BootstrapPlan,
    ) -> Result<BootstrapStoreResult, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                sqlx::query("SELECT pg_advisory_xact_lock($1)")
                    .bind(8_428_154_626_001_i64)
                    .execute(&mut *transaction)
                    .await
                    .map_err(map_sqlx_error)?;

                if let Some(existing) = existing_bootstrap_postgres(&mut transaction).await? {
                    transaction.commit().await.map_err(map_sqlx_error)?;
                    return Ok(BootstrapStoreResult::Existing(existing));
                }
                if bootstrap_row_count_postgres(&mut transaction).await? != 0 {
                    transaction.commit().await.map_err(map_sqlx_error)?;
                    return Ok(BootstrapStoreResult::Conflict);
                }

                let resources = resources_from_plan(&plan);
                insert_bootstrap_postgres(&mut transaction, &plan).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(BootstrapStoreResult::Created(resources))
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool
                    .begin_with("BEGIN IMMEDIATE")
                    .await
                    .map_err(map_sqlx_error)?;
                if let Some(existing) = existing_bootstrap_sqlite(&mut transaction).await? {
                    transaction.commit().await.map_err(map_sqlx_error)?;
                    return Ok(BootstrapStoreResult::Existing(existing));
                }
                if bootstrap_row_count_sqlite(&mut transaction).await? != 0 {
                    transaction.commit().await.map_err(map_sqlx_error)?;
                    return Ok(BootstrapStoreResult::Conflict);
                }
                let resources = resources_from_plan(&plan);
                insert_bootstrap_sqlite(&mut transaction, &plan).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                Ok(BootstrapStoreResult::Created(resources))
            }
        }
    }
}

async fn bootstrap_row_count_postgres(
    connection: &mut PgConnection,
) -> Result<i64, RepositoryError> {
    sqlx::query(
        "SELECT ((SELECT COUNT(*) FROM organizations) + (SELECT COUNT(*) FROM projects) + (SELECT COUNT(*) FROM users) + (SELECT COUNT(*) FROM local_credentials) + (SELECT COUNT(*) FROM memberships) + (SELECT COUNT(*) FROM audit_events))::BIGINT AS row_count",
    )
    .fetch_one(connection)
    .await
    .map_err(map_sqlx_error)?
    .try_get("row_count")
    .map_err(map_sqlx_error)
}

async fn bootstrap_row_count_sqlite(
    connection: &mut SqliteConnection,
) -> Result<i64, RepositoryError> {
    sqlx::query(
        "SELECT ((SELECT COUNT(*) FROM organizations) + (SELECT COUNT(*) FROM projects) + (SELECT COUNT(*) FROM users) + (SELECT COUNT(*) FROM local_credentials) + (SELECT COUNT(*) FROM memberships) + (SELECT COUNT(*) FROM audit_events)) AS row_count",
    )
    .fetch_one(connection)
    .await
    .map_err(map_sqlx_error)?
    .try_get("row_count")
    .map_err(map_sqlx_error)
}

async fn existing_bootstrap_postgres(
    connection: &mut PgConnection,
) -> Result<Option<ExistingBootstrap>, RepositoryError> {
    let row = sqlx::query(
        "SELECT o.id AS organization_id, o.slug AS organization_slug, o.name AS organization_name, o.created_at AS organization_created_at, o.updated_at AS organization_updated_at, o.version AS organization_version, p.id AS project_id, p.slug AS project_slug, p.name AS project_name, p.default_timezone, p.created_at AS project_created_at, p.updated_at AS project_updated_at, p.version AS project_version, u.id AS user_id, u.normalized_username, u.display_name, u.created_at AS user_created_at, u.updated_at AS user_updated_at, u.version AS user_version, c.password_hash, m.id AS membership_id, m.role, m.created_at AS membership_created_at, m.updated_at AS membership_updated_at, m.version AS membership_version, a.id AS audit_id, a.actor_type, a.actor_id, a.action, a.resource_type, a.resource_id, a.request_id, a.metadata, a.occurred_at FROM organizations o JOIN projects p ON p.organization_id = o.id AND p.slug = 'default' JOIN memberships m ON m.organization_id = o.id AND m.project_id IS NULL AND m.role = 'owner' JOIN users u ON u.id = m.user_id JOIN local_credentials c ON c.user_id = u.id JOIN audit_events a ON a.organization_id = o.id AND a.project_id = p.id AND a.actor_id = u.id AND a.action = 'admin.bootstrap' WHERE o.slug = 'default' ORDER BY a.occurred_at ASC LIMIT 1",
    )
    .fetch_optional(connection)
    .await
    .map_err(map_sqlx_error)?;
    row.as_ref().map(existing_from_postgres).transpose()
}

async fn existing_bootstrap_sqlite(
    connection: &mut SqliteConnection,
) -> Result<Option<ExistingBootstrap>, RepositoryError> {
    let row = sqlx::query(
        "SELECT o.id AS organization_id, o.slug AS organization_slug, o.name AS organization_name, o.created_at AS organization_created_at, o.updated_at AS organization_updated_at, o.version AS organization_version, p.id AS project_id, p.slug AS project_slug, p.name AS project_name, p.default_timezone, p.created_at AS project_created_at, p.updated_at AS project_updated_at, p.version AS project_version, u.id AS user_id, u.normalized_username, u.display_name, u.created_at AS user_created_at, u.updated_at AS user_updated_at, u.version AS user_version, c.password_hash, m.id AS membership_id, m.role, m.created_at AS membership_created_at, m.updated_at AS membership_updated_at, m.version AS membership_version, a.id AS audit_id, a.actor_type, a.actor_id, a.action, a.resource_type, a.resource_id, a.request_id, a.metadata, a.occurred_at FROM organizations o JOIN projects p ON p.organization_id = o.id AND p.slug = 'default' JOIN memberships m ON m.organization_id = o.id AND m.project_id IS NULL AND m.role = 'owner' JOIN users u ON u.id = m.user_id JOIN local_credentials c ON c.user_id = u.id JOIN audit_events a ON a.organization_id = o.id AND a.project_id = p.id AND a.actor_id = u.id AND a.action = 'admin.bootstrap' WHERE o.slug = 'default' ORDER BY a.occurred_at ASC LIMIT 1",
    )
    .fetch_optional(connection)
    .await
    .map_err(map_sqlx_error)?;
    row.as_ref().map(existing_from_sqlite).transpose()
}

async fn insert_bootstrap_postgres(
    connection: &mut PgConnection,
    plan: &BootstrapPlan,
) -> Result<(), RepositoryError> {
    let now = postgres_time(plan.organization.now)?;
    sqlx::query(
        "INSERT INTO organizations (id, slug, name, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $4, 1)",
    )
    .bind(plan.organization.id.as_uuid())
    .bind(&plan.organization.slug)
    .bind(&plan.organization.name)
    .bind(now)
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    sqlx::query(
        "INSERT INTO projects (id, organization_id, slug, name, default_timezone, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $5, $6, $6, 1)",
    )
    .bind(plan.project.id.as_uuid())
    .bind(plan.project.organization_id.as_uuid())
    .bind(&plan.project.slug)
    .bind(&plan.project.name)
    .bind(&plan.project.default_timezone)
    .bind(postgres_time(plan.project.now)?)
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    sqlx::query(
        "INSERT INTO users (id, normalized_username, display_name, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $4, 1)",
    )
    .bind(plan.user.id.as_uuid())
    .bind(&plan.user.normalized_username)
    .bind(&plan.user.display_name)
    .bind(postgres_time(plan.user.now)?)
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    sqlx::query(
        "INSERT INTO local_credentials (user_id, password_hash, created_at, updated_at, version) VALUES ($1, $2, $3, $3, 1)",
    )
    .bind(plan.user.id.as_uuid())
    .bind(plan.user.password_hash.expose_for_persistence())
    .bind(postgres_time(plan.user.now)?)
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    sqlx::query(
        "INSERT INTO memberships (id, organization_id, project_id, user_id, role, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $5, $6, $6, 1)",
    )
    .bind(plan.membership.id.as_uuid())
    .bind(plan.membership.organization_id.as_uuid())
    .bind(plan.membership.project_id.map(ProjectId::as_uuid))
    .bind(plan.membership.user_id.as_uuid())
    .bind(plan.membership.role.as_str())
    .bind(postgres_time(plan.membership.now)?)
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    let audit = &plan.audit_event.event;
    sqlx::query(
        "INSERT INTO audit_events (id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(audit.id.as_uuid())
    .bind(audit.organization_id.as_uuid())
    .bind(audit.project_id.map(ProjectId::as_uuid))
    .bind(audit.actor_type.as_str())
    .bind(audit.actor_id.map(UserId::as_uuid))
    .bind(&audit.action)
    .bind(&audit.resource_type)
    .bind(audit.resource_id.as_uuid())
    .bind(audit.request_id.as_uuid())
    .bind(audit_metadata_json(&audit.metadata))
    .bind(postgres_time(audit.occurred_at)?)
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

async fn insert_bootstrap_sqlite(
    connection: &mut SqliteConnection,
    plan: &BootstrapPlan,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO organizations (id, slug, name, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?4, 1)",
    )
    .bind(plan.organization.id.to_string())
    .bind(&plan.organization.slug)
    .bind(&plan.organization.name)
    .bind(plan.organization.now.unix_micros())
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    sqlx::query(
        "INSERT INTO projects (id, organization_id, slug, name, default_timezone, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 1)",
    )
    .bind(plan.project.id.to_string())
    .bind(plan.project.organization_id.to_string())
    .bind(&plan.project.slug)
    .bind(&plan.project.name)
    .bind(&plan.project.default_timezone)
    .bind(plan.project.now.unix_micros())
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    sqlx::query(
        "INSERT INTO users (id, normalized_username, display_name, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?4, 1)",
    )
    .bind(plan.user.id.to_string())
    .bind(&plan.user.normalized_username)
    .bind(&plan.user.display_name)
    .bind(plan.user.now.unix_micros())
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    sqlx::query(
        "INSERT INTO local_credentials (user_id, password_hash, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?3, 1)",
    )
    .bind(plan.user.id.to_string())
    .bind(plan.user.password_hash.expose_for_persistence())
    .bind(plan.user.now.unix_micros())
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    sqlx::query(
        "INSERT INTO memberships (id, organization_id, project_id, user_id, role, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 1)",
    )
    .bind(plan.membership.id.to_string())
    .bind(plan.membership.organization_id.to_string())
    .bind(plan.membership.project_id.map(|id| id.to_string()))
    .bind(plan.membership.user_id.to_string())
    .bind(plan.membership.role.as_str())
    .bind(plan.membership.now.unix_micros())
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    let audit = &plan.audit_event.event;
    sqlx::query(
        "INSERT INTO audit_events (id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )
    .bind(audit.id.to_string())
    .bind(audit.organization_id.to_string())
    .bind(audit.project_id.map(|id| id.to_string()))
    .bind(audit.actor_type.as_str())
    .bind(audit.actor_id.map(|id| id.to_string()))
    .bind(&audit.action)
    .bind(&audit.resource_type)
    .bind(audit.resource_id.to_string())
    .bind(audit.request_id.to_string())
    .bind(audit_metadata_json(&audit.metadata).to_string())
    .bind(audit.occurred_at.unix_micros())
    .execute(&mut *connection)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

fn resources_from_plan(plan: &BootstrapPlan) -> BootstrapResources {
    BootstrapResources {
        organization: Organization {
            id: plan.organization.id,
            slug: plan.organization.slug.clone(),
            name: plan.organization.name.clone(),
            created_at: plan.organization.now,
            updated_at: plan.organization.now,
            version: 1,
        },
        project: Project {
            id: plan.project.id,
            organization_id: plan.project.organization_id,
            slug: plan.project.slug.clone(),
            name: plan.project.name.clone(),
            default_timezone: plan.project.default_timezone.clone(),
            created_at: plan.project.now,
            updated_at: plan.project.now,
            version: 1,
        },
        user: LocalUser {
            id: plan.user.id,
            normalized_username: plan.user.normalized_username.clone(),
            display_name: plan.user.display_name.clone(),
            created_at: plan.user.now,
            updated_at: plan.user.now,
            version: 1,
        },
        membership: Membership {
            id: plan.membership.id,
            organization_id: plan.membership.organization_id,
            project_id: plan.membership.project_id,
            user_id: plan.membership.user_id,
            role: plan.membership.role,
            created_at: plan.membership.now,
            updated_at: plan.membership.now,
            version: 1,
        },
        audit_event: plan.audit_event.event.clone(),
    }
}

fn existing_from_postgres(row: &PgRow) -> Result<ExistingBootstrap, RepositoryError> {
    let organization_id =
        OrganizationId::from_uuid(row.try_get("organization_id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    let project_id = ProjectId::from_uuid(row.try_get("project_id").map_err(map_sqlx_error)?)
        .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    let user_id = UserId::from_uuid(row.try_get("user_id").map_err(map_sqlx_error)?)
        .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    let membership_id =
        MembershipId::from_uuid(row.try_get("membership_id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    let audit_id = AuditEventId::from_uuid(row.try_get("audit_id").map_err(map_sqlx_error)?)
        .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    let resource_id = ResourceId::from_uuid(row.try_get("resource_id").map_err(map_sqlx_error)?)
        .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    let request_id = OperationId::from_uuid(row.try_get("request_id").map_err(map_sqlx_error)?)
        .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    let actor_id = row
        .try_get::<Option<uuid::Uuid>, _>("actor_id")
        .map_err(map_sqlx_error)?
        .map(UserId::from_uuid)
        .transpose()
        .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    let resources = BootstrapResources {
        organization: Organization {
            id: organization_id,
            slug: row.try_get("organization_slug").map_err(map_sqlx_error)?,
            name: row.try_get("organization_name").map_err(map_sqlx_error)?,
            created_at: timestamp_from_postgres(row, "organization_created_at")?,
            updated_at: timestamp_from_postgres(row, "organization_updated_at")?,
            version: row
                .try_get("organization_version")
                .map_err(map_sqlx_error)?,
        },
        project: Project {
            id: project_id,
            organization_id,
            slug: row.try_get("project_slug").map_err(map_sqlx_error)?,
            name: row.try_get("project_name").map_err(map_sqlx_error)?,
            default_timezone: row.try_get("default_timezone").map_err(map_sqlx_error)?,
            created_at: timestamp_from_postgres(row, "project_created_at")?,
            updated_at: timestamp_from_postgres(row, "project_updated_at")?,
            version: row.try_get("project_version").map_err(map_sqlx_error)?,
        },
        user: LocalUser {
            id: user_id,
            normalized_username: row.try_get("normalized_username").map_err(map_sqlx_error)?,
            display_name: row.try_get("display_name").map_err(map_sqlx_error)?,
            created_at: timestamp_from_postgres(row, "user_created_at")?,
            updated_at: timestamp_from_postgres(row, "user_updated_at")?,
            version: row.try_get("user_version").map_err(map_sqlx_error)?,
        },
        membership: Membership {
            id: membership_id,
            organization_id,
            project_id: None,
            user_id,
            role: Role::from_str(row.try_get::<&str, _>("role").map_err(map_sqlx_error)?)
                .map_err(|_| RepositoryError::UnknownInfrastructure)?,
            created_at: timestamp_from_postgres(row, "membership_created_at")?,
            updated_at: timestamp_from_postgres(row, "membership_updated_at")?,
            version: row.try_get("membership_version").map_err(map_sqlx_error)?,
        },
        audit_event: AuditEvent {
            id: audit_id,
            organization_id,
            project_id: Some(project_id),
            actor_type: parse_actor_type(row.try_get("actor_type").map_err(map_sqlx_error)?)?,
            actor_id,
            action: row.try_get("action").map_err(map_sqlx_error)?,
            resource_type: row.try_get("resource_type").map_err(map_sqlx_error)?,
            resource_id,
            request_id,
            metadata: parse_audit_metadata(&row.try_get("metadata").map_err(map_sqlx_error)?)?,
            occurred_at: timestamp_from_postgres(row, "occurred_at")?,
        },
    };
    let password_hash =
        PasswordHash::from_persistence(row.try_get("password_hash").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    Ok(ExistingBootstrap {
        resources,
        password_hash,
    })
}

fn existing_from_sqlite(row: &SqliteRow) -> Result<ExistingBootstrap, RepositoryError> {
    let organization_id = parse_id::<OrganizationId>(row, "organization_id")?;
    let project_id = parse_id::<ProjectId>(row, "project_id")?;
    let user_id = parse_id::<UserId>(row, "user_id")?;
    let membership_id = parse_id::<MembershipId>(row, "membership_id")?;
    let audit_id = parse_id::<AuditEventId>(row, "audit_id")?;
    let resource_id = parse_id::<ResourceId>(row, "resource_id")?;
    let request_id = parse_id::<OperationId>(row, "request_id")?;
    let actor_id = parse_optional_id::<UserId>(row, "actor_id")?;
    let metadata_text: String = row.try_get("metadata").map_err(map_sqlx_error)?;
    let metadata_value =
        serde_json::from_str(&metadata_text).map_err(|_| RepositoryError::UnknownInfrastructure)?;
    let resources = BootstrapResources {
        organization: Organization {
            id: organization_id,
            slug: row.try_get("organization_slug").map_err(map_sqlx_error)?,
            name: row.try_get("organization_name").map_err(map_sqlx_error)?,
            created_at: timestamp_from_sqlite(row, "organization_created_at")?,
            updated_at: timestamp_from_sqlite(row, "organization_updated_at")?,
            version: row
                .try_get("organization_version")
                .map_err(map_sqlx_error)?,
        },
        project: Project {
            id: project_id,
            organization_id,
            slug: row.try_get("project_slug").map_err(map_sqlx_error)?,
            name: row.try_get("project_name").map_err(map_sqlx_error)?,
            default_timezone: row.try_get("default_timezone").map_err(map_sqlx_error)?,
            created_at: timestamp_from_sqlite(row, "project_created_at")?,
            updated_at: timestamp_from_sqlite(row, "project_updated_at")?,
            version: row.try_get("project_version").map_err(map_sqlx_error)?,
        },
        user: LocalUser {
            id: user_id,
            normalized_username: row.try_get("normalized_username").map_err(map_sqlx_error)?,
            display_name: row.try_get("display_name").map_err(map_sqlx_error)?,
            created_at: timestamp_from_sqlite(row, "user_created_at")?,
            updated_at: timestamp_from_sqlite(row, "user_updated_at")?,
            version: row.try_get("user_version").map_err(map_sqlx_error)?,
        },
        membership: Membership {
            id: membership_id,
            organization_id,
            project_id: None,
            user_id,
            role: Role::from_str(row.try_get::<&str, _>("role").map_err(map_sqlx_error)?)
                .map_err(|_| RepositoryError::UnknownInfrastructure)?,
            created_at: timestamp_from_sqlite(row, "membership_created_at")?,
            updated_at: timestamp_from_sqlite(row, "membership_updated_at")?,
            version: row.try_get("membership_version").map_err(map_sqlx_error)?,
        },
        audit_event: AuditEvent {
            id: audit_id,
            organization_id,
            project_id: Some(project_id),
            actor_type: parse_actor_type(row.try_get("actor_type").map_err(map_sqlx_error)?)?,
            actor_id,
            action: row.try_get("action").map_err(map_sqlx_error)?,
            resource_type: row.try_get("resource_type").map_err(map_sqlx_error)?,
            resource_id,
            request_id,
            metadata: parse_audit_metadata(&metadata_value)?,
            occurred_at: timestamp_from_sqlite(row, "occurred_at")?,
        },
    };
    let password_hash =
        PasswordHash::from_persistence(row.try_get("password_hash").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    Ok(ExistingBootstrap {
        resources,
        password_hash,
    })
}

fn organization_from_postgres(row: &PgRow) -> Result<Organization, RepositoryError> {
    Ok(Organization {
        id: OrganizationId::from_uuid(row.try_get("id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        slug: row.try_get("slug").map_err(map_sqlx_error)?,
        name: row.try_get("name").map_err(map_sqlx_error)?,
        created_at: timestamp_from_postgres(row, "created_at")?,
        updated_at: timestamp_from_postgres(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn organization_from_sqlite(row: &SqliteRow) -> Result<Organization, RepositoryError> {
    Ok(Organization {
        id: parse_id(row, "id")?,
        slug: row.try_get("slug").map_err(map_sqlx_error)?,
        name: row.try_get("name").map_err(map_sqlx_error)?,
        created_at: timestamp_from_sqlite(row, "created_at")?,
        updated_at: timestamp_from_sqlite(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn project_from_postgres(row: &PgRow) -> Result<Project, RepositoryError> {
    Ok(Project {
        id: ProjectId::from_uuid(row.try_get("id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        organization_id: OrganizationId::from_uuid(
            row.try_get("organization_id").map_err(map_sqlx_error)?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        slug: row.try_get("slug").map_err(map_sqlx_error)?,
        name: row.try_get("name").map_err(map_sqlx_error)?,
        default_timezone: row.try_get("default_timezone").map_err(map_sqlx_error)?,
        created_at: timestamp_from_postgres(row, "created_at")?,
        updated_at: timestamp_from_postgres(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn project_from_sqlite(row: &SqliteRow) -> Result<Project, RepositoryError> {
    Ok(Project {
        id: parse_id(row, "id")?,
        organization_id: parse_id(row, "organization_id")?,
        slug: row.try_get("slug").map_err(map_sqlx_error)?,
        name: row.try_get("name").map_err(map_sqlx_error)?,
        default_timezone: row.try_get("default_timezone").map_err(map_sqlx_error)?,
        created_at: timestamp_from_sqlite(row, "created_at")?,
        updated_at: timestamp_from_sqlite(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn local_authentication_from_postgres(
    row: &PgRow,
) -> Result<LocalAuthenticationContext, RepositoryError> {
    Ok(LocalAuthenticationContext {
        organization_id: OrganizationId::from_uuid(
            row.try_get("organization_id").map_err(map_sqlx_error)?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        project_id: ProjectId::from_uuid(row.try_get("project_id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        membership_id: MembershipId::from_uuid(
            row.try_get("membership_id").map_err(map_sqlx_error)?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        role: Role::from_str(row.try_get("role").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        user: local_user_from_postgres(row)?,
        password_hash: PasswordHash::from_persistence(
            row.try_get("password_hash").map_err(map_sqlx_error)?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
    })
}

fn local_authentication_from_sqlite(
    row: &SqliteRow,
) -> Result<LocalAuthenticationContext, RepositoryError> {
    Ok(LocalAuthenticationContext {
        organization_id: parse_id(row, "organization_id")?,
        project_id: parse_id(row, "project_id")?,
        membership_id: parse_id(row, "membership_id")?,
        role: Role::from_str(row.try_get("role").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        user: local_user_from_sqlite(row)?,
        password_hash: PasswordHash::from_persistence(
            row.try_get("password_hash").map_err(map_sqlx_error)?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
    })
}

fn local_user_from_postgres(row: &PgRow) -> Result<LocalUser, RepositoryError> {
    Ok(LocalUser {
        id: UserId::from_uuid(row.try_get("id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        normalized_username: row.try_get("normalized_username").map_err(map_sqlx_error)?,
        display_name: row.try_get("display_name").map_err(map_sqlx_error)?,
        created_at: timestamp_from_postgres(row, "created_at")?,
        updated_at: timestamp_from_postgres(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn local_user_from_sqlite(row: &SqliteRow) -> Result<LocalUser, RepositoryError> {
    Ok(LocalUser {
        id: parse_id(row, "id")?,
        normalized_username: row.try_get("normalized_username").map_err(map_sqlx_error)?,
        display_name: row.try_get("display_name").map_err(map_sqlx_error)?,
        created_at: timestamp_from_sqlite(row, "created_at")?,
        updated_at: timestamp_from_sqlite(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn membership_from_postgres(row: &PgRow) -> Result<Membership, RepositoryError> {
    Ok(Membership {
        id: MembershipId::from_uuid(row.try_get("id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        organization_id: OrganizationId::from_uuid(
            row.try_get("organization_id").map_err(map_sqlx_error)?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        project_id: row
            .try_get::<Option<uuid::Uuid>, _>("project_id")
            .map_err(map_sqlx_error)?
            .map(ProjectId::from_uuid)
            .transpose()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        user_id: UserId::from_uuid(row.try_get("user_id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        role: Role::from_str(row.try_get::<&str, _>("role").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        created_at: timestamp_from_postgres(row, "created_at")?,
        updated_at: timestamp_from_postgres(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn membership_from_sqlite(row: &SqliteRow) -> Result<Membership, RepositoryError> {
    Ok(Membership {
        id: parse_id(row, "id")?,
        organization_id: parse_id(row, "organization_id")?,
        project_id: parse_optional_id(row, "project_id")?,
        user_id: parse_id(row, "user_id")?,
        role: Role::from_str(row.try_get::<&str, _>("role").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        created_at: timestamp_from_sqlite(row, "created_at")?,
        updated_at: timestamp_from_sqlite(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

async fn insert_audit_postgres(
    connection: &mut PgConnection,
    event: &NewAuditEvent,
) -> Result<AuditEvent, RepositoryError> {
    validate_audit_event(event)?;
    let metadata = audit_metadata_json(&event.event.metadata);
    audit_from_postgres(
        &sqlx::query(
            "INSERT INTO audit_events (id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at",
        )
        .bind(event.event.id.as_uuid())
        .bind(event.event.organization_id.as_uuid())
        .bind(event.event.project_id.map(ProjectId::as_uuid))
        .bind(event.event.actor_type.as_str())
        .bind(event.event.actor_id.map(UserId::as_uuid))
        .bind(&event.event.action)
        .bind(&event.event.resource_type)
        .bind(event.event.resource_id.as_uuid())
        .bind(event.event.request_id.as_uuid())
        .bind(metadata)
        .bind(postgres_time(event.event.occurred_at)?)
        .fetch_one(connection)
        .await
        .map_err(map_sqlx_error)?,
    )
}

async fn insert_audit_sqlite(
    connection: &mut SqliteConnection,
    event: &NewAuditEvent,
) -> Result<AuditEvent, RepositoryError> {
    validate_audit_event(event)?;
    let metadata = audit_metadata_json(&event.event.metadata);
    audit_from_sqlite(
        &sqlx::query(
            "INSERT INTO audit_events (id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) RETURNING id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at",
        )
        .bind(event.event.id.to_string())
        .bind(event.event.organization_id.to_string())
        .bind(event.event.project_id.map(|id| id.to_string()))
        .bind(event.event.actor_type.as_str())
        .bind(event.event.actor_id.map(|id| id.to_string()))
        .bind(&event.event.action)
        .bind(&event.event.resource_type)
        .bind(event.event.resource_id.to_string())
        .bind(event.event.request_id.to_string())
        .bind(metadata.to_string())
        .bind(event.event.occurred_at.unix_micros())
        .fetch_one(connection)
        .await
        .map_err(map_sqlx_error)?,
    )
}

fn validate_audit_event(event: &NewAuditEvent) -> Result<(), RepositoryError> {
    validate_bounded_text(&event.event.action, 120)?;
    validate_bounded_text(&event.event.resource_type, 64)
}

fn validate_session_audit(
    session_id: SessionId,
    organization_id: OrganizationId,
    user_id: UserId,
    action: &str,
    occurred_at: UtcTimestamp,
    audit_event: &NewAuditEvent,
) -> Result<(), RepositoryError> {
    let event = &audit_event.event;
    if event.organization_id != organization_id
        || event.actor_id != Some(user_id)
        || event.action != action
        || event.resource_type != "session"
        || event.resource_id.as_uuid() != session_id.as_uuid()
        || event.occurred_at != occurred_at
    {
        return Err(RepositoryError::ConstraintViolation);
    }
    validate_audit_event(audit_event)
}

fn validate_recovery_audit(
    recovery_id: RecoveryTokenId,
    organization_id: OrganizationId,
    user_id: UserId,
    action: &str,
    occurred_at: UtcTimestamp,
    audit_event: &NewAuditEvent,
) -> Result<(), RepositoryError> {
    let event = &audit_event.event;
    if event.organization_id != organization_id
        || event.actor_id != Some(user_id)
        || event.action != action
        || event.resource_type != "recovery_token"
        || event.resource_id.as_uuid() != recovery_id.as_uuid()
        || event.occurred_at != occurred_at
    {
        return Err(RepositoryError::ConstraintViolation);
    }
    validate_audit_event(audit_event)
}

fn session_from_postgres(row: &PgRow) -> Result<BrowserSession, RepositoryError> {
    Ok(BrowserSession {
        id: SessionId::from_uuid(row.try_get("id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        organization_id: OrganizationId::from_uuid(
            row.try_get("organization_id").map_err(map_sqlx_error)?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        user_id: UserId::from_uuid(row.try_get("user_id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        window: SessionWindow::from_persistence(
            timestamp_from_postgres(row, "issued_at")?,
            timestamp_from_postgres(row, "last_activity_at")?,
            timestamp_from_postgres(row, "expires_at")?,
            timestamp_from_postgres(row, "absolute_expires_at")?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        revoked_at: optional_timestamp_from_postgres(row, "revoked_at")?,
        created_at: timestamp_from_postgres(row, "created_at")?,
        updated_at: timestamp_from_postgres(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn session_from_sqlite(row: &SqliteRow) -> Result<BrowserSession, RepositoryError> {
    Ok(BrowserSession {
        id: parse_id(row, "id")?,
        organization_id: parse_id(row, "organization_id")?,
        user_id: parse_id(row, "user_id")?,
        window: SessionWindow::from_persistence(
            timestamp_from_sqlite(row, "issued_at")?,
            timestamp_from_sqlite(row, "last_activity_at")?,
            timestamp_from_sqlite(row, "expires_at")?,
            timestamp_from_sqlite(row, "absolute_expires_at")?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        revoked_at: optional_timestamp_from_sqlite(row, "revoked_at")?,
        created_at: timestamp_from_sqlite(row, "created_at")?,
        updated_at: timestamp_from_sqlite(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn recovery_from_postgres(row: &PgRow) -> Result<RecoveryToken, RepositoryError> {
    Ok(RecoveryToken {
        id: RecoveryTokenId::from_uuid(row.try_get("id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        organization_id: OrganizationId::from_uuid(
            row.try_get("organization_id").map_err(map_sqlx_error)?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        user_id: UserId::from_uuid(row.try_get("user_id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        expires_at: timestamp_from_postgres(row, "expires_at")?,
        consumed_at: optional_timestamp_from_postgres(row, "consumed_at")?,
        created_at: timestamp_from_postgres(row, "created_at")?,
        updated_at: timestamp_from_postgres(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn recovery_from_sqlite(row: &SqliteRow) -> Result<RecoveryToken, RepositoryError> {
    Ok(RecoveryToken {
        id: parse_id(row, "id")?,
        organization_id: parse_id(row, "organization_id")?,
        user_id: parse_id(row, "user_id")?,
        expires_at: timestamp_from_sqlite(row, "expires_at")?,
        consumed_at: optional_timestamp_from_sqlite(row, "consumed_at")?,
        created_at: timestamp_from_sqlite(row, "created_at")?,
        updated_at: timestamp_from_sqlite(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn audit_from_postgres(row: &PgRow) -> Result<AuditEvent, RepositoryError> {
    let metadata: Value = row.try_get("metadata").map_err(map_sqlx_error)?;
    Ok(AuditEvent {
        id: AuditEventId::from_uuid(row.try_get("id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        organization_id: OrganizationId::from_uuid(
            row.try_get("organization_id").map_err(map_sqlx_error)?,
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        project_id: row
            .try_get::<Option<uuid::Uuid>, _>("project_id")
            .map_err(map_sqlx_error)?
            .map(ProjectId::from_uuid)
            .transpose()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        actor_type: parse_actor_type(row.try_get("actor_type").map_err(map_sqlx_error)?)?,
        actor_id: row
            .try_get::<Option<uuid::Uuid>, _>("actor_id")
            .map_err(map_sqlx_error)?
            .map(UserId::from_uuid)
            .transpose()
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        action: row.try_get("action").map_err(map_sqlx_error)?,
        resource_type: row.try_get("resource_type").map_err(map_sqlx_error)?,
        resource_id: ResourceId::from_uuid(row.try_get("resource_id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        request_id: OperationId::from_uuid(row.try_get("request_id").map_err(map_sqlx_error)?)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        metadata: parse_audit_metadata(&metadata)?,
        occurred_at: timestamp_from_postgres(row, "occurred_at")?,
    })
}

fn audit_from_sqlite(row: &SqliteRow) -> Result<AuditEvent, RepositoryError> {
    let metadata_text: String = row.try_get("metadata").map_err(map_sqlx_error)?;
    let metadata =
        serde_json::from_str(&metadata_text).map_err(|_| RepositoryError::UnknownInfrastructure)?;
    Ok(AuditEvent {
        id: parse_id(row, "id")?,
        organization_id: parse_id(row, "organization_id")?,
        project_id: parse_optional_id(row, "project_id")?,
        actor_type: parse_actor_type(row.try_get("actor_type").map_err(map_sqlx_error)?)?,
        actor_id: parse_optional_id(row, "actor_id")?,
        action: row.try_get("action").map_err(map_sqlx_error)?,
        resource_type: row.try_get("resource_type").map_err(map_sqlx_error)?,
        resource_id: parse_id(row, "resource_id")?,
        request_id: parse_id(row, "request_id")?,
        metadata: parse_audit_metadata(&metadata)?,
        occurred_at: timestamp_from_sqlite(row, "occurred_at")?,
    })
}

fn parse_id<T>(row: &SqliteRow, column: &str) -> Result<T, RepositoryError>
where
    T: FromStr,
{
    row.try_get::<String, _>(column)
        .map_err(map_sqlx_error)?
        .parse()
        .map_err(|_| RepositoryError::UnknownInfrastructure)
}

fn parse_optional_id<T>(row: &SqliteRow, column: &str) -> Result<Option<T>, RepositoryError>
where
    T: FromStr,
{
    row.try_get::<Option<String>, _>(column)
        .map_err(map_sqlx_error)?
        .map(|value| {
            value
                .parse()
                .map_err(|_| RepositoryError::UnknownInfrastructure)
        })
        .transpose()
}

fn postgres_time(timestamp: UtcTimestamp) -> Result<OffsetDateTime, RepositoryError> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp.unix_micros()) * 1_000)
        .map_err(|_| RepositoryError::ConstraintViolation)
}

fn timestamp_from_postgres(row: &PgRow, column: &str) -> Result<UtcTimestamp, RepositoryError> {
    let timestamp: OffsetDateTime = row.try_get(column).map_err(map_sqlx_error)?;
    let micros = timestamp.unix_timestamp_nanos() / 1_000;
    i64::try_from(micros)
        .map(UtcTimestamp::from_unix_micros)
        .map_err(|_| RepositoryError::UnknownInfrastructure)
}

fn optional_timestamp_from_postgres(
    row: &PgRow,
    column: &str,
) -> Result<Option<UtcTimestamp>, RepositoryError> {
    let timestamp: Option<OffsetDateTime> = row.try_get(column).map_err(map_sqlx_error)?;
    timestamp
        .map(|value| {
            i64::try_from(value.unix_timestamp_nanos() / 1_000)
                .map(UtcTimestamp::from_unix_micros)
                .map_err(|_| RepositoryError::UnknownInfrastructure)
        })
        .transpose()
}

fn timestamp_from_sqlite(row: &SqliteRow, column: &str) -> Result<UtcTimestamp, RepositoryError> {
    row.try_get(column)
        .map(UtcTimestamp::from_unix_micros)
        .map_err(map_sqlx_error)
}

fn optional_timestamp_from_sqlite(
    row: &SqliteRow,
    column: &str,
) -> Result<Option<UtcTimestamp>, RepositoryError> {
    row.try_get::<Option<i64>, _>(column)
        .map(|value| value.map(UtcTimestamp::from_unix_micros))
        .map_err(map_sqlx_error)
}

fn parse_actor_type(value: &str) -> Result<AuditActorType, RepositoryError> {
    match value {
        "system" => Ok(AuditActorType::System),
        "local_cli" => Ok(AuditActorType::LocalCli),
        _ => Err(RepositoryError::UnknownInfrastructure),
    }
}

fn audit_metadata_json(metadata: &BootstrapAuditMetadata) -> Value {
    json!({
        "organization_id": metadata.organization_id.to_string(),
        "project_id": metadata.project_id.to_string(),
        "user_id": metadata.user_id.to_string(),
        "membership_id": metadata.membership_id.to_string(),
        "redacted": true
    })
}

fn parse_audit_metadata(value: &Value) -> Result<BootstrapAuditMetadata, RepositoryError> {
    Ok(BootstrapAuditMetadata {
        organization_id: parse_json_id(value, "organization_id")?,
        project_id: parse_json_id(value, "project_id")?,
        user_id: parse_json_id(value, "user_id")?,
        membership_id: parse_json_id(value, "membership_id")?,
    })
}

fn parse_json_id<T>(value: &Value, key: &str) -> Result<T, RepositoryError>
where
    T: FromStr,
{
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(RepositoryError::UnknownInfrastructure)?
        .parse()
        .map_err(|_| RepositoryError::UnknownInfrastructure)
}

fn validate_bounded_text(value: &str, maximum_characters: usize) -> Result<(), RepositoryError> {
    if value.chars().count() > maximum_characters {
        Err(RepositoryError::ConstraintViolation)
    } else {
        Ok(())
    }
}

fn map_sqlx_error(error: sqlx::Error) -> RepositoryError {
    match error {
        sqlx::Error::RowNotFound => RepositoryError::NotFound,
        sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::WorkerCrashed
        | sqlx::Error::Io(_)
        | sqlx::Error::Tls(_) => RepositoryError::DatabaseUnavailable,
        sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
            RepositoryError::AlreadyExists
        }
        sqlx::Error::Database(database_error)
            if database_error.is_foreign_key_violation() || database_error.is_check_violation() =>
        {
            RepositoryError::ConstraintViolation
        }
        sqlx::Error::Database(database_error)
            if database_error.code().as_deref() == Some("22001") =>
        {
            RepositoryError::ConstraintViolation
        }
        _ => RepositoryError::UnknownInfrastructure,
    }
}
