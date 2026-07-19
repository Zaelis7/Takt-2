#![forbid(unsafe_code)]

use std::{
    future::{Future, IntoFuture},
    io::{self, Read},
    net::SocketAddr,
    process::ExitCode,
    sync::Arc,
};

use async_trait::async_trait;
use clap::{Parser, Subcommand, ValueEnum};
use takt_api::{HealthMetrics, ReadinessCheck, ReadinessFailure};
use takt_application::{
    ApplicationError, Argon2idConfig, BootstrapOutput, BootstrapService, BootstrapStatus,
    PasswordHash, PasswordHasher, PasswordHashing, RepositoryError, SystemClock, UuidV7Generator,
    ValidationError,
};
use takt_persistence::{
    ConfigError, Database, DatabaseConfig, DatabaseError, ReadinessError, RuntimeProfile,
    SqlxRepository,
};
use tokio::net::TcpListener;
use zeroize::Zeroizing;

const EXIT_SUCCESS: u8 = 0;
const EXIT_VALIDATION: u8 = 3;
const EXIT_CONFLICT: u8 = 5;
const EXIT_INFRASTRUCTURE: u8 = 10;

#[derive(Parser)]
#[command(
    name = "takt-server",
    version,
    about = "Takt server and local administration"
)]
struct Cli {
    /// Apply pending migrations and exit without serving.
    #[arg(long, conflicts_with = "no_auto_migrate")]
    migrate_only: bool,
    /// Refuse to apply pending migrations automatically.
    #[arg(long)]
    no_auto_migrate: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Local administrative operations.
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
}

#[derive(Subcommand)]
enum AdminCommand {
    /// Atomically create the default organization/project and first local owner.
    Bootstrap(BootstrapArgs),
}

#[derive(clap::Args)]
struct BootstrapArgs {
    /// Local username (normalized to lowercase ASCII).
    #[arg(long)]
    username: String,
    /// Read the password from standard input. No password argument is supported.
    #[arg(long, required = true)]
    password_stdin: bool,
    /// Select human-readable or machine-readable stdout.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    output: OutputFormat,
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

struct DatabaseReadiness {
    database: Database,
}

#[derive(Clone, Copy)]
struct TokioPasswordHasher {
    config: Argon2idConfig,
}

#[async_trait]
impl PasswordHashing for TokioPasswordHasher {
    async fn hash(&self, password: &str) -> Result<PasswordHash, ValidationError> {
        let password = Zeroizing::new(password.to_owned());
        let config = self.config;
        tokio::task::spawn_blocking(move || PasswordHasher::new(config).hash(password.as_str()))
            .await
            .map_err(|_| ValidationError::PasswordHashFailed)?
    }

    async fn verify(&self, password: &str, hash: &PasswordHash) -> Result<bool, ValidationError> {
        let password = Zeroizing::new(password.to_owned());
        let hash = hash.clone();
        tokio::task::spawn_blocking(move || {
            PasswordHasher::new(Argon2idConfig::production()).verify(password.as_str(), &hash)
        })
        .await
        .map_err(|_| ValidationError::PasswordHashFailed)?
    }
}

#[async_trait]
impl ReadinessCheck for DatabaseReadiness {
    async fn check(&self) -> Result<(), ReadinessFailure> {
        self.database
            .readiness()
            .await
            .map_err(|error| match error {
                ReadinessError::DatabaseUnavailable => ReadinessFailure::DatabaseUnavailable,
                ReadinessError::MigrationInProgress => ReadinessFailure::MigrationInProgress,
                ReadinessError::SchemaNotReady => ReadinessFailure::SchemaNotReady,
            })
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    initialize_logging();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = if error.exit_code() == 0 {
                EXIT_SUCCESS
            } else {
                EXIT_VALIDATION
            };
            let _ = error.print();
            return ExitCode::from(exit_code);
        }
    };
    match run(cli).await {
        Ok(()) => ExitCode::from(EXIT_SUCCESS),
        Err(failure) => {
            eprintln!("{}", failure.message());
            ExitCode::from(failure.exit_code())
        }
    }
}

