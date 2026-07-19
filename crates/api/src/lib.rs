#![forbid(unsafe_code)]

use axum::{
    Json, Router,
    body::Body,
    extract::{OriginalUri, Request},
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

#[derive(Serialize)]
struct Health {
    status: HealthStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum HealthStatus {
    Ok,
}

/// Builds the complete HTTP router for the bootstrap server.
///
/// Readiness is healthy once this router is serving because this milestone has
/// no database, worker, key, or other external dependency to evaluate.
pub fn router() -> Router {
    Router::new()
        .route("/health/live", get(health))
        .route("/health/ready", get(health))
        .fallback(serve_web_asset)
        .layer(middleware::from_fn(attach_request_id))
}

async fn health() -> Json<Health> {
    Json(Health {
        status: HealthStatus::Ok,
    })
}

async fn attach_request_id(request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .filter(|value| value.get_version() == Some(Version::SortRand))
        .unwrap_or_else(Uuid::now_v7)
        .to_string();

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
