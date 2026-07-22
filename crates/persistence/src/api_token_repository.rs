use std::str::FromStr;

use async_trait::async_trait;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row, SqliteConnection, postgres::PgRow, sqlite::SqliteRow};
use takt_application::api_token::{
    API_TOKEN_CREATED_AUDIT_ACTION, API_TOKEN_REVOKED_AUDIT_ACTION, API_TOKEN_UPDATED_AUDIT_ACTION,
    ApiTokenCreateIdempotencyError, ApiTokenCreateIdempotencyRepository,
    ApiTokenCreateIdempotencyResult, ApiTokenHash, ApiTokenIdempotencyContext,
    ApiTokenLifecycleRepository, ApiTokenListQuery, ApiTokenMutationIdempotencyError,
    ApiTokenMutationIdempotencyRepository, ApiTokenMutationIdempotencyResult, ApiTokenPatch,
    ApiTokenStore, ApiTokenWriteMethod, CreateApiTokenIdempotencyPlan, CreateApiTokenPlan,
    EncryptedApiTokenReplay, NewApiToken, RevokeApiTokenPlan, StoredApiToken,
    StoredApiTokenCreateReplay, StoredApiTokenMutationResult, UpdateApiTokenIdempotencyPlan,
    UpdateApiTokenPlan, validate_new_api_token,
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
const UPDATE_PG: &str = concat!(
    "UPDATE api_tokens SET name = COALESCE($3, name), expires_at = CASE WHEN $4 THEN $5 ELSE expires_at END, ip_networks = COALESCE($6, ip_networks), updated_at = $7, version = version + 1 WHERE id = $1 AND version = $2 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > $7) AND updated_at <= $7 RETURNING ",
    token_columns!()
);
const UPDATE_SQLITE: &str = concat!(
    "UPDATE api_tokens SET name = COALESCE(?3, name), expires_at = CASE WHEN ?4 THEN ?5 ELSE expires_at END, ip_networks = COALESCE(?6, ip_networks), updated_at = ?7, version = version + 1 WHERE id = ?1 AND version = ?2 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?7) AND updated_at <= ?7 RETURNING ",
    token_columns!()
);
const REVOKE_PG: &str = concat!(
    "UPDATE api_tokens SET revoked_at = $3, updated_at = $3, version = version + 1 WHERE id = $1 AND version = $2 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > $3) AND updated_at <= $3 RETURNING ",
    token_columns!()
);
const REVOKE_SQLITE: &str = concat!(
    "UPDATE api_tokens SET revoked_at = ?3, updated_at = ?3, version = version + 1 WHERE id = ?1 AND version = ?2 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?3) AND updated_at <= ?3 RETURNING ",
    token_columns!()
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
                insert_token_postgres(&mut transaction, &plan.token, &scopes, &networks).await?;
                insert_audit_postgres(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                insert_token_sqlite(&mut transaction, &plan.token, &scopes, &networks).await?;
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

#[async_trait]
impl ApiTokenCreateIdempotencyRepository for SqlxRepository {
    async fn create_api_token_idempotent(
        &self,
        plan: CreateApiTokenIdempotencyPlan,
    ) -> Result<ApiTokenCreateIdempotencyResult, ApiTokenCreateIdempotencyError> {
        validate_idempotent_create(&plan)?;
        let scopes = scopes_json(&plan.create.token.scopes);
        let networks = networks_json(&plan.create.token.ip_networks);
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                delete_expired_postgres(&mut transaction, &plan.context).await?;
                let reserved = reserve_postgres(&mut transaction, &plan.context).await?;
                if !reserved {
                    let replay = replay_postgres(&mut transaction, &plan).await?;
                    transaction.commit().await.map_err(map_sqlx_error)?;
                    return Ok(ApiTokenCreateIdempotencyResult::Replay(replay));
                }
                insert_token_postgres(&mut transaction, &plan.create.token, &scopes, &networks)
                    .await?;
                insert_audit_postgres(&mut transaction, &plan.create.audit_event).await?;
                complete_postgres(&mut transaction, &plan).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                delete_expired_sqlite(&mut transaction, &plan.context).await?;
                let reserved = reserve_sqlite(&mut transaction, &plan.context).await?;
                if !reserved {
                    let replay = replay_sqlite(&mut transaction, &plan).await?;
                    transaction.commit().await.map_err(map_sqlx_error)?;
                    return Ok(ApiTokenCreateIdempotencyResult::Replay(replay));
                }
                insert_token_sqlite(&mut transaction, &plan.create.token, &scopes, &networks)
                    .await?;
                insert_audit_sqlite(&mut transaction, &plan.create.audit_event).await?;
                complete_sqlite(&mut transaction, &plan).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
            }
        }
        let api_token = new_token_projection(plan.create.token);
        let replay = StoredApiTokenCreateReplay {
            api_token_id: api_token.id,
            result_version: api_token.version,
            encrypted_replay: plan.encrypted_replay,
        };
        Ok(ApiTokenCreateIdempotencyResult::Created {
            api_token: Box::new(api_token),
            replay,
        })
    }

    async fn purge_expired_api_token_idempotency(
        &self,
        now: UtcTimestamp,
        limit: u16,
    ) -> Result<u64, RepositoryError> {
        if !(1..=200).contains(&limit) {
            return Err(RepositoryError::ConstraintViolation);
        }
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                sqlx::query("DELETE FROM api_token_idempotency WHERE ctid IN (SELECT ctid FROM api_token_idempotency WHERE expires_at <= $1 ORDER BY expires_at LIMIT $2)")
                    .bind(postgres_time(now)?)
                    .bind(i64::from(limit))
                    .execute(pool)
                    .await
                    .map(|result| result.rows_affected())
                    .map_err(map_sqlx_error)
            }
            DatabasePool::Sqlite(pool) => {
                sqlx::query("DELETE FROM api_token_idempotency WHERE rowid IN (SELECT rowid FROM api_token_idempotency WHERE expires_at <= ?1 ORDER BY expires_at LIMIT ?2)")
                    .bind(now.unix_micros())
                    .bind(i64::from(limit))
                    .execute(pool)
                    .await
                    .map(|result| result.rows_affected())
                    .map_err(map_sqlx_error)
            }
        }
    }
}

