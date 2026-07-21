#![forbid(unsafe_code)]

mod common;

use std::error::Error;

use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};
use takt_application::{BootstrapService, BootstrapStatus};
use takt_persistence::{Database, DatabaseConfig, ReadinessError, SchemaStatus, SqlxRepository};

use common::{
    FixedClock, SequenceIds, TEST_PASSWORD, TEST_RAW_RECOVERY_TOKEN, TEST_RAW_SESSION_TOKEN,
    TestPasswordHasher,
};

async fn sqlite_database(
    directory: &tempfile::TempDir,
    name: &str,
) -> Result<(Database, std::path::PathBuf), Box<dyn Error>> {
    let path = directory.path().join(name);
    let config = DatabaseConfig::sqlite_for_test(path.clone())?;
    let database = Database::connect(&config).await?;
    Ok((database, path))
}

async fn raw_connection(path: &std::path::Path) -> Result<SqliteConnection, Box<dyn Error>> {
    Ok(SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true),
    )
    .await?)
}

// PRD-DATA-002 / PRD-NFR-001: a fresh SQLite file migrates, repeats without
// drift, and is never created in the repository working directory.
#[tokio::test]
async fn sqlite_migrations_are_forward_only_and_repeatable() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (database, path) = sqlite_database(&directory, "migration.sqlite3").await?;
    assert_eq!(
        database.schema_status().await?,
        SchemaStatus::MigrationRequired { found: 0 }
    );
    assert_eq!(
        database.readiness().await,
        Err(ReadinessError::SchemaNotReady)
    );

    database.migrate().await?;
    assert_eq!(database.schema_status().await?, SchemaStatus::Current);
    database.readiness().await?;
    database.migrate().await?;

    let mut connection = raw_connection(&path).await?;
    let foreign_keys: i64 = sqlx::query("PRAGMA foreign_keys")
        .fetch_one(&mut connection)
        .await?
        .try_get(0)?;
    let journal_mode: String = sqlx::query("PRAGMA journal_mode")
        .fetch_one(&mut connection)
        .await?
        .try_get(0)?;
    assert_eq!(foreign_keys, 1);
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

    common::run_shutdown_contract(database).await
}

#[tokio::test]
// PRD-DATA-002: newer schema versions fail closed and never become ready.
async fn sqlite_rejects_unknown_newer_schema_versions() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (database, path) = sqlite_database(&directory, "newer.sqlite3").await?;
    database.migrate().await?;
    let mut connection = raw_connection(&path).await?;
    sqlx::query("UPDATE _sqlx_migrations SET version = 4 WHERE version = 3")
        .execute(&mut connection)
        .await?;
    assert_eq!(
        database.schema_status().await?,
        SchemaStatus::TooNew {
            found: 4,
            supported: 3
        }
    );
    assert_eq!(
        database.readiness().await,
        Err(ReadinessError::SchemaNotReady)
    );
    assert!(database.migrate().await.is_err());
    Ok(())
}

#[tokio::test]
// PRD-DATA-001: SQLite executes the same repository behavior as PostgreSQL.
async fn sqlite_runs_the_shared_repository_contract() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (database, path) = sqlite_database(&directory, "repository.sqlite3").await?;
    database.migrate().await?;
    let repository = SqlxRepository::new(database);
    common::run_repository_contract(&repository).await?;
    common::run_session_repository_contract(&repository).await?;
    common::run_recovery_repository_contract(&repository).await?;

    let mut connection = raw_connection(&path).await?;
    let row = sqlx::query(
        "SELECT group_concat(token_digest || ' ' || csrf_digest, ' ') AS stored, (SELECT group_concat(metadata, ' ') FROM audit_events WHERE resource_type = 'session') AS metadata FROM sessions",
    )
    .fetch_one(&mut connection)
    .await?;
    common::assert_persisted_session_is_redacted(
        &row.try_get::<String, _>("stored")?,
        &row.try_get::<String, _>("metadata")?,
    );
    assert!(
        sqlx::query("UPDATE sessions SET token_digest = ?1")
            .bind(TEST_RAW_SESSION_TOKEN)
            .execute(&mut connection)
            .await
            .is_err(),
        "SQLite must reject non-digest session values"
    );
    let recovery_row = sqlx::query(
        "SELECT group_concat(token_digest, ' ') AS stored, (SELECT group_concat(metadata, ' ') FROM audit_events WHERE resource_type = 'recovery_token') AS metadata FROM recovery_tokens",
    )
    .fetch_one(&mut connection)
    .await?;
    common::assert_persisted_recovery_is_redacted(
        &recovery_row.try_get::<String, _>("stored")?,
        &recovery_row.try_get::<String, _>("metadata")?,
    );
    assert!(
        sqlx::query("UPDATE recovery_tokens SET token_digest = ?1")
            .bind(TEST_RAW_RECOVERY_TOKEN)
            .execute(&mut connection)
            .await
            .is_err(),
        "SQLite must reject non-digest recovery values"
    );
    let credential: String =
        sqlx::query("SELECT password_hash FROM local_credentials WHERE user_id = ?1")
            .bind(common::resource_id(3)?.to_string())
            .fetch_one(&mut connection)
            .await?
            .try_get("password_hash")?;
    common::assert_replacement_password_hash(&credential)?;
    Ok(())
}

