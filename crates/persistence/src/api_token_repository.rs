use std::str::FromStr;

use async_trait::async_trait;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row, SqliteConnection, postgres::PgRow, sqlite::SqliteRow};
use takt_application::api_token::{
    API_TOKEN_CREATED_AUDIT_ACTION, ApiTokenHash, ApiTokenListQuery, ApiTokenStore,
    CreateApiTokenPlan, NewApiToken, StoredApiToken, validate_new_api_token,
};
use takt_application::{NewAuditEvent, RepositoryError};
use takt_domain::{
    ApiTokenId, OrganizationId, ProjectId, UtcTimestamp,
    api_token::{ApiToken, ApiTokenKind, ApiTokenPrefix, ApiTokenScope, IpNetwork},
};
use time::OffsetDateTime;

use crate::{
    database::DatabasePool,
    repository::{SqlxRepository, map_sqlx_error, postgres_time},
};

macro_rules! token_columns {
    () => {
        "id, organization_id, project_id, name, kind, token_prefix, scopes, ip_networks, expires_at, last_used_at, revoked_at, created_at, updated_at, version"
    };
}
const SELECT_ID_PG: &str = concat!(
    "SELECT ",
    token_columns!(),
    " FROM api_tokens WHERE id = $1"
);
const SELECT_ID_SQLITE: &str = concat!(
    "SELECT ",
    token_columns!(),
    " FROM api_tokens WHERE id = ?1"
);
const SELECT_PREFIX_PG: &str = concat!(
    "SELECT ",
    token_columns!(),
    ", token_hash FROM api_tokens WHERE token_prefix = $1"
);
const SELECT_PREFIX_SQLITE: &str = concat!(
    "SELECT ",
    token_columns!(),
    ", token_hash FROM api_tokens WHERE token_prefix = ?1"
);
const LIST_PG: &str = concat!(
    "SELECT ",
    token_columns!(),
    " FROM api_tokens WHERE organization_id = $1 AND ($2::uuid IS NULL OR project_id = $2) AND ($3::text IS NULL OR kind = $3) AND ($4::text IS NULL OR ($4 = 'active' AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > $6)) OR ($4 = 'revoked' AND revoked_at IS NOT NULL) OR ($4 = 'expired' AND revoked_at IS NULL AND expires_at <= $6)) AND ($5::text IS NULL OR scopes @> to_jsonb(ARRAY[$5::text])) AND ($7::timestamptz IS NULL OR created_at < $7 OR (created_at = $7 AND id < $8)) ORDER BY created_at DESC, id DESC LIMIT $9"
);
const LIST_SQLITE: &str = concat!(
    "SELECT ",
    token_columns!(),
    " FROM api_tokens WHERE organization_id = ?1 AND (?2 IS NULL OR project_id = ?2) AND (?3 IS NULL OR kind = ?3) AND (?4 IS NULL OR (?4 = 'active' AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?6)) OR (?4 = 'revoked' AND revoked_at IS NOT NULL) OR (?4 = 'expired' AND revoked_at IS NULL AND expires_at <= ?6)) AND (?5 IS NULL OR EXISTS (SELECT 1 FROM json_each(api_tokens.scopes) WHERE value = ?5)) AND (?7 IS NULL OR created_at < ?7 OR (created_at = ?7 AND id < ?8)) ORDER BY created_at DESC, id DESC LIMIT ?9"
);