fn validate_idempotent_create(
    plan: &CreateApiTokenIdempotencyPlan,
) -> Result<(), ApiTokenCreateIdempotencyError> {
    validate_new_api_token(&plan.create.token).map_err(|_| RepositoryError::ConstraintViolation)?;
    validate_audit(
        &plan.create.audit_event,
        plan.create.token.id,
        plan.create.token.organization_id,
        plan.create.token.project_id,
        API_TOKEN_CREATED_AUDIT_ACTION,
        plan.create.token.now,
    )?;
    let event = &plan.create.audit_event.event;
    if plan.context.method() != ApiTokenWriteMethod::Post
        || plan.context.created_at() != plan.create.token.now
        || plan.context.actor_type() != event.actor_type
        || event.actor_id.map(|id| id.as_uuid()) != Some(plan.context.actor_id().as_uuid())
    {
        return Err(RepositoryError::ConstraintViolation.into());
    }
    Ok(())
}

async fn insert_token_postgres(
    connection: &mut PgConnection,
    token: &NewApiToken,
    scopes: &Value,
    networks: &Value,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO api_tokens (id, organization_id, project_id, name, kind, token_prefix, token_hash, scopes, ip_networks, expires_at, created_at, updated_at, version) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11, 1)")
        .bind(token.id.as_uuid())
        .bind(token.organization_id.as_uuid())
        .bind(token.project_id.map(ProjectId::as_uuid))
        .bind(&token.name)
        .bind(token.kind.as_str())
        .bind(token.token_prefix.as_str())
        .bind(token.token_hash.expose_for_persistence())
        .bind(scopes)
        .bind(networks)
        .bind(token.expires_at.map(postgres_time).transpose()?)
        .bind(postgres_time(token.now)?)
        .execute(connection)
        .await
        .map_err(map_sqlx_error)?;
    Ok(())
}

async fn insert_token_sqlite(
    connection: &mut SqliteConnection,
    token: &NewApiToken,
    scopes: &Value,
    networks: &Value,
) -> Result<(), RepositoryError> {
    sqlx::query("INSERT INTO api_tokens (id, organization_id, project_id, name, kind, token_prefix, token_hash, scopes, ip_networks, expires_at, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, 1)")
        .bind(token.id.to_string())
        .bind(token.organization_id.to_string())
        .bind(token.project_id.map(|id| id.to_string()))
        .bind(&token.name)
        .bind(token.kind.as_str())
        .bind(token.token_prefix.as_str())
        .bind(token.token_hash.expose_for_persistence())
        .bind(scopes.to_string())
        .bind(networks.to_string())
        .bind(token.expires_at.map(UtcTimestamp::unix_micros))
        .bind(token.now.unix_micros())
        .execute(connection)
        .await
        .map_err(map_sqlx_error)?;
    Ok(())
}

