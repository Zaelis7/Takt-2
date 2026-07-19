use std::{
    env,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseEngine {
    PostgreSql,
    Sqlite,
}

impl DatabaseEngine {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PostgreSql => "postgresql",
            Self::Sqlite => "sqlite",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeProfile {
    Local,
    Production,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolSettings {
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub query_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub sqlite_busy_timeout: Duration,
}

impl PoolSettings {
    fn defaults(engine: DatabaseEngine) -> Self {
        Self {
            max_connections: match engine {
                DatabaseEngine::PostgreSql => 10,
                DatabaseEngine::Sqlite => 5,
            },
            connection_timeout: Duration::from_secs(5),
            query_timeout: Duration::from_secs(5),
            shutdown_timeout: Duration::from_secs(5),
            sqlite_busy_timeout: Duration::from_secs(5),
        }
    }
}

pub(crate) enum ConnectionSettings {
    PostgreSql { url: SecretDatabaseUrl },
    Sqlite { path: PathBuf },
}

pub struct DatabaseConfig {
    profile: RuntimeProfile,
    connection: ConnectionSettings,
    pool: PoolSettings,
}

impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseConfig")
            .field("profile", &self.profile)
            .field("engine", &self.engine())
            .field("connection", &"[REDACTED]")
            .field("pool", &self.pool)
            .finish()
    }
}

impl DatabaseConfig {
    pub fn from_environment() -> Result<Self, ConfigError> {
        let profile = parse_profile(env::var("TAKT_PROFILE").ok().as_deref())?;
        let engine = parse_engine(env::var("TAKT_DATABASE_ENGINE").ok().as_deref(), profile)?;
        if profile == RuntimeProfile::Production && engine != DatabaseEngine::PostgreSql {
            return Err(ConfigError::ProductionRequiresPostgreSql);
        }

        let connection = match engine {
            DatabaseEngine::PostgreSql => ConnectionSettings::PostgreSql {
                url: SecretDatabaseUrl::new(read_database_url()?)?,
            },
            DatabaseEngine::Sqlite => {
                let data_directory = data_directory()?;
                validate_sqlite_location(&data_directory)?;
                ConnectionSettings::Sqlite {
                    path: data_directory.join("takt.sqlite3"),
                }
            }
        };
        let mut pool = PoolSettings::defaults(engine);
        pool.max_connections = parse_u32("TAKT_DB_MAX_CONNECTIONS", pool.max_connections, 1, 100)?;
        pool.connection_timeout = parse_duration(
            "TAKT_DB_CONNECTION_TIMEOUT_MS",
            pool.connection_timeout,
            100,
            120_000,
        )?;
        pool.query_timeout =
            parse_duration("TAKT_DB_QUERY_TIMEOUT_MS", pool.query_timeout, 100, 300_000)?;
        pool.shutdown_timeout = parse_duration(
            "TAKT_DB_SHUTDOWN_TIMEOUT_MS",
            pool.shutdown_timeout,
            100,
            120_000,
        )?;
        pool.sqlite_busy_timeout = parse_duration(
            "TAKT_SQLITE_BUSY_TIMEOUT_MS",
            pool.sqlite_busy_timeout,
            100,
            120_000,
        )?;

        Ok(Self {
            profile,
            connection,
            pool,
        })
    }

    pub fn sqlite_for_test(path: PathBuf) -> Result<Self, ConfigError> {
        if !path.is_absolute() {
            return Err(ConfigError::DataDirectoryMustBeAbsolute);
        }
        Ok(Self {
            profile: RuntimeProfile::Local,
            connection: ConnectionSettings::Sqlite { path },
            pool: PoolSettings {
                max_connections: 5,
                connection_timeout: Duration::from_secs(5),
                query_timeout: Duration::from_secs(5),
                shutdown_timeout: Duration::from_secs(5),
                sqlite_busy_timeout: Duration::from_secs(5),
            },
        })
    }

    pub fn postgresql_for_test(url: String) -> Result<Self, ConfigError> {
        Ok(Self {
            profile: RuntimeProfile::Local,
            connection: ConnectionSettings::PostgreSql {
                url: SecretDatabaseUrl::new(Zeroizing::new(url))?,
            },
            pool: PoolSettings::defaults(DatabaseEngine::PostgreSql),
        })
    }

    #[must_use]
    pub const fn profile(&self) -> RuntimeProfile {
        self.profile
    }

    #[must_use]
    pub const fn engine(&self) -> DatabaseEngine {
        match self.connection {
            ConnectionSettings::PostgreSql { .. } => DatabaseEngine::PostgreSql,
            ConnectionSettings::Sqlite { .. } => DatabaseEngine::Sqlite,
        }
    }

    #[must_use]
    pub const fn pool(&self) -> &PoolSettings {
        &self.pool
    }

    pub fn sqlite_path(&self) -> Option<&Path> {
        match &self.connection {
            ConnectionSettings::Sqlite { path } => Some(path),
            ConnectionSettings::PostgreSql { .. } => None,
        }
    }

    pub(crate) const fn connection(&self) -> &ConnectionSettings {
        &self.connection
    }
}

pub(crate) struct SecretDatabaseUrl(Zeroizing<String>);

impl SecretDatabaseUrl {
    fn new(value: Zeroizing<String>) -> Result<Self, ConfigError> {
        if !(value.starts_with("postgres://") || value.starts_with("postgresql://")) {
            return Err(ConfigError::InvalidDatabaseUrl);
        }
        Ok(Self(value))
    }

    pub(crate) fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for SecretDatabaseUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretDatabaseUrl([REDACTED])")
    }
}

