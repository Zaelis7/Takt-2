use std::{
    error::Error,
    fmt, fs,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use sqlx::{
    PgPool, Row, SqlitePool,
    migrate::Migrator,
    postgres::{PgConnectOptions, PgPoolOptions},
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use tokio::time::timeout;

use crate::config::{ConnectionSettings, DatabaseConfig, DatabaseEngine};

static POSTGRES_MIGRATOR: Migrator = sqlx::migrate!("../../migrations/postgres");
static SQLITE_MIGRATOR: Migrator = sqlx::migrate!("../../migrations/sqlite");
const LATEST_SCHEMA_VERSION: i64 = 3;
const STATE_NOT_READY: u8 = 0;
const STATE_MIGRATING: u8 = 1;
const STATE_READY: u8 = 2;
const STATE_FAILED: u8 = 3;

#[derive(Clone)]
pub(crate) enum DatabasePool {
    PostgreSql(PgPool),
    Sqlite(SqlitePool),
}

#[derive(Clone)]
pub struct Database {
    pub(crate) pool: DatabasePool,
    engine: DatabaseEngine,
    query_timeout: Duration,
    shutdown_timeout: Duration,
    state: Arc<AtomicU8>,
}

impl Database {
    pub async fn connect(config: &DatabaseConfig) -> Result<Self, DatabaseError> {
        let settings = config.pool();
        let pool = match config.connection() {
            ConnectionSettings::PostgreSql { url } => {
                let statement_timeout = format!("{}ms", settings.query_timeout.as_millis());
                let options = PgConnectOptions::from_str(url.expose())
                    .map_err(|_| DatabaseError::InvalidConfiguration)?
                    .application_name("takt-server")
                    .options([("statement_timeout", statement_timeout)]);
                let pool = timeout(
                    settings.connection_timeout,
                    PgPoolOptions::new()
                        .max_connections(settings.max_connections)
                        .acquire_timeout(settings.connection_timeout)
                        .idle_timeout(Some(Duration::from_secs(300)))
                        .connect_with(options),
                )
                .await
                .map_err(|_| DatabaseError::Unavailable)?
                .map_err(|_| DatabaseError::Unavailable)?;
                DatabasePool::PostgreSql(pool)
            }
            ConnectionSettings::Sqlite { path } => {
                let parent = path.parent().ok_or(DatabaseError::InvalidConfiguration)?;
                fs::create_dir_all(parent).map_err(|_| DatabaseError::Unavailable)?;
                let options = SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true)
                    .foreign_keys(true)
                    .journal_mode(SqliteJournalMode::Wal)
                    .synchronous(SqliteSynchronous::Normal)
                    .busy_timeout(settings.sqlite_busy_timeout.min(settings.query_timeout));
                let pool = timeout(
                    settings.connection_timeout,
                    SqlitePoolOptions::new()
                        .max_connections(settings.max_connections)
                        .acquire_timeout(settings.connection_timeout)
                        .idle_timeout(Some(Duration::from_secs(300)))
                        .connect_with(options),
                )
                .await
                .map_err(|_| DatabaseError::Unavailable)?
                .map_err(|_| DatabaseError::Unavailable)?;
                harden_sqlite_permissions(parent, path)?;
                DatabasePool::Sqlite(pool)
            }
        };

        Ok(Self {
            pool,
            engine: config.engine(),
            query_timeout: settings.query_timeout,
            shutdown_timeout: settings.shutdown_timeout,
            state: Arc::new(AtomicU8::new(STATE_NOT_READY)),
        })
    }

    #[must_use]
    pub const fn engine(&self) -> DatabaseEngine {
        self.engine
    }

    #[must_use]
    pub const fn query_timeout(&self) -> Duration {
        self.query_timeout
    }

    pub async fn migrate(&self) -> Result<(), DatabaseError> {
        self.state.store(STATE_MIGRATING, Ordering::Release);
        let result = self.migrate_inner().await;
        self.state.store(
            if result.is_ok() {
                STATE_READY
            } else {
                STATE_FAILED
            },
            Ordering::Release,
        );
        result
    }

    async fn migrate_inner(&self) -> Result<(), DatabaseError> {
        match self.schema_status().await? {
            SchemaStatus::TooNew { found, supported } => {
                return Err(DatabaseError::SchemaTooNew { found, supported });
            }
            SchemaStatus::Current | SchemaStatus::MigrationRequired { .. } => {}
        }
        match &self.pool {
            DatabasePool::PostgreSql(pool) => {
                timeout(self.query_timeout, POSTGRES_MIGRATOR.run(pool))
                    .await
                    .map_err(|_| DatabaseError::MigrationFailed)?
                    .map_err(|_| DatabaseError::MigrationFailed)?
            }
            DatabasePool::Sqlite(pool) => timeout(self.query_timeout, SQLITE_MIGRATOR.run(pool))
                .await
                .map_err(|_| DatabaseError::MigrationFailed)?
                .map_err(|_| DatabaseError::MigrationFailed)?,
        }
        match self.schema_status().await? {
            SchemaStatus::Current => Ok(()),
            SchemaStatus::TooNew { found, supported } => {
                Err(DatabaseError::SchemaTooNew { found, supported })
            }
            SchemaStatus::MigrationRequired { .. } => Err(DatabaseError::MigrationFailed),
        }
    }

    pub async fn require_current_schema(&self) -> Result<(), DatabaseError> {
        let result = match self.schema_status().await? {
            SchemaStatus::Current => match &self.pool {
                DatabasePool::PostgreSql(pool) => POSTGRES_MIGRATOR
                    .run(pool)
                    .await
                    .map_err(|_| DatabaseError::MigrationFailed),
                DatabasePool::Sqlite(pool) => SQLITE_MIGRATOR
                    .run(pool)
                    .await
                    .map_err(|_| DatabaseError::MigrationFailed),
            },
            SchemaStatus::MigrationRequired { found } => {
                Err(DatabaseError::MigrationRequired { found })
            }
            SchemaStatus::TooNew { found, supported } => {
                Err(DatabaseError::SchemaTooNew { found, supported })
            }
        };
        self.state.store(
            if result.is_ok() {
                STATE_READY
            } else {
                STATE_FAILED
            },
            Ordering::Release,
        );
        result
    }

    pub async fn schema_status(&self) -> Result<SchemaStatus, DatabaseError> {
        let version = match &self.pool {
            DatabasePool::PostgreSql(pool) => {
                let table_exists = timeout(
                    self.query_timeout,
                    sqlx::query("SELECT to_regclass('_sqlx_migrations')::text AS table_name")
                        .fetch_one(pool),
                )
                .await
                .map_err(|_| DatabaseError::Unavailable)?
                .map_err(|_| DatabaseError::Unavailable)?
                .try_get::<Option<String>, _>("table_name")
                .map_err(|_| DatabaseError::UnknownInfrastructure)?
                .is_some();
                if !table_exists {
                    0
                } else {
                    timeout(
                        self.query_timeout,
                        sqlx::query(
                            "SELECT COALESCE(MAX(version), 0) AS version FROM _sqlx_migrations",
                        )
                        .fetch_one(pool),
                    )
                    .await
                    .map_err(|_| DatabaseError::Unavailable)?
                    .map_err(|_| DatabaseError::Unavailable)?
                    .try_get::<i64, _>("version")
                    .map_err(|_| DatabaseError::UnknownInfrastructure)?
                }
            }
            DatabasePool::Sqlite(pool) => {
                let table_exists = timeout(
                    self.query_timeout,
                    sqlx::query(
                        "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
                    )
                    .fetch_optional(pool),
                )
                .await
                .map_err(|_| DatabaseError::Unavailable)?
                .map_err(|_| DatabaseError::Unavailable)?
                .is_some();
                if !table_exists {
                    0
                } else {
                    timeout(
                        self.query_timeout,
                        sqlx::query(
                            "SELECT COALESCE(MAX(version), 0) AS version FROM _sqlx_migrations",
                        )
                        .fetch_one(pool),
                    )
                    .await
                    .map_err(|_| DatabaseError::Unavailable)?
                    .map_err(|_| DatabaseError::Unavailable)?
                    .try_get::<i64, _>("version")
                    .map_err(|_| DatabaseError::UnknownInfrastructure)?
                }
            }
        };

        Ok(if version > LATEST_SCHEMA_VERSION {
            SchemaStatus::TooNew {
                found: version,
                supported: LATEST_SCHEMA_VERSION,
            }
        } else if version < LATEST_SCHEMA_VERSION {
            SchemaStatus::MigrationRequired { found: version }
        } else {
            SchemaStatus::Current
        })
    }

    pub async fn readiness(&self) -> Result<(), ReadinessError> {
        match self.state.load(Ordering::Acquire) {
            STATE_MIGRATING => return Err(ReadinessError::MigrationInProgress),
            STATE_READY => {}
            STATE_NOT_READY | STATE_FAILED => return Err(ReadinessError::SchemaNotReady),
            _ => return Err(ReadinessError::SchemaNotReady),
        }
        let healthy = match &self.pool {
            DatabasePool::PostgreSql(pool) => {
                timeout(self.query_timeout, sqlx::query("SELECT 1").execute(pool))
                    .await
                    .is_ok_and(|result| result.is_ok())
            }
            DatabasePool::Sqlite(pool) => {
                timeout(self.query_timeout, sqlx::query("SELECT 1").execute(pool))
                    .await
                    .is_ok_and(|result| result.is_ok())
            }
        };
        if !healthy {
            return Err(ReadinessError::DatabaseUnavailable);
        }
        match self.schema_status().await {
            Ok(SchemaStatus::Current) => Ok(()),
            Ok(SchemaStatus::MigrationRequired { .. } | SchemaStatus::TooNew { .. }) => {
                Err(ReadinessError::SchemaNotReady)
            }
            Err(_) => Err(ReadinessError::DatabaseUnavailable),
        }
    }

    pub async fn close(&self) -> Result<(), DatabaseError> {
        self.state.store(STATE_FAILED, Ordering::Release);
        let close = async {
            match &self.pool {
                DatabasePool::PostgreSql(pool) => pool.close().await,
                DatabasePool::Sqlite(pool) => pool.close().await,
            }
        };
        timeout(self.shutdown_timeout, close)
            .await
            .map_err(|_| DatabaseError::ShutdownTimedOut)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaStatus {
    Current,
    MigrationRequired { found: i64 },
    TooNew { found: i64, supported: i64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadinessError {
    DatabaseUnavailable,
    MigrationInProgress,
    SchemaNotReady,
}

impl ReadinessError {
    #[must_use]
    pub const fn event_code(self) -> &'static str {
        match self {
            Self::DatabaseUnavailable => "database_unavailable",
            Self::MigrationInProgress => "database_migration_in_progress",
            Self::SchemaNotReady => "database_schema_not_ready",
        }
    }
}

impl fmt::Display for ReadinessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.event_code())
    }
}

impl Error for ReadinessError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseError {
    InvalidConfiguration,
    Unavailable,
    MigrationRequired { found: i64 },
    SchemaTooNew { found: i64, supported: i64 },
    MigrationFailed,
    ShutdownTimedOut,
    UnknownInfrastructure,
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration => formatter.write_str("database configuration is invalid"),
            Self::Unavailable => formatter.write_str("database is unavailable"),
            Self::MigrationRequired { found } => {
                write!(
                    formatter,
                    "database migration is required from schema version {found}"
                )
            }
            Self::SchemaTooNew { found, supported } => write!(
                formatter,
                "database schema version {found} is newer than supported version {supported}"
            ),
            Self::MigrationFailed => formatter.write_str("database migration failed"),
            Self::ShutdownTimedOut => formatter.write_str("database pool shutdown timed out"),
            Self::UnknownInfrastructure => {
                formatter.write_str("unknown database infrastructure error")
            }
        }
    }
}

impl Error for DatabaseError {}

#[cfg(unix)]
fn harden_sqlite_permissions(
    directory: &std::path::Path,
    database_file: &std::path::Path,
) -> Result<(), DatabaseError> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
        .map_err(|_| DatabaseError::Unavailable)?;
    fs::set_permissions(database_file, fs::Permissions::from_mode(0o600))
        .map_err(|_| DatabaseError::Unavailable)
}

#[cfg(not(unix))]
fn harden_sqlite_permissions(
    _directory: &std::path::Path,
    _database_file: &std::path::Path,
) -> Result<(), DatabaseError> {
    Ok(())
}