async fn delete_expired_postgres(
    connection: &mut PgConnection,
    context: &ApiTokenIdempotencyContext,
) -> Result<(), RepositoryError> {
    sqlx::query("DELETE FROM api_token_idempotency WHERE actor_type = $1 AND actor_id = $2 AND method = $3 AND path = $4 AND idempotency_key = $5 AND expires_at <= $6")
        .bind(context.actor_type().as_str())
        .bind(context.actor_id().as_uuid())
        .bind(context.method().as_str())
        .bind(context.path())
        .bind(context.key())
        .bind(postgres_time(context.created_at())?)
        .execute(connection)
        .await
        .map_err(map_sqlx_error)?;
    Ok(())
}

async fn delete_expired_sqlite(
    connection: &mut SqliteConnection,
    context: &ApiTokenIdempotencyContext,
) -> Result<(), RepositoryError> {
    sqlx::query("DELETE FROM api_token_idempotency WHERE actor_type = ?1 AND actor_id = ?2 AND method = ?3 AND path = ?4 AND idempotency_key = ?5 AND expires_at <= ?6")
        .bind(context.actor_type().as_str())
        .bind(context.actor_id().to_string())
        .bind(context.method().as_str())
        .bind(context.path())
        .bind(context.key())
        .bind(context.created_at().unix_micros())
        .execute(connection)
        .await
        .map_err(map_sqlx_error)?;
    Ok(())
}

async fn reserve_postgres(
    connection: &mut PgConnection,
    context: &ApiTokenIdempotencyContext,
) -> Result<bool, RepositoryError> {
    sqlx::query("INSERT INTO api_token_idempotency (actor_type, actor_id, method, path, idempotency_key, request_hash, created_at, expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT DO NOTHING")
        .bind(context.actor_type().as_str())
        .bind(context.actor_id().as_uuid())
        .bind(context.method().as_str())
        .bind(context.path())
        .bind(context.key())
        .bind(context.request_hash().as_slice())
        .bind(postgres_time(context.created_at())?)
        .bind(postgres_time(context.expires_at())?)
        .execute(connection)
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(map_sqlx_error)
}

async fn reserve_sqlite(
    connection: &mut SqliteConnection,
    context: &ApiTokenIdempotencyContext,
) -> Result<bool, RepositoryError> {
    sqlx::query("INSERT INTO api_token_idempotency (actor_type, actor_id, method, path, idempotency_key, request_hash, created_at, expires_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT DO NOTHING")
        .bind(context.actor_type().as_str())
        .bind(context.actor_id().to_string())
        .bind(context.method().as_str())
        .bind(context.path())
        .bind(context.key())
        .bind(context.request_hash().as_slice())
        .bind(context.created_at().unix_micros())
        .bind(context.expires_at().unix_micros())
        .execute(connection)
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(map_sqlx_error)
}

async fn complete_postgres(
    connection: &mut PgConnection,
    plan: &CreateApiTokenIdempotencyPlan,
) -> Result<(), RepositoryError> {
    let affected = sqlx::query("UPDATE api_token_idempotency SET api_token_id = $6, result_version = 1, replay_key_version = $7, replay_nonce = $8, replay_ciphertext = $9 WHERE actor_type = $1 AND actor_id = $2 AND method = $3 AND path = $4 AND idempotency_key = $5 AND api_token_id IS NULL")
        .bind(plan.context.actor_type().as_str())
        .bind(plan.context.actor_id().as_uuid())
        .bind(plan.context.method().as_str())
        .bind(plan.context.path())
        .bind(plan.context.key())
        .bind(plan.create.token.id.as_uuid())
        .bind(plan.encrypted_replay.key_version())
        .bind(plan.encrypted_replay.nonce().as_slice())
        .bind(plan.encrypted_replay.ciphertext())
        .execute(connection)
        .await
        .map_err(map_sqlx_error)?
        .rows_affected();
    ensure_one_row(affected)
}

