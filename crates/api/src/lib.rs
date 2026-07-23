#![forbid(unsafe_code)]

mod api_token_cursor;
mod api_tokens;
mod auth;

pub use api_token_cursor::{
    ApiTokenCursorBoundary, ApiTokenCursorError, ApiTokenCursorFilter, ApiTokenCursorKey,
};
pub use api_tokens::{
    ApiTokenHttpCredential, ApiTokenHttpCredentialKind, ApiTokenHttpKind, ApiTokenHttpListPage,
    ApiTokenHttpListQuery, ApiTokenHttpResource, ApiTokenHttpStatus, ApiTokenReadHttpError,
    ApiTokenReadHttpPort,
};
pub use auth::{
    AuthHttpConfig, AuthHttpError, BrowserAuthenticationHttpPort, HttpAuthentication, HttpLogin,
    HttpSecret,
};

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, OriginalUri, Request, State},
    http::{
        HeaderName, HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::{EmbeddedFile, RustEmbed};
use serde::Serialize;
use uuid::{Uuid, Version};

const API_PREFIX: &str = "api/";
const HEALTH_PREFIX: &str = "health/";
const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(RustEmbed)]
#[folder = "../../web/dist/"]
struct WebAssets;

#[derive(Clone)]
pub(crate) struct ApiState {
    readiness: Arc<dyn ReadinessCheck>,
    health_metrics: Arc<HealthMetrics>,
    authentication: Option<Arc<dyn BrowserAuthenticationHttpPort>>,
    api_token_reads: Option<Arc<dyn ApiTokenReadHttpPort>>,
    api_token_cursor_key: Option<ApiTokenCursorKey>,
    auth_config: AuthHttpConfig,
    login_guard: Arc<auth::LoginGuard>,
}

#[derive(Serialize)]
struct Health {
    status: HealthStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum HealthStatus {
    Ok,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadinessFailure {
    DatabaseUnavailable,
    MigrationInProgress,
    SchemaNotReady,
}

impl ReadinessFailure {
    #[must_use]
    pub const fn event_code(self) -> &'static str {
        match self {
            Self::DatabaseUnavailable => "database_unavailable",
            Self::MigrationInProgress => "database_migration_in_progress",
            Self::SchemaNotReady => "database_schema_not_ready",
        }
    }

    const fn metric_index(self) -> usize {
        match self {
            Self::DatabaseUnavailable => 0,
            Self::MigrationInProgress => 1,
            Self::SchemaNotReady => 2,
        }
    }
}

#[async_trait]
pub trait ReadinessCheck: Send + Sync {
    async fn check(&self) -> Result<(), ReadinessFailure>;
}

#[derive(Default)]
pub struct HealthMetrics {
    readiness_failures: [AtomicU64; 3],
}

impl HealthMetrics {
    fn record_readiness_failure(&self, failure: ReadinessFailure) {
        self.readiness_failures[failure.metric_index()].fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn readiness_failure_count(&self, failure: ReadinessFailure) -> u64 {
        self.readiness_failures[failure.metric_index()].load(Ordering::Relaxed)
    }
}

struct AlwaysReady;

#[async_trait]
impl ReadinessCheck for AlwaysReady {
    async fn check(&self) -> Result<(), ReadinessFailure> {
        Ok(())
    }
}

pub fn router() -> Router {
    router_with_readiness(Arc::new(AlwaysReady), Arc::new(HealthMetrics::default()))
}

/// Builds the HTTP router with an injected dependency readiness check.
pub fn router_with_readiness(
    readiness: Arc<dyn ReadinessCheck>,
    health_metrics: Arc<HealthMetrics>,
) -> Router {
    build_router(
        readiness,
        health_metrics,
        None,
        None,
        None,
        AuthHttpConfig::localhost(),
    )
}

pub fn router_with_auth(
    authentication: Arc<dyn BrowserAuthenticationHttpPort>,
    config: AuthHttpConfig,
) -> Router {
    build_router(
        Arc::new(AlwaysReady),
        Arc::new(HealthMetrics::default()),
        Some(authentication),
        None,
        None,
        config,
    )
}

pub fn router_with_api_token_reads(
    api_token_reads: Arc<dyn ApiTokenReadHttpPort>,
    api_token_cursor_key: ApiTokenCursorKey,
) -> Router {
    build_router(
        Arc::new(AlwaysReady),
        Arc::new(HealthMetrics::default()),
        None,
        Some(api_token_reads),
        Some(api_token_cursor_key),
        AuthHttpConfig::localhost(),
    )
}

pub fn router_with_dependencies(
    readiness: Arc<dyn ReadinessCheck>,
    health_metrics: Arc<HealthMetrics>,
    authentication: Arc<dyn BrowserAuthenticationHttpPort>,
    api_token_reads: Arc<dyn ApiTokenReadHttpPort>,
    api_token_cursor_key: ApiTokenCursorKey,
    config: AuthHttpConfig,
) -> Router {
    build_router(
        readiness,
        health_metrics,
        Some(authentication),
        Some(api_token_reads),
        Some(api_token_cursor_key),
        config,
    )
}

fn build_router(
    readiness: Arc<dyn ReadinessCheck>,
    health_metrics: Arc<HealthMetrics>,
    authentication: Option<Arc<dyn BrowserAuthenticationHttpPort>>,
    api_token_reads: Option<Arc<dyn ApiTokenReadHttpPort>>,
    api_token_cursor_key: Option<ApiTokenCursorKey>,
    auth_config: AuthHttpConfig,
) -> Router {
    let login_guard = Arc::new(auth::LoginGuard::new(auth_config));
    Router::new()
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness_handler))
        .merge(auth::routes())
        .merge(api_tokens::routes())
        .fallback(serve_web_asset)
        .with_state(ApiState {
            readiness,
            health_metrics,
            authentication,
            api_token_reads,
            api_token_cursor_key,
            auth_config,
            login_guard,
        })
        .layer(DefaultBodyLimit::max(4 * 1024))
        .layer(middleware::from_fn(attach_request_id))
}

