#![forbid(unsafe_code)]

use std::{error::Error, io, sync::Arc};

use async_trait::async_trait;
use axum::Router;
use takt_api::{HealthMetrics, ReadinessCheck, ReadinessFailure};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use uuid::{Uuid, Version};

const PROVIDED_REQUEST_ID: &str = "019b0000-0000-7000-8000-000000000002";
const NON_V7_REQUEST_ID: &str = "550e8400-e29b-41d4-a716-446655440000";

async fn request(path: &str, request_id: Option<&str>) -> Result<String, Box<dyn Error>> {
    request_with_router(takt_api::router(), path, request_id).await
}

async fn request_with_router(
    router: Router,
    path: &str,
    request_id: Option<&str>,
) -> Result<String, Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, router).await });

    let mut stream = TcpStream::connect(address).await?;
    let request_id_header = request_id
        .map(|value| format!("X-Request-Id: {value}\r\n"))
        .unwrap_or_default();
    let raw_request = format!(
        "GET {path} HTTP/1.1\r\nHost: {address}\r\n{request_id_header}Connection: close\r\n\r\n"
    );
    stream.write_all(raw_request.as_bytes()).await?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    server.abort();

    Ok(String::from_utf8(response)?)
}

struct FailedReadiness(ReadinessFailure);

#[async_trait]
impl ReadinessCheck for FailedReadiness {
    async fn check(&self) -> Result<(), ReadinessFailure> {
        Err(self.0)
    }
}

fn assert_health_response(response: &str) -> Result<(), Box<dyn Error>> {
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| io::Error::other("HTTP response has no header/body separator"))?;

    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(
        head.to_ascii_lowercase()
            .contains("content-type: application/json")
    );
    assert!(
        head.to_ascii_lowercase()
            .contains("x-content-type-options: nosniff")
    );
    assert!(
        head.to_ascii_lowercase()
            .contains("content-security-policy: default-src 'self'")
    );
    assert_eq!(body, r#"{"status":"ok"}"#);
    Ok(())
}

fn response_request_id(response: &str) -> Result<Uuid, Box<dyn Error>> {
    let header = response
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("x-request-id: "))
        .ok_or_else(|| io::Error::other("HTTP response has no X-Request-Id header"))?;
    let (_, value) = header
        .split_once(':')
        .ok_or_else(|| io::Error::other("X-Request-Id header is malformed"))?;
    Ok(Uuid::parse_str(value.trim())?)
}

// PRD-NFR-008 / getLiveness: exercise the endpoint through a real TCP listener.
#[tokio::test]
async fn prd_nfr_008_liveness_is_contract_compliant_http() -> Result<(), Box<dyn Error>> {
    let response = request("/health/live", None).await?;

    assert_health_response(&response)?;
    assert!(response.to_ascii_lowercase().contains("x-request-id: "));
    Ok(())
}

// PRD-NFR-008 / getReadiness: a bootstrap server has no external dependencies yet.
#[tokio::test]
async fn prd_nfr_008_readiness_is_contract_compliant_http() -> Result<(), Box<dyn Error>> {
    let response = request("/health/ready", Some(PROVIDED_REQUEST_ID)).await?;

    assert_health_response(&response)?;
    assert!(
        response
            .to_ascii_lowercase()
            .contains(&format!("x-request-id: {PROVIDED_REQUEST_ID}"))
    );
    Ok(())
}

#[tokio::test]
async fn prd_nfr_008_database_failure_only_affects_readiness() -> Result<(), Box<dyn Error>> {
    let metrics = Arc::new(HealthMetrics::default());
    let router = takt_api::router_with_readiness(
        Arc::new(FailedReadiness(ReadinessFailure::DatabaseUnavailable)),
        metrics.clone(),
    );
    let ready = request_with_router(router.clone(), "/health/ready", None).await?;
    assert!(ready.starts_with("HTTP/1.1 503 Service Unavailable\r\n"));
    assert!(ready.contains("application/problem+json"));
    assert!(ready.contains(r#""code":"service_unavailable""#));
    assert!(!ready.contains("postgres://"));
    assert!(!ready.contains("sqlite://"));
    assert_eq!(
        metrics.readiness_failure_count(ReadinessFailure::DatabaseUnavailable),
        1
    );

    let live = request_with_router(router, "/health/live", None).await?;
    assert_health_response(&live)?;
    Ok(())
}

#[tokio::test]
async fn prd_nfr_008_migration_in_progress_is_not_ready() -> Result<(), Box<dyn Error>> {
    let router = takt_api::router_with_readiness(
        Arc::new(FailedReadiness(ReadinessFailure::MigrationInProgress)),
        Arc::new(HealthMetrics::default()),
    );
    let response = request_with_router(router, "/health/ready", None).await?;
    assert!(response.starts_with("HTTP/1.1 503 Service Unavailable\r\n"));
    assert!(!response.contains("database_migration_in_progress"));
    Ok(())
}

#[tokio::test]
async fn prd_api_002_invalid_request_id_is_not_reflected() -> Result<(), Box<dyn Error>> {
    let response = request("/health/live", Some("invalid-request-id")).await?;

    assert!(!response.contains("x-request-id: invalid-request-id"));
    assert!(response.to_ascii_lowercase().contains("x-request-id: "));
    Ok(())
}

#[tokio::test]
async fn prd_api_002_non_v7_request_id_is_replaced() -> Result<(), Box<dyn Error>> {
    let response = request("/health/live", Some(NON_V7_REQUEST_ID)).await?;

    assert!(!response.contains(&format!("x-request-id: {NON_V7_REQUEST_ID}")));
    assert_eq!(
        response_request_id(&response)?.get_version(),
        Some(Version::SortRand),
        "replacement request IDs must use UUIDv7"
    );
    Ok(())
}

#[tokio::test]
async fn bootstrap_web_production_build_is_embedded() -> Result<(), Box<dyn Error>> {
    let response = request("/", None).await?;

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(
        response
            .to_ascii_lowercase()
            .contains("content-type: text/html; charset=utf-8")
    );
    assert!(response.contains("<div id=\"root\"></div>"));
    Ok(())
}

#[tokio::test]
async fn out_of_scope_api_paths_do_not_pretend_to_exist() -> Result<(), Box<dyn Error>> {
    let response = request("/api/v1/system/info", None).await?;

    assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
    assert!(!response.contains("<div id=\"root\"></div>"));
    assert!(response.to_ascii_lowercase().contains("x-request-id: "));

    let reserved_root = request("/health", None).await?;
    assert!(reserved_root.starts_with("HTTP/1.1 404 Not Found\r\n"));
    Ok(())
}