async fn complete_sqlite(
    connection: &mut SqliteConnection,
    plan: &CreateApiTokenIdempotencyPlan,
) -> Result<(), RepositoryError> {
    let affected = sqlx::query("UPDATE api_token_idempotency SET api_token_id = ?6, result_version = 1, replay_key_version = ?7, replay_nonce = ?8, replay_ciphertext = ?9 WHERE actor_type = ?1 AND actor_id = ?2 AND method = ?3 AND path = ?4 AND idempotency_key = ?5 AND api_token_id IS NULL")
        .bind(plan.context.actor_type().as_str())
        .bind(plan.context.actor_id().to_string())
        .bind(plan.context.method().as_str())
        .bind(plan.context.path())
        .bind(plan.context.key())
        .bind(plan.create.token.id.to_string())
        .bind(plan.encrypted_replay.key_version())
        .bind(plan.encrypted_replay.nonce().as_slice())
        .bind(plan.encrypted_replay.ciphertext())
        .execute(connection)
        .await
        .map_err(map_sqlx_error)?
        .rows_affected();
    ensure_one_row(affected)
}

fn ensure_one_row(affected: u64) -> Result<(), RepositoryError> {
    if affected == 1 {
        Ok(())
    } else {
        Err(RepositoryError::UnknownInfrastructure)
    }
}

async fn replay_postgres(
    connection: &mut PgConnection,
    plan: &CreateApiTokenIdempotencyPlan,
) -> Result<StoredApiTokenCreateReplay, ApiTokenCreateIdempotencyError> {
    let row = sqlx::query("SELECT request_hash, api_token_id, result_version, replay_key_version, replay_nonce, replay_ciphertext FROM api_token_idempotency WHERE actor_type = $1 AND actor_id = $2 AND method = $3 AND path = $4 AND idempotency_key = $5")
        .bind(plan.context.actor_type().as_str())
        .bind(plan.context.actor_id().as_uuid())
        .bind(plan.context.method().as_str())
        .bind(plan.context.path())
        .bind(plan.context.key())
        .fetch_one(connection)
        .await
        .map_err(map_sqlx_error)?;
    let id = row
        .try_get::<Option<uuid::Uuid>, _>("api_token_id")
        .map_err(map_sqlx_error)?
        .map(ApiTokenId::from_uuid)
        .transpose()
        .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    replay_from_parts(&row, id, &plan.context)
}

async fn replay_sqlite(
    connection: &mut SqliteConnection,
    plan: &CreateApiTokenIdempotencyPlan,
) -> Result<StoredApiTokenCreateReplay, ApiTokenCreateIdempotencyError> {
    let row = sqlx::query("SELECT request_hash, api_token_id, result_version, replay_key_version, replay_nonce, replay_ciphertext FROM api_token_idempotency WHERE actor_type = ?1 AND actor_id = ?2 AND method = ?3 AND path = ?4 AND idempotency_key = ?5")
        .bind(plan.context.actor_type().as_str())
        .bind(plan.context.actor_id().to_string())
        .bind(plan.context.method().as_str())
        .bind(plan.context.path())
        .bind(plan.context.key())
        .fetch_one(connection)
        .await
        .map_err(map_sqlx_error)?;
    let id = row
        .try_get::<Option<String>, _>("api_token_id")
        .map_err(map_sqlx_error)?
        .map(|value| ApiTokenId::from_str(&value))
        .transpose()
        .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    replay_from_parts(&row, id, &plan.context)
}

fn replay_from_parts<R: Row>(
    row: &R,
    api_token_id: Option<ApiTokenId>,
    context: &takt_application::api_token::ApiTokenIdempotencyContext,
) -> Result<StoredApiTokenCreateReplay, ApiTokenCreateIdempotencyError>
where
    for<'a> &'a str: sqlx::ColumnIndex<R>,
    for<'a> Vec<u8>: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> Option<Vec<u8>>: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> Option<i32>: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> Option<i64>: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
{
    let request_hash: Vec<u8> = row.try_get("request_hash").map_err(map_sqlx_error)?;
    if request_hash.as_slice() != context.request_hash() {
        return Err(ApiTokenCreateIdempotencyError::KeyReused);
    }
    let result_version = row
        .try_get::<Option<i64>, _>("result_version")
        .map_err(map_sqlx_error)?;
    let key_version = row
        .try_get::<Option<i32>, _>("replay_key_version")
        .map_err(map_sqlx_error)?;
    let nonce = row
        .try_get::<Option<Vec<u8>>, _>("replay_nonce")
        .map_err(map_sqlx_error)?;
    let ciphertext = row
        .try_get::<Option<Vec<u8>>, _>("replay_ciphertext")
        .map_err(map_sqlx_error)?;
    let (
        Some(api_token_id),
        Some(result_version),
        Some(key_version),
        Some(nonce),
        Some(ciphertext),
    ) = (api_token_id, result_version, key_version, nonce, ciphertext)
    else {
        return Err(RepositoryError::UnknownInfrastructure.into());
    };
    let encrypted_replay =
        EncryptedApiTokenReplay::from_persistence(key_version, nonce, ciphertext)
            .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    Ok(StoredApiTokenCreateReplay {
        api_token_id,
        result_version,
        encrypted_replay,
    })
}

