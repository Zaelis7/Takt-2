use std::{
    collections::HashSet,
    error::Error,
    fmt,
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{ConnectInfo, OriginalUri, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::{Uuid, Version};
use zeroize::Zeroizing;

use super::{ApiState, RequestId};

const SESSION_COOKIE: &str = "takt_session";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenHttpCredentialKind {
    Bearer,
    Session,
}

pub struct ApiTokenHttpCredential {
    kind: ApiTokenHttpCredentialKind,
    value: Zeroizing<String>,
}

impl ApiTokenHttpCredential {
    #[must_use]
    pub const fn kind(&self) -> ApiTokenHttpCredentialKind {
        self.kind
    }

    #[must_use]
    pub fn expose_to_port(&self) -> &str {
        self.value.as_str()
    }
}

impl fmt::Debug for ApiTokenHttpCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ApiTokenHttpCredential({:?}, [REDACTED])",
            self.kind
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenHttpKind {
    Personal,
    Service,
}

impl ApiTokenHttpKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Service => "service",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenHttpStatus {
    Active,
    Revoked,
    Expired,
}

impl ApiTokenHttpStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiTokenHttpResource {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Option<Uuid>,
    pub name: String,
    pub kind: ApiTokenHttpKind,
    pub token_prefix: String,
    pub scopes: Vec<String>,
    pub ip_networks: Vec<String>,
    pub status: ApiTokenHttpStatus,
    pub expires_at_unix_micros: Option<i64>,
    pub last_used_at_unix_micros: Option<i64>,
    pub revoked_at_unix_micros: Option<i64>,
    pub created_at_unix_micros: i64,
    pub updated_at_unix_micros: i64,
    pub version: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTokenReadHttpError {
    AuthenticationFailed,
    PermissionDenied,
    NotFound,
    Internal,
}

impl fmt::Display for ApiTokenReadHttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("API token read request failed")
    }
}

impl Error for ApiTokenReadHttpError {}

#[async_trait]
pub trait ApiTokenReadHttpPort: Send + Sync {
    async fn get(
        &self,
        credential: ApiTokenHttpCredential,
        source: IpAddr,
        id: Uuid,
        request_id: &str,
    ) -> Result<ApiTokenHttpResource, ApiTokenReadHttpError>;
}

pub(crate) fn routes() -> Router<ApiState> {
    Router::new().route("/api/v1/api-tokens/{api_token_id}", get(get_api_token))
}

async fn get_api_token(
    State(state): State<ApiState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    axum::Extension(request_id): axum::Extension<RequestId>,
    OriginalUri(uri): OriginalUri,
    Path(api_token_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let credential = match credential(&headers) {
        Ok(value) => value,
        Err(()) => return authentication_problem(uri.path(), &request_id),
    };
    let Some(port) = &state.api_token_reads else {
        return problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            uri.path(),
            &request_id,
        );
    };
    let id = match Uuid::parse_str(&api_token_id) {
        Ok(value) if value.get_version() == Some(Version::SortRand) => value,
        _ => {
            return problem(StatusCode::NOT_FOUND, "not_found", uri.path(), &request_id);
        }
    };
    match port.get(credential, peer.ip(), id, &request_id.0).await {
        Ok(resource) if resource.id == id => resource_response(resource, uri.path(), &request_id),
        Ok(_) => internal_problem(uri.path(), &request_id),
        Err(error) => port_problem(error, uri.path(), &request_id),
    }
}

fn credential(headers: &HeaderMap) -> Result<ApiTokenHttpCredential, ()> {
    let mut authorizations = headers.get_all(header::AUTHORIZATION).iter();
    let authorization = authorizations.next();
    if authorizations.next().is_some() {
        return Err(());
    }
    let bearer = authorization.map(parse_bearer).transpose()?;
    let session = session_cookie(headers)?;
    match (bearer, session) {
        (Some(value), None) => Ok(ApiTokenHttpCredential {
            kind: ApiTokenHttpCredentialKind::Bearer,
            value: Zeroizing::new(value.to_owned()),
        }),
        (None, Some(value)) => Ok(ApiTokenHttpCredential {
            kind: ApiTokenHttpCredentialKind::Session,
            value: Zeroizing::new(value.to_owned()),
        }),
        _ => Err(()),
    }
}

fn parse_bearer(value: &HeaderValue) -> Result<&str, ()> {
    let value = value.to_str().map_err(|_| ())?;
    let (scheme, token) = value.split_once(' ').ok_or(())?;
    if !scheme.eq_ignore_ascii_case("bearer")
        || !(48..=512).contains(&token.len())
        || !token.starts_with("takt_")
        || !token[5..]
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(());
    }
    Ok(token)
}