#[async_trait]
impl ApiTokenStore for SqlxRepository {
    async fn create_api_token(
        &self,
        plan: CreateApiTokenPlan,
    ) -> Result<ApiToken, RepositoryError> {
        validate_new_api_token(&plan.token).map_err(|_| RepositoryError::ConstraintViolation)?;
        validate_audit(
            &plan.audit_event,
            plan.token.id,
            plan.token.organization_id,
            plan.token.project_id,
            API_TOKEN_CREATED_AUDIT_ACTION,
            plan.token.now,
        )?;
        let scopes = scopes_json(&plan.token.scopes);
        let networks = networks_json(&plan.token.ip_networks);
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                sqlx::query("INSERT INTO api_tokens (id, organization_id, project_id, name, kind, token_prefix, token_hash, scopes, ip_networks, expires_at, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11, 1)")
                    .bind(plan.token.id.as_uuid())
                    .bind(plan.token.organization_id.as_uuid())
                    .bind(plan.token.project_id.map(ProjectId::as_uuid))
                    .bind(&plan.token.name)
                    .bind(plan.token.kind.as_str())
                    .bind(plan.token.token_prefix.as_str())
                    .bind(plan.token.token_hash.expose_for_persistence())
                    .bind(&scopes)
                    .bind(&networks)
                    .bind(plan.token.expires_at.map(postgres_time).transpose()?)
                    .bind(postgres_time(plan.token.now)?)
                    .execute(&mut *transaction).await.map_err(map_sqlx_error)?;
                insert_audit_postgres(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                sqlx::query("INSERT INTO api_tokens (id, organization_id, project_id, name, kind, token_prefix, token_hash, scopes, ip_networks, expires_at, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, 1)")
                    .bind(plan.token.id.to_string())
                    .bind(plan.token.organization_id.to_string())
                    .bind(plan.token.project_id.map(|id| id.to_string()))
                    .bind(&plan.token.name)
                    .bind(plan.token.kind.as_str())
                    .bind(plan.token.token_prefix.as_str())
                    .bind(plan.token.token_hash.expose_for_persistence())
                    .bind(scopes.to_string())
                    .bind(networks.to_string())
                    .bind(plan.token.expires_at.map(UtcTimestamp::unix_micros))
                    .bind(plan.token.now.unix_micros())
                    .execute(&mut *transaction).await.map_err(map_sqlx_error)?;
                insert_audit_sqlite(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
            }
        }
        Ok(new_token_projection(plan.token))
    }

    async fn api_token_by_id(&self, id: ApiTokenId) -> Result<ApiToken, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => token_from_postgres(
                &sqlx::query(SELECT_ID_PG)
                    .bind(id.as_uuid())
                    .fetch_one(pool)
                    .await
                    .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => token_from_sqlite(
                &sqlx::query(SELECT_ID_SQLITE)
                    .bind(id.to_string())
                    .fetch_one(pool)
                    .await
                    .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn api_token_by_prefix(
        &self,
        prefix: &ApiTokenPrefix,
    ) -> Result<StoredApiToken, RepositoryError> {
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => stored_from_postgres(
                &sqlx::query(SELECT_PREFIX_PG)
                    .bind(prefix.as_str())
                    .fetch_one(pool)
                    .await
                    .map_err(map_sqlx_error)?,
            ),
            DatabasePool::Sqlite(pool) => stored_from_sqlite(
                &sqlx::query(SELECT_PREFIX_SQLITE)
                    .bind(prefix.as_str())
                    .fetch_one(pool)
                    .await
                    .map_err(map_sqlx_error)?,
            ),
        }
    }

    async fn list_api_tokens(
        &self,
        query: ApiTokenListQuery,
    ) -> Result<Vec<ApiToken>, RepositoryError> {
        if !(1..=200).contains(&query.limit) {
            return Err(RepositoryError::ConstraintViolation);
        }
        let kind = query.kind.map(ApiTokenKind::as_str);
        let status = query.status.map(|value| value.as_str());
        let scope = query.scope.as_ref().map(ApiTokenScope::as_str);
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let (before_time, before_id) = query
                    .before
                    .map_or((None, None), |(time, id)| (Some(time), Some(id.as_uuid())));
                let rows = sqlx::query(LIST_PG)
                    .bind(query.organization_id.as_uuid())
                    .bind(query.project_id.map(ProjectId::as_uuid))
                    .bind(kind)
                    .bind(status)
                    .bind(scope)
                    .bind(postgres_time(query.now)?)
                    .bind(before_time.map(postgres_time).transpose()?)
                    .bind(before_id)
                    .bind(i64::from(query.limit))
                    .fetch_all(pool)
                    .await
                    .map_err(map_sqlx_error)?;
                rows.iter().map(token_from_postgres).collect()
            }
            DatabasePool::Sqlite(pool) => {
                let (before_time, before_id) = query.before.map_or((None, None), |(time, id)| {
                    (Some(time.unix_micros()), Some(id.to_string()))
                });
                let rows = sqlx::query(LIST_SQLITE)
                    .bind(query.organization_id.to_string())
                    .bind(query.project_id.map(|id| id.to_string()))
                    .bind(kind)
                    .bind(status)
                    .bind(scope)
                    .bind(query.now.unix_micros())
                    .bind(before_time)
                    .bind(before_id)
                    .bind(i64::from(query.limit))
                    .fetch_all(pool)
                    .await
                    .map_err(map_sqlx_error)?;
                rows.iter().map(token_from_sqlite).collect()
            }
        }
    }
}