fn validate_idempotent_update(
    plan: &UpdateApiTokenIdempotencyPlan,
) -> Result<(), ApiTokenMutationIdempotencyError> {
    validate_patch(&plan.update.patch, plan.update.now)?;
    let event = &plan.update.audit_event.event;
    if plan.context.method() != ApiTokenWriteMethod::Patch
        || plan.context.path() != format!("/api/v1/api-tokens/{}", plan.update.id)
        || plan.context.created_at() != plan.update.now
        || plan.context.actor_type() != event.actor_type
        || event.actor_id.map(|id| id.as_uuid()) != Some(plan.context.actor_id().as_uuid())
    {
        return Err(RepositoryError::ConstraintViolation.into());
    }
    Ok(())
}

#[async_trait]
impl ApiTokenMutationIdempotencyRepository for SqlxRepository {
    async fn update_api_token_idempotent(
        &self,
        plan: UpdateApiTokenIdempotencyPlan,
    ) -> Result<ApiTokenMutationIdempotencyResult, ApiTokenMutationIdempotencyError> {
        validate_idempotent_update(&plan)?;
        let api_token = match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                delete_expired_postgres(&mut transaction, &plan.context).await?;
                if !reserve_postgres(&mut transaction, &plan.context).await? {
                    let replay = mutation_replay_postgres(&mut transaction, &plan.context).await?;
                    transaction.commit().await.map_err(map_sqlx_error)?;
                    return Ok(ApiTokenMutationIdempotencyResult::Replay(replay));
                }
                let current = token_by_id_postgres(&mut transaction, plan.update.id).await?;
                validate_audit(
                    &plan.update.audit_event,
                    plan.update.id,
                    current.organization_id,
                    current.project_id,
                    API_TOKEN_UPDATED_AUDIT_ACTION,
                    plan.update.now,
                )?;
                let Some(api_token) = apply_update_postgres(&mut transaction, &plan.update).await?
                else {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return Err(RepositoryError::VersionConflict.into());
                };
                insert_audit_postgres(&mut transaction, &plan.update.audit_event).await?;
                complete_mutation_postgres(&mut transaction, &plan.context, &api_token).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                api_token
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                delete_expired_sqlite(&mut transaction, &plan.context).await?;
                if !reserve_sqlite(&mut transaction, &plan.context).await? {
                    let replay = mutation_replay_sqlite(&mut transaction, &plan.context).await?;
                    transaction.commit().await.map_err(map_sqlx_error)?;
                    return Ok(ApiTokenMutationIdempotencyResult::Replay(replay));
                }
                let current = token_by_id_sqlite(&mut transaction, plan.update.id).await?;
                validate_audit(
                    &plan.update.audit_event,
                    plan.update.id,
                    current.organization_id,
                    current.project_id,
                    API_TOKEN_UPDATED_AUDIT_ACTION,
                    plan.update.now,
                )?;
                let Some(api_token) = apply_update_sqlite(&mut transaction, &plan.update).await?
                else {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return Err(RepositoryError::VersionConflict.into());
                };
                insert_audit_sqlite(&mut transaction, &plan.update.audit_event).await?;
                complete_mutation_sqlite(&mut transaction, &plan.context, &api_token).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                api_token
            }
        };
        let result = StoredApiTokenMutationResult {
            api_token_id: api_token.id,
            result_version: api_token.version,
        };
        Ok(ApiTokenMutationIdempotencyResult::Mutated {
            api_token: Box::new(api_token),
            result,
        })
    }
}