#[tokio::test]
// PRD-DATA-004 / PRD-IAM-001: bootstrap persists typed identity metadata.
async fn sqlite_bootstrap_is_atomic_idempotent_and_redacted() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (database, path) = sqlite_database(&directory, "bootstrap.sqlite3").await?;
    database.migrate().await?;
    let repository = SqlxRepository::new(database.clone());
    let first = common::run_bootstrap_contract(&repository).await?;

    let mut connection = raw_connection(&path).await?;
    let password_hash: String = sqlx::query("SELECT password_hash FROM local_credentials")
        .fetch_one(&mut connection)
        .await?
        .try_get("password_hash")?;
    assert!(password_hash.starts_with("$argon2id$"));
    assert!(!password_hash.contains(TEST_PASSWORD));
    let audit_metadata: String = sqlx::query("SELECT metadata FROM audit_events")
        .fetch_one(&mut connection)
        .await?
        .try_get("metadata")?;
    assert!(!audit_metadata.contains(TEST_PASSWORD));
    assert!(!audit_metadata.contains("argon2"));
    assert_eq!(
        sqlx::query("SELECT COUNT(*) AS count FROM audit_events")
            .fetch_one(&mut connection)
            .await?
            .try_get::<i64, _>("count")?,
        1
    );
    assert!(
        sqlx::query("UPDATE audit_events SET action = 'mutated'")
            .execute(&mut connection)
            .await
            .is_err(),
        "audit rows must be immutable in SQLite"
    );
    assert_eq!(
        first.resources.organization.id.as_uuid().get_version_num(),
        7
    );
    Ok(())
}

#[tokio::test]
async fn sqlite_concurrent_bootstraps_create_one_administrator() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (database, path) = sqlite_database(&directory, "concurrent.sqlite3").await?;
    database.migrate().await?;
    let repository = SqlxRepository::new(database);
    let hasher = TestPasswordHasher::new();
    let clock = FixedClock;
    let ids = SequenceIds::new(1_000);
    let service = BootstrapService::new(&repository, &hasher, &clock, &ids);

    let (left, right) = tokio::join!(
        service.execute("admin", TEST_PASSWORD),
        service.execute("admin", TEST_PASSWORD)
    );
    let left = left?;
    let right = right?;
    assert!(matches!(
        (left.status, right.status),
        (BootstrapStatus::Created, BootstrapStatus::AlreadyPresent)
            | (BootstrapStatus::AlreadyPresent, BootstrapStatus::Created)
    ));
    let mut connection = raw_connection(&path).await?;
    assert_eq!(
        sqlx::query("SELECT COUNT(*) AS count FROM users")
            .fetch_one(&mut connection)
            .await?
            .try_get::<i64, _>("count")?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn sqlite_bootstrap_rolls_back_a_mid_transaction_failure() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (database, path) = sqlite_database(&directory, "rollback.sqlite3").await?;
    database.migrate().await?;
    let mut connection = raw_connection(&path).await?;
    sqlx::query(
        "CREATE TRIGGER fail_bootstrap_membership BEFORE INSERT ON memberships BEGIN SELECT RAISE(ABORT, 'controlled failure'); END",
    )
    .execute(&mut connection)
    .await?;

    let repository = SqlxRepository::new(database);
    let hasher = TestPasswordHasher::new();
    let clock = FixedClock;
    let ids = SequenceIds::new(2_000);
    let service = BootstrapService::new(&repository, &hasher, &clock, &ids);
    assert!(service.execute("admin", TEST_PASSWORD).await.is_err());

    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM organizations) AS organizations, (SELECT COUNT(*) FROM projects) AS projects, (SELECT COUNT(*) FROM users) AS users, (SELECT COUNT(*) FROM local_credentials) AS local_credentials, (SELECT COUNT(*) FROM memberships) AS memberships, (SELECT COUNT(*) FROM audit_events) AS audit_events",
    )
    .fetch_one(&mut connection)
    .await?;
    for column in [
        "organizations",
        "projects",
        "users",
        "local_credentials",
        "memberships",
        "audit_events",
    ] {
        assert_eq!(
            row.try_get::<i64, _>(column)?,
            0,
            "{column} must be rolled back"
        );
    }
    Ok(())
}