fn validate_audit(
    event: &NewAuditEvent,
    id: ApiTokenId,
    organization_id: OrganizationId,
    project_id: Option<ProjectId>,
    action: &str,
    now: UtcTimestamp,
) -> Result<(), RepositoryError> {
    let event = &event.event;
    if event.organization_id != organization_id
        || event.project_id != project_id
        || event.organization_id != event.metadata.organization_id
        || event.action != action
        || event.resource_type != "api_token"
        || event.resource_id.as_uuid() != id.as_uuid()
        || event.occurred_at != now
        || event.actor_id.is_none()
    {
        return Err(RepositoryError::ConstraintViolation);
    }
    Ok(())
}

fn new_token_projection(value: NewApiToken) -> ApiToken {
    ApiToken {
        id: value.id,
        organization_id: value.organization_id,
        project_id: value.project_id,
        name: value.name,
        kind: value.kind,
        token_prefix: value.token_prefix,
        scopes: value.scopes,
        ip_networks: value.ip_networks,
        expires_at: value.expires_at,
        last_used_at: None,
        revoked_at: None,
        created_at: value.now,
        updated_at: value.now,
        version: 1,
    }
}

fn scopes_json(values: &[ApiTokenScope]) -> Value {
    json!(values.iter().map(ApiTokenScope::as_str).collect::<Vec<_>>())
}
fn networks_json(values: &[IpNetwork]) -> Value {
    json!(values.iter().map(ToString::to_string).collect::<Vec<_>>())
}

fn token_from_postgres(row: &PgRow) -> Result<ApiToken, RepositoryError> {
    token_from_parts(
        row,
        postgres_timestamp,
        postgres_optional_timestamp,
        |row, column| row.try_get::<Value, _>(column).map_err(map_sqlx_error),
        |row, column| {
            row.try_get::<uuid::Uuid, _>(column)
                .map_err(map_sqlx_error)
                .and_then(|id| {
                    ApiTokenId::from_uuid(id).map_err(|_| RepositoryError::UnknownInfrastructure)
                })
        },
        |row, column| {
            row.try_get::<uuid::Uuid, _>(column)
                .map_err(map_sqlx_error)
                .and_then(|id| {
                    OrganizationId::from_uuid(id)
                        .map_err(|_| RepositoryError::UnknownInfrastructure)
                })
        },
        |row, column| {
            row.try_get::<Option<uuid::Uuid>, _>(column)
                .map_err(map_sqlx_error)
                .and_then(|id| {
                    id.map(ProjectId::from_uuid)
                        .transpose()
                        .map_err(|_| RepositoryError::UnknownInfrastructure)
                })
        },
    )
}

fn token_from_sqlite(row: &SqliteRow) -> Result<ApiToken, RepositoryError> {
    token_from_parts(
        row,
        sqlite_timestamp,
        sqlite_optional_timestamp,
        |row, column| {
            let value: String = row.try_get(column).map_err(map_sqlx_error)?;
            serde_json::from_str(&value).map_err(|_| RepositoryError::UnknownInfrastructure)
        },
        parse_sqlite_id,
        parse_sqlite_id,
        parse_optional_sqlite_id,
    )
}