async fn token_by_id_postgres(
    connection: &mut PgConnection,
    id: ApiTokenId,
) -> Result<ApiToken, RepositoryError> {
    token_from_postgres(
        &sqlx::query(SELECT_ID_PG)
            .bind(id.as_uuid())
            .fetch_one(connection)
            .await
            .map_err(map_sqlx_error)?,
    )
}

async fn token_by_id_sqlite(
    connection: &mut SqliteConnection,
    id: ApiTokenId,
) -> Result<ApiToken, RepositoryError> {
    token_from_sqlite(
        &sqlx::query(SELECT_ID_SQLITE)
            .bind(id.to_string())
            .fetch_one(connection)
            .await
            .map_err(map_sqlx_error)?,
    )
}

async fn apply_update_postgres(
    connection: &mut PgConnection,
    plan: &UpdateApiTokenPlan,
) -> Result<Option<ApiToken>, RepositoryError> {
    let networks = plan
        .patch
        .ip_networks
        .as_ref()
        .map(|values| networks_json(values));
    let row = sqlx::query(UPDATE_PG)
        .bind(plan.id.as_uuid())
        .bind(plan.expected_version)
        .bind(&plan.patch.name)
        .bind(plan.patch.expires_at.is_some())
        .bind(
            plan.patch
                .expires_at
                .flatten()
                .map(postgres_time)
                .transpose()?,
        )
        .bind(networks.as_ref())
        .bind(postgres_time(plan.now)?)
        .fetch_optional(connection)
        .await
        .map_err(map_sqlx_error)?;
    row.as_ref().map(token_from_postgres).transpose()
}

async fn apply_update_sqlite(
    connection: &mut SqliteConnection,
    plan: &UpdateApiTokenPlan,
) -> Result<Option<ApiToken>, RepositoryError> {
    let networks = plan
        .patch
        .ip_networks
        .as_ref()
        .map(|values| networks_json(values).to_string());
    let row = sqlx::query(UPDATE_SQLITE)
        .bind(plan.id.to_string())
        .bind(plan.expected_version)
        .bind(&plan.patch.name)
        .bind(plan.patch.expires_at.is_some())
        .bind(
            plan.patch
                .expires_at
                .flatten()
                .map(UtcTimestamp::unix_micros),
        )
        .bind(networks)
        .bind(plan.now.unix_micros())
        .fetch_optional(connection)
        .await
        .map_err(map_sqlx_error)?;
    row.as_ref().map(token_from_sqlite).transpose()
}

async fn complete_mutation_postgres(
    connection: &mut PgConnection,
    context: &ApiTokenIdempotencyContext,
    result: &ApiToken,
) -> Result<(), RepositoryError> {
    let affected = sqlx::query("UPDATE api_token_idempotency SET api_token_id = $6, result_version = $7 WHERE actor_type = $1 AND actor_id = $2 AND method = $3 AND path = $4 AND idempotency_key = $5 AND api_token_id IS NULL")
        .bind(context.actor_type().as_str())
        .bind(context.actor_id().as_uuid())
        .bind(context.method().as_str())
        .bind(context.path())
        .bind(context.key())
        .bind(result.id.as_uuid())
        .bind(result.version)
        .execute(connection)
        .await
        .map_err(map_sqlx_error)?
        .rows_affected();
    ensure_one_row(affected)
}

async fn complete_mutation_sqlite(
    connection: &mut SqliteConnection,
    context: &ApiTokenIdempotencyContext,
    result: &ApiToken,
) -> Result<(), RepositoryError> {
    let affected = sqlx::query("UPDATE api_token_idempotency SET api_token_id = ?6, result_version = ?7 WHERE actor_type = ?1 AND actor_id = ?2 AND method = ?3 AND path = ?4 AND idempotency_key = ?5 AND api_token_id IS NULL")
        .bind(context.actor_type().as_str())
        .bind(context.actor_id().to_string())
        .bind(context.method().as_str())
        .bind(context.path())
        .bind(context.key())
        .bind(result.id.to_string())
        .bind(result.version)
        .execute(connection)
        .await
        .map_err(map_sqlx_error)?
        .rows_affected();
    ensure_one_row(affected)
}