async fn run(cli: Cli) -> Result<(), CliFailure> {
    if cli.migrate_only && cli.command.is_some() {
        return Err(CliFailure::Validation(
            "--migrate-only cannot be combined with a command",
        ));
    }
    let config = DatabaseConfig::from_environment().map_err(CliFailure::Configuration)?;
    let should_auto_migrate =
        !cli.no_auto_migrate && (cli.migrate_only || config.profile() == RuntimeProfile::Local);
    let database = Database::connect(&config)
        .await
        .map_err(CliFailure::Database)?;

    if !cli.migrate_only && cli.command.is_none() {
        return serve(database, should_auto_migrate).await;
    }

    let schema_result = if should_auto_migrate {
        database.migrate().await
    } else {
        database.require_current_schema().await
    };
    if let Err(error) = schema_result {
        if let Err(close_error) = database.close().await {
            tracing::warn!(
                event_code = "database_shutdown_failed",
                reason = %close_error,
                "database pool did not close cleanly after schema failure"
            );
        }
        return Err(CliFailure::Database(error));
    }
    if cli.migrate_only {
        database.close().await.map_err(CliFailure::Database)?;
        return Ok(());
    }

    match cli.command {
        Some(Command::Admin { command }) => {
            let result = match command {
                AdminCommand::Bootstrap(arguments) => {
                    run_admin_bootstrap(&database, arguments).await
                }
            };
            let close_result = database.close().await.map_err(CliFailure::Database);
            result?;
            close_result
        }
        None => Err(CliFailure::Validation("server mode was already dispatched")),
    }
}

async fn run_admin_bootstrap(
    database: &Database,
    arguments: BootstrapArgs,
) -> Result<(), CliFailure> {
    if !arguments.password_stdin {
        return Err(CliFailure::Validation("--password-stdin is required"));
    }
    let password = tokio::task::spawn_blocking(read_password_from_stdin)
        .await
        .map_err(|_| CliFailure::Input)?
        .map_err(|_| CliFailure::Input)?;
    let repository = SqlxRepository::new(database.clone());
    let hasher = TokioPasswordHasher {
        config: Argon2idConfig::production(),
    };
    let clock = SystemClock;
    let ids = UuidV7Generator;
    let service = BootstrapService::new(&repository, &hasher, &clock, &ids);
    let output = service
        .execute(&arguments.username, password.as_str())
        .await
        .map_err(CliFailure::Application)?;
    write_bootstrap_output(&output, arguments.output)?;
    Ok(())
}

fn read_password_from_stdin() -> Result<Zeroizing<String>, io::Error> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(1_027));
    io::stdin().take(1_027).read_to_end(&mut bytes)?;
    if bytes.len() > 1_026 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "password input exceeds the maximum framed length",
        ));
    }
    if bytes.ends_with(b"\n") {
        bytes.pop();
        if bytes.ends_with(b"\r") {
            bytes.pop();
        }
    }
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "password input must be UTF-8"))?;
    Ok(Zeroizing::new(value.to_owned()))
}

fn write_bootstrap_output(
    output: &BootstrapOutput,
    format: OutputFormat,
) -> Result<(), CliFailure> {
    match format {
        OutputFormat::Human => {
            println!(
                "status={} organization_id={} organization_slug={} project_id={} project_slug={} user_id={} username={} membership_id={} role={} audit_event_id={} operation_id={}",
                status_name(output.status),
                output.resources.organization.id,
                output.resources.organization.slug,
                output.resources.project.id,
                output.resources.project.slug,
                output.resources.user.id,
                output.resources.user.normalized_username,
                output.resources.membership.id,
                output.resources.membership.role.as_str(),
                output.resources.audit_event.id,
                output.resources.audit_event.request_id,
            );
        }
        OutputFormat::Json => {
            let document = serde_json::json!({
                "status": status_name(output.status),
                "organization": {
                    "id": output.resources.organization.id.to_string(),
                    "slug": output.resources.organization.slug,
                },
                "project": {
                    "id": output.resources.project.id.to_string(),
                    "slug": output.resources.project.slug,
                },
                "user": {
                    "id": output.resources.user.id.to_string(),
                    "username": output.resources.user.normalized_username,
                },
                "membership": {
                    "id": output.resources.membership.id.to_string(),
                    "role": output.resources.membership.role.as_str(),
                },
                "audit_event_id": output.resources.audit_event.id.to_string(),
                "operation_id": output.resources.audit_event.request_id.to_string(),
            });
            println!(
                "{}",
                serde_json::to_string(&document).map_err(|_| CliFailure::Output)?
            );
        }
    }
    Ok(())
}

const fn status_name(status: BootstrapStatus) -> &'static str {
    match status {
        BootstrapStatus::Created => "created",
        BootstrapStatus::AlreadyPresent => "already_present",
    }
}

async fn serve(database: Database, should_auto_migrate: bool) -> Result<(), CliFailure> {
    let address = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = TcpListener::bind(address)
        .await
        .map_err(|_| CliFailure::Network)?;
    let metrics = Arc::new(HealthMetrics::default());
    let readiness = Arc::new(DatabaseReadiness {
        database: database.clone(),
    });
    tracing::info!(
        event_code = "server_started",
        database_engine = database.engine().as_str(),
        listen_address = %address,
        "Takt server is listening"
    );
    let application = takt_api::router_with_readiness(readiness, metrics);
    let initialize_schema = async {
        if should_auto_migrate {
            database.migrate().await
        } else {
            database.require_current_schema().await
        }
    };
    let serve_result =
        serve_while_initializing(listener, application, initialize_schema, shutdown_signal())
            .await
            .map_err(|error| match error {
                ServeWhileInitializingError::Initialization(error) => CliFailure::Database(error),
                ServeWhileInitializingError::Network => CliFailure::Network,
            });
    let close_result = database.close().await.map_err(CliFailure::Database);
    serve_result?;
    close_result
}