fn token_from_parts<R, FTime, FOptionalTime, FJson, FTokenId, FOrganizationId, FProjectId>(
    row: &R,
    time: FTime,
    optional_time: FOptionalTime,
    json_value: FJson,
    token_id: FTokenId,
    organization_id: FOrganizationId,
    project_id: FProjectId,
) -> Result<ApiToken, RepositoryError>
where
    R: Row,
    FTime: Fn(&R, &str) -> Result<UtcTimestamp, RepositoryError>,
    FOptionalTime: Fn(&R, &str) -> Result<Option<UtcTimestamp>, RepositoryError>,
    FJson: Fn(&R, &str) -> Result<Value, RepositoryError>,
    FTokenId: Fn(&R, &str) -> Result<ApiTokenId, RepositoryError>,
    FOrganizationId: Fn(&R, &str) -> Result<OrganizationId, RepositoryError>,
    FProjectId: Fn(&R, &str) -> Result<Option<ProjectId>, RepositoryError>,
    for<'a> &'a str: sqlx::ColumnIndex<R>,
    for<'a> String: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    i64: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
{
    let scopes = parse_string_array(json_value(row, "scopes")?)?;
    let networks = parse_network_array(json_value(row, "ip_networks")?)?;
    Ok(ApiToken {
        id: token_id(row, "id")?,
        organization_id: organization_id(row, "organization_id")?,
        project_id: project_id(row, "project_id")?,
        name: row.try_get("name").map_err(map_sqlx_error)?,
        kind: ApiTokenKind::from_str(
            row.try_get::<String, _>("kind")
                .map_err(map_sqlx_error)?
                .as_str(),
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        token_prefix: ApiTokenPrefix::from_str(
            row.try_get::<String, _>("token_prefix")
                .map_err(map_sqlx_error)?
                .as_str(),
        )
        .map_err(|_| RepositoryError::UnknownInfrastructure)?,
        scopes,
        ip_networks: networks,
        expires_at: optional_time(row, "expires_at")?,
        last_used_at: optional_time(row, "last_used_at")?,
        revoked_at: optional_time(row, "revoked_at")?,
        created_at: time(row, "created_at")?,
        updated_at: time(row, "updated_at")?,
        version: row.try_get("version").map_err(map_sqlx_error)?,
    })
}

fn stored_from_postgres(row: &PgRow) -> Result<StoredApiToken, RepositoryError> {
    Ok(StoredApiToken {
        token: token_from_postgres(row)?,
        token_hash: parse_hash(row)?,
    })
}
fn stored_from_sqlite(row: &SqliteRow) -> Result<StoredApiToken, RepositoryError> {
    Ok(StoredApiToken {
        token: token_from_sqlite(row)?,
        token_hash: parse_hash(row)?,
    })
}
fn parse_hash<R: Row>(row: &R) -> Result<ApiTokenHash, RepositoryError>
where
    for<'a> &'a str: sqlx::ColumnIndex<R>,
    for<'a> String: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
{
    ApiTokenHash::from_persistence(row.try_get("token_hash").map_err(map_sqlx_error)?)
        .map_err(|_| RepositoryError::UnknownInfrastructure)
}

fn parse_string_array(value: Value) -> Result<Vec<ApiTokenScope>, RepositoryError> {
    value
        .as_array()
        .ok_or(RepositoryError::UnknownInfrastructure)?
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or(RepositoryError::UnknownInfrastructure)
                .and_then(|value| {
                    ApiTokenScope::from_str(value)
                        .map_err(|_| RepositoryError::UnknownInfrastructure)
                })
        })
        .collect()
}
fn parse_network_array(value: Value) -> Result<Vec<IpNetwork>, RepositoryError> {
    value
        .as_array()
        .ok_or(RepositoryError::UnknownInfrastructure)?
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or(RepositoryError::UnknownInfrastructure)
                .and_then(|value| {
                    IpNetwork::from_str(value).map_err(|_| RepositoryError::UnknownInfrastructure)
                })
        })
        .collect()
}