async fn mutation_replay_postgres(
    connection: &mut PgConnection,
    context: &ApiTokenIdempotencyContext,
) -> Result<StoredApiTokenMutationResult, ApiTokenMutationIdempotencyError> {
    let row = sqlx::query("SELECT request_hash, api_token_id, result_version FROM api_token_idempotency WHERE actor_type = $1 AND actor_id = $2 AND method = $3 AND path = $4 AND idempotency_key = $5")
        .bind(context.actor_type().as_str())
        .bind(context.actor_id().as_uuid())
        .bind(context.method().as_str())
        .bind(context.path())
        .bind(context.key())
        .fetch_one(connection)
        .await
        .map_err(map_sqlx_error)?;
    let id = row
        .try_get::<Option<uuid::Uuid>, _>("api_token_id")
        .map_err(map_sqlx_error)?
        .map(ApiTokenId::from_uuid)
        .transpose()
        .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    mutation_replay_from_parts(&row, id, context)
}

async fn mutation_replay_sqlite(
    connection: &mut SqliteConnection,
    context: &ApiTokenIdempotencyContext,
) -> Result<StoredApiTokenMutationResult, ApiTokenMutationIdempotencyError> {
    let row = sqlx::query("SELECT request_hash, api_token_id, result_version FROM api_token_idempotency WHERE actor_type = ?1 AND actor_id = ?2 AND method = ?3 AND path = ?4 AND idempotency_key = ?5")
        .bind(context.actor_type().as_str())
        .bind(context.actor_id().to_string())
        .bind(context.method().as_str())
        .bind(context.path())
        .bind(context.key())
        .fetch_one(connection)
        .await
        .map_err(map_sqlx_error)?;
    let id = row
        .try_get::<Option<String>, _>("api_token_id")
        .map_err(map_sqlx_error)?
        .map(|value| ApiTokenId::from_str(&value))
        .transpose()
        .map_err(|_| RepositoryError::UnknownInfrastructure)?;
    mutation_replay_from_parts(&row, id, context)
}

fn mutation_replay_from_parts<R: Row>(
    row: &R,
    api_token_id: Option<ApiTokenId>,
    context: &ApiTokenIdempotencyContext,
) -> Result<StoredApiTokenMutationResult, ApiTokenMutationIdempotencyError>
where
    for<'a> &'a str: sqlx::ColumnIndex<R>,
    for<'a> Vec<u8>: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    for<'a> Option<i64>: sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
{
    let request_hash: Vec<u8> = row.try_get("request_hash").map_err(map_sqlx_error)?;
    if request_hash.as_slice() != context.request_hash() {
        return Err(ApiTokenMutationIdempotencyError::KeyReused);
    }
    let result_version = row
        .try_get::<Option<i64>, _>("result_version")
        .map_err(map_sqlx_error)?;
    let (Some(api_token_id), Some(result_version)) = (api_token_id, result_version) else {
        return Err(RepositoryError::UnknownInfrastructure.into());
    };
    Ok(StoredApiTokenMutationResult {
        api_token_id,
        result_version,
    })
}

#[async_trait]
impl ApiTokenLifecycleRepository for SqlxRepository {
    async fn update_api_token(
        &self,
        plan: UpdateApiTokenPlan,
    ) -> Result<ApiToken, RepositoryError> {
        validate_patch(&plan.patch, plan.now)?;
        let current = self.api_token_by_id(plan.id).await?;
        validate_audit(
            &plan.audit_event,
            plan.id,
            current.organization_id,
            current.project_id,
            API_TOKEN_UPDATED_AUDIT_ACTION,
            plan.now,
        )?;
        let networks = plan
            .patch
            .ip_networks
            .as_ref()
            .map(|values| networks_json(values));
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(UPDATE_PG)
                    .bind(plan.id.as_uuid())
                    .bind(plan.expected_version)
                    .bind(&plan.patch.name)
                    .bind(plan.patch.expires_at.is_some())
                    .bind(
                        plan.patch
                            .expires_at
                            .flatten()
                            .map(postgres_time)
                            .transpose()?,
                    )
                    .bind(networks.as_ref())
                    .bind(postgres_time(plan.now)?)
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(map_sqlx_error)?;
                let Some(row) = row else {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return classify_mutation(self, plan.id).await;
                };
                insert_audit_postgres(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                token_from_postgres(&row)
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(UPDATE_SQLITE)
                    .bind(plan.id.to_string())
                    .bind(plan.expected_version)
                    .bind(&plan.patch.name)
                    .bind(plan.patch.expires_at.is_some())
                    .bind(
                        plan.patch
                            .expires_at
                            .flatten()
                            .map(UtcTimestamp::unix_micros),
                    )
                    .bind(networks.map(|value| value.to_string()))
                    .bind(plan.now.unix_micros())
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(map_sqlx_error)?;
                let Some(row) = row else {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return classify_mutation(self, plan.id).await;
                };
                insert_audit_sqlite(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                token_from_sqlite(&row)
            }
        }
    }

