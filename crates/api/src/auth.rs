use std::fmt;

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use zeroize::Zeroizing;

use super::{ApiState, RequestId};

const LOGIN_PATH: &str = "/api/v1/auth/login";
const LOGOUT_PATH: &str = "/api/v1/auth/logout";
const SESSION_PATH: &str = "/api/v1/auth/session";
const SESSION_COOKIE: &str = "takt_session";
const CSRF_HEADER: &str = "x-csrf-token";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthHttpError {
    InvalidCredentials,
    Unauthenticated,
    CsrfFailed,
    ValidationFailed,
    Internal,
}

#[derive(Clone, Eq, PartialEq)]
pub struct HttpSecret(Zeroizing<String>);

impl HttpSecret {
    pub fn new(value: String) -> Result<Self, AuthHttpError> {
        if !(32..=512).contains(&value.len()) || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(AuthHttpError::Internal);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    #[must_use]
    pub fn expose_to_client(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for HttpSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HttpSecret([REDACTED])")
    }
}

pub struct HttpAuthentication {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub permissions: Vec<String>,
    pub csrf_token: HttpSecret,
    pub expires_at_unix_micros: i64,
    pub absolute_expires_at_unix_micros: i64,
}

pub struct HttpLogin {
    pub authentication: HttpAuthentication,
    pub session_token: HttpSecret,
}

#[async_trait]
pub trait BrowserAuthenticationHttpPort: Send + Sync {
    async fn login(
        &self,
        username: &str,
        password: &str,
        request_id: &str,
    ) -> Result<HttpLogin, AuthHttpError>;
    async fn current_session(
        &self,
        session_token: &str,
    ) -> Result<HttpAuthentication, AuthHttpError>;
    async fn logout(
        &self,
        session_token: &str,
        csrf_token: &str,
        request_id: &str,
    ) -> Result<(), AuthHttpError>;
}

#[derive(Clone, Copy)]
pub struct AuthHttpConfig {
    pub(crate) secure_cookies: bool,
}

impl AuthHttpConfig {
    #[must_use]
    pub const fn localhost() -> Self {
        Self {
            secure_cookies: false,
        }
    }

    #[must_use]
    pub const fn production() -> Self {
        Self {
            secure_cookies: true,
        }
    }
}

pub(crate) fn routes() -> Router<ApiState> {
    Router::new()
        .route(LOGIN_PATH, post(login))
        .route(LOGOUT_PATH, post(logout))
        .route(SESSION_PATH, get(current_session))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct SessionUser<'a> {
    id: &'a str,
    username: &'a str,
    display_name: &'a str,
}

#[derive(Serialize)]
struct SessionResponse<'a> {
    user: SessionUser<'a>,
    permissions: &'a [String],
    csrf_token: &'a str,
    expires_at: String,
    absolute_expires_at: String,
}

#[derive(Serialize)]
struct Problem {
    r#type: String,
    title: &'static str,
    status: u16,
    code: &'static str,
    detail: &'static str,
    instance: &'static str,
    request_id: String,
}

async fn login(
    State(state): State<ApiState>,
    axum::Extension(request_id): axum::Extension<RequestId>,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Response {
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(_) => {
            return problem(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                LOGIN_PATH,
                &request_id,
            );
        }
    };
    let Some(authentication) = &state.authentication else {
        return problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            LOGIN_PATH,
            &request_id,
        );
    };
    match authentication
        .login(&payload.username, &payload.password, &request_id.0)
        .await
    {
        Ok(login) => login_response(login, state.auth_config.secure_cookies, &request_id),
        Err(error) => authentication_problem(error, LOGIN_PATH, &request_id),
    }
}

async fn current_session(
    State(state): State<ApiState>,
    axum::Extension(request_id): axum::Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let Some(token) = session_cookie(&headers) else {
        return problem(
            StatusCode::UNAUTHORIZED,
            "authentication_failed",
            SESSION_PATH,
            &request_id,
        );
    };
    let Some(authentication) = &state.authentication else {
        return problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            SESSION_PATH,
            &request_id,
        );
    };
    match authentication.current_session(token).await {
        Ok(authentication) => session_response(authentication, &request_id),
        Err(error) => authentication_problem(error, SESSION_PATH, &request_id),
    }
}