async fn liveness() -> Json<Health> {
    Json(Health {
        status: HealthStatus::Ok,
    })
}

#[derive(Serialize)]
struct ReadinessProblem {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
    detail: &'static str,
    request_id: String,
}

async fn readiness_handler(
    State(state): State<ApiState>,
    axum::Extension(request_id): axum::Extension<RequestId>,
) -> Response {
    match state.readiness.check().await {
        Ok(()) => liveness().await.into_response(),
        Err(failure) => {
            state.health_metrics.record_readiness_failure(failure);
            tracing::warn!(
                event_code = failure.event_code(),
                "readiness dependency check failed"
            );
            (
                StatusCode::SERVICE_UNAVAILABLE,
                [(
                    CONTENT_TYPE,
                    HeaderValue::from_static("application/problem+json"),
                )],
                Json(ReadinessProblem {
                    r#type: "https://takt.dev/problems/service_unavailable",
                    title: "Service unavailable",
                    status: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                    code: "service_unavailable",
                    detail: "The service is not ready.",
                    request_id: request_id.0,
                }),
            )
                .into_response()
        }
    }
}

#[derive(Clone)]
struct RequestId(String);

async fn attach_request_id(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .filter(|value| value.get_version() == Some(Version::SortRand))
        .unwrap_or_else(Uuid::now_v7)
        .to_string();

    request
        .extensions_mut()
        .insert(RequestId(request_id.clone()));
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    response.headers_mut().insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
        ),
    );
    response.headers_mut().insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    response
}

async fn serve_web_asset(OriginalUri(uri): OriginalUri) -> Response {
    let requested_path = uri.path().trim_start_matches('/');
    let asset_path = if requested_path.is_empty() {
        "index.html"
    } else {
        requested_path
    };

    if let Some(asset) = WebAssets::get(asset_path) {
        return embedded_response(asset_path, asset);
    }

    let is_api_path = requested_path == API_PREFIX.trim_end_matches('/')
        || requested_path.starts_with(API_PREFIX)
        || requested_path == HEALTH_PREFIX.trim_end_matches('/')
        || requested_path.starts_with(HEALTH_PREFIX)
        || requested_path.contains('.');
    if !is_api_path && let Some(index) = WebAssets::get("index.html") {
        return embedded_response("index.html", index);
    }

    StatusCode::NOT_FOUND.into_response()
}

fn embedded_response(path: &str, asset: EmbeddedFile) -> Response {
    let content_type = match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    };
    let cache_control = if path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };

    (
        [
            (CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (CACHE_CONTROL, HeaderValue::from_static(cache_control)),
        ],
        Body::from(asset.data.into_owned()),
    )
        .into_response()
}