    async fn revoke_api_token(
        &self,
        plan: RevokeApiTokenPlan,
    ) -> Result<ApiToken, RepositoryError> {
        let current = self.api_token_by_id(plan.id).await?;
        validate_audit(
            &plan.audit_event,
            plan.id,
            current.organization_id,
            current.project_id,
            API_TOKEN_REVOKED_AUDIT_ACTION,
            plan.now,
        )?;
        match &self.database.pool {
            DatabasePool::PostgreSql(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(REVOKE_PG)
                    .bind(plan.id.as_uuid())
                    .bind(plan.expected_version)
                    .bind(postgres_time(plan.now)?)
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(map_sqlx_error)?;
                let Some(row) = row else {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return classify_mutation(self, plan.id).await;
                };
                insert_audit_postgres(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                token_from_postgres(&row)
            }
            DatabasePool::Sqlite(pool) => {
                let mut transaction = pool.begin().await.map_err(map_sqlx_error)?;
                let row = sqlx::query(REVOKE_SQLITE)
                    .bind(plan.id.to_string())
                    .bind(plan.expected_version)
                    .bind(plan.now.unix_micros())
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(map_sqlx_error)?;
                let Some(row) = row else {
                    transaction.rollback().await.map_err(map_sqlx_error)?;
                    return classify_mutation(self, plan.id).await;
                };
                insert_audit_sqlite(&mut transaction, &plan.audit_event).await?;
                transaction.commit().await.map_err(map_sqlx_error)?;
                token_from_sqlite(&row)
            }
        }
    }

    async fn record_api_token_used(
        &self,
        id: ApiTokenId,
        now: UtcTimestamp,
    ) -> Result<(), RepositoryError> {
        let affected = match &self.database.pool {
            DatabasePool::PostgreSql(pool) => sqlx::query("UPDATE api_tokens SET last_used_at = $2, updated_at = $2, version = version + 1 WHERE id = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > $2) AND updated_at <= $2 AND (last_used_at IS NULL OR last_used_at < $2)")
                .bind(id.as_uuid()).bind(postgres_time(now)?).execute(pool).await.map_err(map_sqlx_error)?.rows_affected(),
            DatabasePool::Sqlite(pool) => sqlx::query("UPDATE api_tokens SET last_used_at = ?2, updated_at = ?2, version = version + 1 WHERE id = ?1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?2) AND updated_at <= ?2 AND (last_used_at IS NULL OR last_used_at < ?2)")
                .bind(id.to_string()).bind(now.unix_micros()).execute(pool).await.map_err(map_sqlx_error)?.rows_affected(),
        };
        if affected == 1 {
            Ok(())
        } else {
            classify_mutation(self, id).await.map(|_| ())
        }
    }
}

fn validate_patch(patch: &ApiTokenPatch, now: UtcTimestamp) -> Result<(), RepositoryError> {
    if patch.name.is_none() && patch.expires_at.is_none() && patch.ip_networks.is_none() {
        return Err(RepositoryError::ConstraintViolation);
    }
    if patch
        .name
        .as_ref()
        .is_some_and(|name| name.is_empty() || name.chars().count() > 120)
        || patch
            .expires_at
            .flatten()
            .is_some_and(|expiry| expiry <= now)
        || patch
            .ip_networks
            .as_ref()
            .is_some_and(|values| values.len() > 32)
    {
        return Err(RepositoryError::ConstraintViolation);
    }
    if let Some(values) = &patch.ip_networks {
        let mut unique = values.clone();
        unique.sort_by_key(ToString::to_string);
        unique.dedup();
        if unique.len() != values.len() {
            return Err(RepositoryError::ConstraintViolation);
        }
    }
    Ok(())
}

async fn classify_mutation(
    repository: &SqlxRepository,
    id: ApiTokenId,
) -> Result<ApiToken, RepositoryError> {
    match repository.api_token_by_id(id).await {
        Ok(_) => Err(RepositoryError::VersionConflict),
        Err(RepositoryError::NotFound) => Err(RepositoryError::NotFound),
        Err(error) => Err(error),
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
