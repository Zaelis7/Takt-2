#![forbid(unsafe_code)]

use std::{
    error::Error,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use serde_json::Value;
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};

const PASSWORD: &str = "correct horse battery";

fn run_server(
    data_directory: &Path,
    arguments: &[&str],
    stdin: Option<&str>,
) -> Result<Output, Box<dyn Error>> {
    run_server_bytes(data_directory, arguments, stdin.map(str::as_bytes))
}

fn run_server_bytes(
    data_directory: &Path,
    arguments: &[&str],
    stdin: Option<&[u8]>,
) -> Result<Output, Box<dyn Error>> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_takt-server"));
    command
        .args(arguments)
        .env("TAKT_PROFILE", "local")
        .env("TAKT_DATABASE_ENGINE", "sqlite")
        .env("TAKT_DATA_DIR", data_directory)
        .env_remove("TAKT_DATABASE_URL")
        .env_remove("TAKT_DATABASE_URL_FILE")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    if let (Some(value), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(value)?;
    }
    Ok(child.wait_with_output()?)
}

// PRD-IAM-001: the scriptable CLI emits only JSON on stdout and never emits
// the password or its hash.
#[test]
fn bootstrap_cli_is_json_idempotent_and_uses_stable_exit_codes() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let arguments = [
        "admin",
        "bootstrap",
        "--username",
        "Admin",
        "--password-stdin",
        "--output",
        "json",
    ];
    let first = run_server(directory.path(), &arguments, Some(&format!("{PASSWORD}\n")))?;
    assert_eq!(first.status.code(), Some(0));
    assert!(first.stderr.is_empty());
    let first_json: Value = serde_json::from_slice(&first.stdout)?;
    assert_eq!(first_json["status"], "created");
    assert_eq!(first_json["organization"]["slug"], "default");
    assert_eq!(first_json["project"]["slug"], "default");
    assert_eq!(first_json["membership"]["role"], "owner");
    let first_text = String::from_utf8(first.stdout)?;
    assert!(!first_text.contains(PASSWORD));
    assert!(!first_text.contains("argon2"));

    let repeated = run_server(directory.path(), &arguments, Some(&format!("{PASSWORD}\n")))?;
    assert_eq!(repeated.status.code(), Some(0));
    let repeated_json: Value = serde_json::from_slice(&repeated.stdout)?;
    assert_eq!(repeated_json["status"], "already_present");
    assert_eq!(repeated_json["user"]["id"], first_json["user"]["id"]);

    let conflict = run_server(
        directory.path(),
        &arguments,
        Some("different safe password\n"),
    )?;
    assert_eq!(conflict.status.code(), Some(5));
    let diagnostics = String::from_utf8(conflict.stderr)?;
    assert!(!diagnostics.contains("different safe password"));
    assert!(!diagnostics.contains("argon2"));
    Ok(())
}

#[test]
fn bootstrap_cli_rejects_invalid_password_and_password_arguments() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let short = run_server(
        directory.path(),
        &[
            "admin",
            "bootstrap",
            "--username",
            "admin",
            "--password-stdin",
        ],
        Some("short\n"),
    )?;
    assert_eq!(short.status.code(), Some(3));
    assert!(!String::from_utf8(short.stderr)?.contains("short"));

    let unsupported = run_server(
        directory.path(),
        &[
            "admin",
            "bootstrap",
            "--username",
            "admin",
            "--password",
            "not-a-secret-fixture",
        ],
        None,
    )?;
    assert_eq!(unsupported.status.code(), Some(3));
    let diagnostics = String::from_utf8(unsupported.stderr)?;
    assert!(diagnostics.contains("--password"));
    assert!(!diagnostics.contains("not-a-secret-fixture"));

    let mut trailing_input = vec![b'a'; 1_024];
    trailing_input.extend_from_slice(b"\r\nEXTRA");
    let trailing_directory = tempfile::tempdir()?;
    let trailing = run_server_bytes(
        trailing_directory.path(),
        &[
            "admin",
            "bootstrap",
            "--username",
            "admin",
            "--password-stdin",
        ],
        Some(&trailing_input),
    )?;
    assert_eq!(trailing.status.code(), Some(3));
    assert!(trailing.stdout.is_empty());
    Ok(())
}

#[test]
fn sqlite_configuration_refuses_the_working_directory() -> Result<(), Box<dyn Error>> {
    let working_directory = std::env::current_dir()?;
    let database_path = working_directory.join("takt.sqlite3");
    let existed_before = database_path.exists();
    let output = Command::new(env!("CARGO_BIN_EXE_takt-server"))
        .arg("--migrate-only")
        .env("TAKT_PROFILE", "local")
        .env("TAKT_DATABASE_ENGINE", "sqlite")
        .env("TAKT_DATA_DIR", &working_directory)
        .output()?;
    assert_eq!(output.status.code(), Some(10));
    assert!(String::from_utf8(output.stderr)?.contains("must not be"));
    assert_eq!(database_path.exists(), existed_before);
    Ok(())
}

#[test]
fn sqlite_configuration_resolves_parent_components_before_safety_checks()
-> Result<(), Box<dyn Error>> {
    let current_directory = std::env::current_dir()?;
    let repository = current_directory
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .ok_or("test must run below a repository root")?;
    let repository_parent = repository.parent().ok_or("repository must have a parent")?;
    let repository_name = repository
        .file_name()
        .ok_or("repository must have a directory name")?;
    let protected_directory = tempfile::tempdir_in(repository.join("target"))?;
    let sibling_directory = tempfile::tempdir_in(repository_parent)?;
    let disguised_directory = sibling_directory
        .path()
        .join("..")
        .join(repository_name)
        .join("target")
        .join(
            protected_directory
                .path()
                .file_name()
                .ok_or("temporary directory must have a name")?,
        );

    let output = run_server(&disguised_directory, &["--migrate-only"], None)?;
    assert_eq!(output.status.code(), Some(10));
    assert!(String::from_utf8(output.stderr)?.contains("must not be"));
    assert!(!protected_directory.path().join("takt.sqlite3").exists());
    Ok(())
}

#[tokio::test]
async fn migrate_only_and_no_auto_migrate_have_explicit_behavior() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let refused = run_server(directory.path(), &["--no-auto-migrate"], None)?;
    assert_eq!(refused.status.code(), Some(10));
    assert!(String::from_utf8(refused.stderr)?.contains("migration is required"));

    let migrated = run_server(directory.path(), &["--migrate-only"], None)?;
    assert_eq!(migrated.status.code(), Some(0));
    assert!(migrated.stdout.is_empty());
    let database_path = directory.path().join("takt.sqlite3");
    assert!(database_path.is_file());
    assert!(!Path::new("takt.sqlite3").exists());

    let mut connection =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(database_path))
            .await?;
    let migration_count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM _sqlx_migrations")
        .fetch_one(&mut connection)
        .await?
        .try_get("count")?;
    assert_eq!(migration_count, 7);
    sqlx::query("UPDATE _sqlx_migrations SET version = 8 WHERE version = 7")
        .execute(&mut connection)
        .await?;
    connection.close().await?;
    let newer = run_server(directory.path(), &["--migrate-only"], None)?;
    assert_eq!(newer.status.code(), Some(10));
    assert!(String::from_utf8(newer.stderr)?.contains("newer than supported"));
    Ok(())
}