impl Drop for SecretDatabaseUrl {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    InvalidProfile,
    InvalidEngine,
    ProductionRequiresPostgreSql,
    MissingDatabaseUrl,
    ConflictingDatabaseUrlSources,
    InvalidDatabaseUrl,
    SecretSourceUnreadable,
    SecretSourceTooLarge,
    DataDirectoryUnavailable,
    DataDirectoryMustBeAbsolute,
    UnsafeSqliteLocation,
    InvalidNumericSetting,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidProfile => "TAKT_PROFILE must be 'local' or 'production'",
            Self::InvalidEngine => "TAKT_DATABASE_ENGINE must be 'sqlite' or 'postgresql'",
            Self::ProductionRequiresPostgreSql => {
                "the production profile requires explicit PostgreSQL configuration"
            }
            Self::MissingDatabaseUrl => {
                "PostgreSQL requires TAKT_DATABASE_URL or TAKT_DATABASE_URL_FILE"
            }
            Self::ConflictingDatabaseUrlSources => {
                "set only one of TAKT_DATABASE_URL and TAKT_DATABASE_URL_FILE"
            }
            Self::InvalidDatabaseUrl => "the configured database URL is not a PostgreSQL URL",
            Self::SecretSourceUnreadable => "the configured database secret source is unreadable",
            Self::SecretSourceTooLarge => "the configured database secret source is too large",
            Self::DataDirectoryUnavailable => "a platform Takt data directory is unavailable",
            Self::DataDirectoryMustBeAbsolute => "TAKT_DATA_DIR must be an absolute path",
            Self::UnsafeSqliteLocation => {
                "the SQLite data directory must not be the working directory or repository"
            }
            Self::InvalidNumericSetting => {
                "a TAKT database numeric setting is outside its allowed range"
            }
        };
        formatter.write_str(message)
    }
}

impl Error for ConfigError {}

fn parse_profile(value: Option<&str>) -> Result<RuntimeProfile, ConfigError> {
    match value.unwrap_or("local") {
        "local" => Ok(RuntimeProfile::Local),
        "production" => Ok(RuntimeProfile::Production),
        _ => Err(ConfigError::InvalidProfile),
    }
}

fn parse_engine(
    value: Option<&str>,
    profile: RuntimeProfile,
) -> Result<DatabaseEngine, ConfigError> {
    match value {
        Some("sqlite") => Ok(DatabaseEngine::Sqlite),
        Some("postgres") | Some("postgresql") => Ok(DatabaseEngine::PostgreSql),
        Some(_) => Err(ConfigError::InvalidEngine),
        None if profile == RuntimeProfile::Local => Ok(DatabaseEngine::Sqlite),
        None => Err(ConfigError::ProductionRequiresPostgreSql),
    }
}

fn read_database_url() -> Result<Zeroizing<String>, ConfigError> {
    let direct = env::var("TAKT_DATABASE_URL").ok();
    let file = env::var_os("TAKT_DATABASE_URL_FILE");
    match (direct, file) {
        (Some(_), Some(_)) => Err(ConfigError::ConflictingDatabaseUrlSources),
        (Some(value), None) => Ok(Zeroizing::new(value)),
        (None, Some(path)) => {
            let metadata = fs::metadata(&path).map_err(|_| ConfigError::SecretSourceUnreadable)?;
            if metadata.len() > 8_192 {
                return Err(ConfigError::SecretSourceTooLarge);
            }
            let mut value =
                fs::read_to_string(path).map_err(|_| ConfigError::SecretSourceUnreadable)?;
            let new_length = value.trim_end_matches(['\r', '\n']).len();
            value.truncate(new_length);
            Ok(Zeroizing::new(value))
        }
        (None, None) => Err(ConfigError::MissingDatabaseUrl),
    }
}

fn data_directory() -> Result<PathBuf, ConfigError> {
    if let Some(configured) = env::var_os("TAKT_DATA_DIR") {
        let path = PathBuf::from(configured);
        return if path.is_absolute() {
            Ok(path)
        } else {
            Err(ConfigError::DataDirectoryMustBeAbsolute)
        };
    }

    #[cfg(target_os = "windows")]
    let base = env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = env::var_os("HOME")
        .map(PathBuf::from)
        .map(|path| path.join("Library/Application Support"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| {
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join(".local/share"))
    });

    let path = base
        .ok_or(ConfigError::DataDirectoryUnavailable)?
        .join("Takt");
    if !path.is_absolute() {
        return Err(ConfigError::DataDirectoryUnavailable);
    }
    Ok(path)
}

fn validate_sqlite_location(data_directory: &Path) -> Result<(), ConfigError> {
    let current_directory =
        env::current_dir().map_err(|_| ConfigError::DataDirectoryUnavailable)?;
    if data_directory == current_directory {
        return Err(ConfigError::UnsafeSqliteLocation);
    }
    if let Some(repository_root) = current_directory
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        && data_directory.starts_with(repository_root)
    {
        return Err(ConfigError::UnsafeSqliteLocation);
    }
    Ok(())
}

fn parse_u32(name: &str, default: u32, minimum: u32, maximum: u32) -> Result<u32, ConfigError> {
    let Some(value) = env::var(name).ok() else {
        return Ok(default);
    };
    let parsed = value
        .parse::<u32>()
        .map_err(|_| ConfigError::InvalidNumericSetting)?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(ConfigError::InvalidNumericSetting);
    }
    Ok(parsed)
}

fn parse_duration(
    name: &str,
    default: Duration,
    minimum_ms: u32,
    maximum_ms: u32,
) -> Result<Duration, ConfigError> {
    parse_u32(
        name,
        u32::try_from(default.as_millis()).map_err(|_| ConfigError::InvalidNumericSetting)?,
        minimum_ms,
        maximum_ms,
    )
    .map(|milliseconds| Duration::from_millis(u64::from(milliseconds)))
}