fn postgres_timestamp(row: &PgRow, column: &str) -> Result<UtcTimestamp, RepositoryError> {
    let value: OffsetDateTime = row.try_get(column).map_err(map_sqlx_error)?;
    i64::try_from(value.unix_timestamp_nanos() / 1_000)
        .map(UtcTimestamp::from_unix_micros)
        .map_err(|_| RepositoryError::UnknownInfrastructure)
}
fn postgres_optional_timestamp(
    row: &PgRow,
    column: &str,
) -> Result<Option<UtcTimestamp>, RepositoryError> {
    row.try_get::<Option<OffsetDateTime>, _>(column)
        .map_err(map_sqlx_error)?
        .map(|value| {
            i64::try_from(value.unix_timestamp_nanos() / 1_000)
                .map(UtcTimestamp::from_unix_micros)
                .map_err(|_| RepositoryError::UnknownInfrastructure)
        })
        .transpose()
}
fn sqlite_timestamp(row: &SqliteRow, column: &str) -> Result<UtcTimestamp, RepositoryError> {
    row.try_get(column)
        .map(UtcTimestamp::from_unix_micros)
        .map_err(map_sqlx_error)
}
fn sqlite_optional_timestamp(
    row: &SqliteRow,
    column: &str,
) -> Result<Option<UtcTimestamp>, RepositoryError> {
    row.try_get::<Option<i64>, _>(column)
        .map(|value| value.map(UtcTimestamp::from_unix_micros))
        .map_err(map_sqlx_error)
}
fn parse_sqlite_id<T: FromStr>(row: &SqliteRow, column: &str) -> Result<T, RepositoryError> {
    row.try_get::<String, _>(column)
        .map_err(map_sqlx_error)?
        .parse()
        .map_err(|_| RepositoryError::UnknownInfrastructure)
}
fn parse_optional_sqlite_id<T: FromStr>(
    row: &SqliteRow,
    column: &str,
) -> Result<Option<T>, RepositoryError> {
    row.try_get::<Option<String>, _>(column)
        .map_err(map_sqlx_error)?
        .map(|value| {
            value
                .parse()
                .map_err(|_| RepositoryError::UnknownInfrastructure)
        })
        .transpose()
}

fn audit_metadata(event: &NewAuditEvent) -> Value {
    json!({
    "organization_id": event.event.metadata.organization_id.to_string(),
    "project_id": event.event.metadata.project_id.to_string(), "user_id": event.event.metadata.user_id.to_string(),
    "membership_id": event.event.metadata.membership_id.to_string(), "redacted": true })
}
async fn insert_audit_postgres(
    connection: &mut PgConnection,
    event: &NewAuditEvent,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO audit_events (id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
        .bind(event.event.id.as_uuid()).bind(event.event.organization_id.as_uuid()).bind(event.event.project_id.map(ProjectId::as_uuid))
        .bind(event.event.actor_type.as_str()).bind(event.event.actor_id.map(|id| id.as_uuid())).bind(&event.event.action)
        .bind(&event.event.resource_type).bind(event.event.resource_id.as_uuid()).bind(event.event.request_id.as_uuid())
        .bind(audit_metadata(event)).bind(postgres_time(event.event.occurred_at)?).execute(connection).await.map_err(map_sqlx_error)?;
    Ok(())
}
async fn insert_audit_sqlite(
    connection: &mut SqliteConnection,
    event: &NewAuditEvent,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO audit_events (id, organization_id, project_id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata, occurred_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)")
        .bind(event.event.id.to_string()).bind(event.event.organization_id.to_string()).bind(event.event.project_id.map(|id| id.to_string()))
        .bind(event.event.actor_type.as_str()).bind(event.event.actor_id.map(|id| id.to_string())).bind(&event.event.action)
        .bind(&event.event.resource_type).bind(event.event.resource_id.to_string()).bind(event.event.request_id.to_string())
        .bind(audit_metadata(event).to_string()).bind(event.event.occurred_at.unix_micros()).execute(connection).await.map_err(map_sqlx_error)?;
    Ok(())
}