fn session_cookie(headers: &HeaderMap) -> Result<Option<&str>, ()> {
    let mut found = None;
    for cookie in headers.get_all(header::COOKIE) {
        for pair in cookie.to_str().map_err(|_| ())?.split(';') {
            let Some((name, value)) = pair.trim().split_once('=') else {
                continue;
            };
            if name == SESSION_COOKIE {
                if found.is_some()
                    || !(32..=512).contains(&value.len())
                    || !value.bytes().all(|byte| byte.is_ascii_graphic())
                {
                    return Err(());
                }
                found = Some(value);
            }
        }
    }
    Ok(found)
}

fn valid_scope(value: &str) -> bool {
    let Some((resource, verb)) = value.split_once(':') else {
        return false;
    };
    value.len() <= 100 && valid_scope_part(resource) && valid_scope_part(verb)
}

fn valid_scope_part(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

#[derive(Serialize)]
struct ApiTokenResponse {
    id: String,
    organization_id: String,
    project_id: Option<String>,
    name: String,
    kind: &'static str,
    token_prefix: String,
    scopes: Vec<String>,
    ip_networks: Vec<String>,
    status: &'static str,
    expires_at: Option<String>,
    last_used_at: Option<String>,
    revoked_at: Option<String>,
    created_at: String,
    updated_at: String,
    version: i64,
}

impl TryFrom<ApiTokenHttpResource> for ApiTokenResponse {
    type Error = ();

    fn try_from(value: ApiTokenHttpResource) -> Result<Self, Self::Error> {
        let scopes = value.scopes.iter().collect::<HashSet<_>>();
        let networks = value.ip_networks.iter().collect::<HashSet<_>>();
        if value.id.get_version() != Some(Version::SortRand)
            || value.organization_id.get_version() != Some(Version::SortRand)
            || value
                .project_id
                .is_some_and(|id| id.get_version() != Some(Version::SortRand))
            || !(1..=120).contains(&value.name.chars().count())
            || !valid_prefix(&value.token_prefix)
            || value.scopes.is_empty()
            || value.scopes.len() > 100
            || scopes.len() != value.scopes.len()
            || value.scopes.iter().any(|scope| !valid_scope(scope))
            || value.ip_networks.len() > 32
            || networks.len() != value.ip_networks.len()
            || value
                .ip_networks
                .iter()
                .any(|network| !valid_network(network))
            || value.version < 1
        {
            return Err(());
        }
        Ok(Self {
            id: value.id.to_string(),
            organization_id: value.organization_id.to_string(),
            project_id: value.project_id.map(|id| id.to_string()),
            name: value.name,
            kind: value.kind.as_str(),
            token_prefix: value.token_prefix,
            scopes: value.scopes,
            ip_networks: value.ip_networks,
            status: value.status.as_str(),
            expires_at: optional_time(value.expires_at_unix_micros)?,
            last_used_at: optional_time(value.last_used_at_unix_micros)?,
            revoked_at: optional_time(value.revoked_at_unix_micros)?,
            created_at: format_time(value.created_at_unix_micros)?,
            updated_at: format_time(value.updated_at_unix_micros)?,
            version: value.version,
        })
    }
}

fn valid_prefix(value: &str) -> bool {
    (8..=32).contains(&value.len())
        && value.starts_with("takt_")
        && value[5..]
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_network(value: &str) -> bool {
    if !(3..=64).contains(&value.len()) {
        return false;
    }
    let Some((address, prefix)) = value.split_once('/') else {
        return false;
    };
    let Ok(address) = IpAddr::from_str(address) else {
        return false;
    };
    let Ok(prefix) = prefix.parse::<u8>() else {
        return false;
    };
    if format!("{address}/{prefix}") != value {
        return false;
    }
    match address {
        IpAddr::V4(address) if prefix <= 32 => {
            prefix == 0 || u32::from(address) & (u32::MAX << (32 - prefix)) == u32::from(address)
        }
        IpAddr::V6(address) if prefix <= 128 => {
            prefix == 0
                || u128::from(address) & (u128::MAX << (128 - prefix)) == u128::from(address)
        }
        _ => false,
    }
}

fn optional_time(value: Option<i64>) -> Result<Option<String>, ()> {
    value.map(format_time).transpose()
}

fn format_time(value: i64) -> Result<String, ()> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(value) * 1_000)
        .map_err(|_| ())?
        .format(&Rfc3339)
        .map_err(|_| ())
}