enum ServeWhileInitializingError<E> {
    Initialization(E),
    Network,
}

async fn serve_while_initializing<F, S, E>(
    listener: TcpListener,
    application: axum::Router,
    initialize: F,
    shutdown: S,
) -> Result<(), ServeWhileInitializingError<E>>
where
    F: Future<Output = Result<(), E>>,
    S: Future<Output = ()> + Send + 'static,
{
    let server = axum::serve(listener, application)
        .with_graceful_shutdown(shutdown)
        .into_future();
    tokio::pin!(server);
    tokio::pin!(initialize);

    tokio::select! {
        result = &mut server => result.map_err(|_| ServeWhileInitializingError::Network),
        initialization = &mut initialize => match initialization {
            Ok(()) => server.await.map_err(|_| ServeWhileInitializingError::Network),
            Err(error) => Err(ServeWhileInitializingError::Initialization(error)),
        },
    }
}

async fn shutdown_signal() {
    let _signal_result = tokio::signal::ctrl_c().await;
}

fn initialize_logging() {
    let _subscriber_result = tracing_subscriber::fmt()
        .json()
        .with_writer(io::stderr)
        .try_init();
}

enum CliFailure {
    Validation(&'static str),
    Configuration(ConfigError),
    Database(DatabaseError),
    Application(ApplicationError),
    Input,
    Output,
    Network,
}

impl CliFailure {
    const fn exit_code(&self) -> u8 {
        match self {
            Self::Validation(_) | Self::Input => EXIT_VALIDATION,
            Self::Application(ApplicationError::Validation(_)) => EXIT_VALIDATION,
            Self::Application(ApplicationError::Conflict) => EXIT_CONFLICT,
            Self::Application(ApplicationError::Repository(RepositoryError::AlreadyExists)) => {
                EXIT_CONFLICT
            }
            Self::Configuration(_)
            | Self::Database(_)
            | Self::Application(_)
            | Self::Output
            | Self::Network => EXIT_INFRASTRUCTURE,
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Validation(message) => (*message).to_owned(),
            Self::Configuration(error) => error.to_string(),
            Self::Database(error) => error.to_string(),
            Self::Application(error) => error.to_string(),
            Self::Input => "failed to read password from standard input".to_owned(),
            Self::Output => "failed to serialize command output".to_owned(),
            Self::Network => "server network operation failed".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use async_trait::async_trait;
    use takt_api::{HealthMetrics, ReadinessCheck, ReadinessFailure};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::{Notify, oneshot},
    };

    use super::serve_while_initializing;

    struct GatedReadiness(Arc<AtomicBool>);

    #[async_trait]
    impl ReadinessCheck for GatedReadiness {
        async fn check(&self) -> Result<(), ReadinessFailure> {
            if self.0.load(Ordering::Acquire) {
                Ok(())
            } else {
                Err(ReadinessFailure::MigrationInProgress)
            }
        }
    }

    async fn http_get(address: std::net::SocketAddr, path: &str) -> Result<String, io::Error> {
        let mut stream = TcpStream::connect(address).await?;
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await?;
        let mut response = String::new();
        stream.read_to_string(&mut response).await?;
        Ok(response)
    }

    // PRD-API-002 / PRD-DATA-002: the production startup composition serves
    // readiness while initialization is still in progress.
    #[tokio::test]
    async fn readiness_is_served_during_schema_initialization()
    -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let ready = Arc::new(AtomicBool::new(false));
        let gate = Arc::new(Notify::new());
        let application = takt_api::router_with_readiness(
            Arc::new(GatedReadiness(ready.clone())),
            Arc::new(HealthMetrics::default()),
        );
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let initialization_gate = gate.clone();
        let initialization_ready = ready.clone();

        let server = tokio::spawn(async move {
            serve_while_initializing(
                listener,
                application,
                async move {
                    initialization_gate.notified().await;
                    initialization_ready.store(true, Ordering::Release);
                    Ok::<(), io::Error>(())
                },
                async move {
                    let _ = shutdown_receiver.await;
                },
            )
            .await
        });

        let migrating = http_get(address, "/health/ready").await?;
        assert!(migrating.contains(" 503 "), "response was: {migrating:?}");
        assert!(migrating.contains("service_unavailable"));

        gate.notify_one();
        while !ready.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        let available = http_get(address, "/health/ready").await?;
        assert!(available.starts_with("HTTP/1.1 200"));

        let _ = shutdown_sender.send(());
        assert!(server.await?.is_ok());
        Ok(())
    }
}