async fn logout(
    State(state): State<ApiState>,
    axum::Extension(request_id): axum::Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    let Some(token) = session_cookie(&headers) else {
        return problem(
            StatusCode::UNAUTHORIZED,
            "authentication_failed",
            LOGOUT_PATH,
            &request_id,
        );
    };
    let Some(csrf) = headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return problem(
            StatusCode::FORBIDDEN,
            "csrf_failed",
            LOGOUT_PATH,
            &request_id,
        );
    };
    let Some(authentication) = &state.authentication else {
        return problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            LOGOUT_PATH,
            &request_id,
        );
    };
    match authentication.logout(token, csrf, &request_id.0).await {
        Ok(()) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            let cookie = expired_cookie(state.auth_config.secure_cookies);
            if let Ok(value) = HeaderValue::from_str(&cookie) {
                response.headers_mut().insert(header::SET_COOKIE, value);
            }
            no_store(response)
        }
        Err(error) => authentication_problem(error, LOGOUT_PATH, &request_id),
    }
}

fn login_response(login: HttpLogin, secure: bool, request_id: &RequestId) -> Response {
    let cookie = session_cookie_value(login.session_token.expose_to_client(), secure);
    let mut response = session_response(login.authentication, request_id);
    match HeaderValue::from_str(&cookie) {
        Ok(value) => {
            response.headers_mut().insert(header::SET_COOKIE, value);
            response
        }
        Err(_) => problem(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            LOGIN_PATH,
            request_id,
        ),
    }
}

fn session_response(authentication: HttpAuthentication, request_id: &RequestId) -> Response {
    let response = SessionResponse {
        user: SessionUser {
            id: &authentication.user_id,
            username: &authentication.username,
            display_name: &authentication.display_name,
        },
        permissions: &authentication.permissions,
        csrf_token: authentication.csrf_token.expose_to_client(),
        expires_at: format_time(authentication.expires_at_unix_micros),
        absolute_expires_at: format_time(authentication.absolute_expires_at_unix_micros),
    };
    let _ = request_id;
    no_store((StatusCode::OK, Json(response)).into_response())
}

fn format_time(timestamp_unix_micros: i64) -> String {
    let Ok(value) =
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp_unix_micros) * 1_000)
    else {
        return "1970-01-01T00:00:00Z".to_owned();
    };
    match value.format(&Rfc3339) {
        Ok(formatted) => formatted,
        Err(_) => "1970-01-01T00:00:00Z".to_owned(),
    }
}

fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    let mut found = None;
    for cookie in headers.get_all(header::COOKIE) {
        for pair in cookie.to_str().ok()?.split(';') {
            let Some((name, value)) = pair.trim().split_once('=') else {
                continue;
            };
            if name == SESSION_COOKIE {
                if found.is_some() || value.is_empty() {
                    return None;
                }
                found = Some(value);
            }
        }
    }
    found
}

fn session_cookie_value(token: &str, secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/{}",
        secure_flag(secure)
    )
}

fn expired_cookie(secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT{}",
        secure_flag(secure)
    )
}

const fn secure_flag(secure: bool) -> &'static str {
    if secure { "; Secure" } else { "" }
}

fn authentication_problem(
    error: AuthHttpError,
    path: &'static str,
    request_id: &RequestId,
) -> Response {
    match error {
        AuthHttpError::InvalidCredentials | AuthHttpError::Unauthenticated => problem(
            StatusCode::UNAUTHORIZED,
            "authentication_failed",
            path,
            request_id,
        ),
        AuthHttpError::CsrfFailed => {
            problem(StatusCode::FORBIDDEN, "csrf_failed", path, request_id)
        }
        AuthHttpError::ValidationFailed => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_failed",
            path,
            request_id,
        ),
        _ => problem(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            path,
            request_id,
        ),
    }
}

fn problem(
    status: StatusCode,
    code: &'static str,
    path: &'static str,
    request_id: &RequestId,
) -> Response {
    let (title, detail) = match status {
        StatusCode::BAD_REQUEST => ("Invalid request", "The request is invalid."),
        StatusCode::UNAUTHORIZED => ("Authentication failed", "Authentication failed."),
        StatusCode::FORBIDDEN => (
            "CSRF verification failed",
            "The CSRF proof is missing or invalid.",
        ),
        StatusCode::UNPROCESSABLE_ENTITY => {
            ("Validation failed", "One or more fields are invalid.")
        }
        StatusCode::TOO_MANY_REQUESTS => ("Rate limit exceeded", "Too many requests."),
        StatusCode::SERVICE_UNAVAILABLE => ("Service unavailable", "The service is not ready."),
        _ => (
            "Internal server error",
            "The request could not be completed.",
        ),
    };
    let problem = Problem {
        r#type: format!("https://takt.dev/problems/{code}"),
        title,
        status: status.as_u16(),
        code,
        detail,
        instance: path,
        request_id: request_id.0.clone(),
    };
    no_store(
        (
            status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            Json(problem),
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