fn resource_response(
    resource: ApiTokenHttpResource,
    path: &str,
    request_id: &RequestId,
) -> Response {
    let version = resource.version;
    let resource = match ApiTokenResponse::try_from(resource) {
        Ok(value) => value,
        Err(()) => return internal_problem(path, request_id),
    };
    let mut response = no_store((StatusCode::OK, Json(resource)).into_response());
    match HeaderValue::from_str(&format!("\"{version}\"")) {
        Ok(value) => {
            response.headers_mut().insert(header::ETAG, value);
            response
        }
        Err(_) => internal_problem(path, request_id),
    }
}

#[derive(Serialize)]
struct Problem {
    r#type: String,
    title: &'static str,
    status: u16,
    code: &'static str,
    detail: &'static str,
    instance: String,
    request_id: String,
}

fn authentication_problem(path: &str, request_id: &RequestId) -> Response {
    problem(
        StatusCode::UNAUTHORIZED,
        "authentication_failed",
        path,
        request_id,
    )
}

fn internal_problem(path: &str, request_id: &RequestId) -> Response {
    tracing::warn!(
        event_code = "api_token_read_failed",
        request_id = %request_id.0,
        "API token read boundary failed"
    );
    problem(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        path,
        request_id,
    )
}

fn port_problem(error: ApiTokenReadHttpError, path: &str, request_id: &RequestId) -> Response {
    match error {
        ApiTokenReadHttpError::AuthenticationFailed => authentication_problem(path, request_id),
        ApiTokenReadHttpError::PermissionDenied => {
            problem(StatusCode::FORBIDDEN, "permission_denied", path, request_id)
        }
        ApiTokenReadHttpError::NotFound => {
            problem(StatusCode::NOT_FOUND, "not_found", path, request_id)
        }
        ApiTokenReadHttpError::Internal => internal_problem(path, request_id),
    }
}

fn problem(status: StatusCode, code: &'static str, path: &str, request_id: &RequestId) -> Response {
    let (title, detail) = match status {
        StatusCode::UNAUTHORIZED => ("Authentication failed", "Authentication failed."),
        StatusCode::FORBIDDEN => ("Permission denied", "Permission denied."),
        StatusCode::NOT_FOUND => ("Not found", "The requested resource was not found."),
        StatusCode::SERVICE_UNAVAILABLE => ("Service unavailable", "The service is not ready."),
        _ => (
            "Internal server error",
            "The request could not be completed.",
        ),
    };
    no_store(
        (
            status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            Json(Problem {
                r#type: format!("https://takt.dev/problems/{code}"),
                title,
                status: status.as_u16(),
                code,
                detail,
                instance: path.to_owned(),
                request_id: request_id.0.clone(),
            }),
        )
            .into_response(),
    )
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
