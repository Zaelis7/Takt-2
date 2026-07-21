use std::{
    collections::HashMap,
    fmt,
    net::{IpAddr, SocketAddr},
    sync::Mutex,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{ConnectInfo, State, rejection::JsonRejection},
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
const LOGIN_ATTEMPTS_PER_WINDOW: u32 = 10;
const LOGIN_WINDOW: Duration = Duration::from_secs(60);
const FAILURE_DELAY_STEP: Duration = Duration::from_millis(100);
const FAILURE_DELAY_MAX: Duration = Duration::from_secs(2);
const MAX_RATE_LIMIT_KEYS: usize = 20_000;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthHttpConfig {
    pub(crate) secure_cookies: bool,
    login_attempt_limit: u32,
    login_window: Duration,
    failure_delay_step: Duration,
    failure_delay_max: Duration,
}

impl AuthHttpConfig {
    #[must_use]
    pub const fn localhost() -> Self {
        Self {
            secure_cookies: false,
            login_attempt_limit: LOGIN_ATTEMPTS_PER_WINDOW,
            login_window: LOGIN_WINDOW,
            failure_delay_step: FAILURE_DELAY_STEP,
            failure_delay_max: FAILURE_DELAY_MAX,
        }
    }

    #[must_use]
    pub const fn production() -> Self {
        Self {
            secure_cookies: true,
            login_attempt_limit: LOGIN_ATTEMPTS_PER_WINDOW,
            login_window: LOGIN_WINDOW,
            failure_delay_step: FAILURE_DELAY_STEP,
            failure_delay_max: FAILURE_DELAY_MAX,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum RateKey {
    Peer(IpAddr),
    Account(String),
}

struct RateBucket {
    window_started: Instant,
    attempts: u32,
    failures: u32,
}

#[derive(Default)]
struct RateState {
    buckets: HashMap<RateKey, RateBucket>,
}

enum LoginAdmission {
    Allowed,
    Limited { retry_after_seconds: u64 },
}

pub(crate) struct LoginGuard {
    state: Mutex<RateState>,
    attempt_limit: u32,
    window: Duration,
    failure_delay_step: Duration,
    failure_delay_max: Duration,
}

impl LoginGuard {
    pub(crate) fn new(config: AuthHttpConfig) -> Self {
        Self::with_settings(
            config.login_attempt_limit,
            config.login_window,
            config.failure_delay_step,
            config.failure_delay_max,
        )
    }

    fn with_settings(
        attempt_limit: u32,
        window: Duration,
        failure_delay_step: Duration,
        failure_delay_max: Duration,
    ) -> Self {
        debug_assert!(attempt_limit > 0);
        debug_assert!(!window.is_zero());
        Self {
            state: Mutex::new(RateState::default()),
            attempt_limit,
            window,
            failure_delay_step,
            failure_delay_max,
        }
    }

    fn admit(&self, peer: IpAddr, account: &str, now: Instant) -> Result<LoginAdmission, ()> {
        let keys = [RateKey::Peer(peer), RateKey::Account(account.to_owned())];
        let mut state = self.state.lock().map_err(|_| ())?;
        let missing = keys
            .iter()
            .filter(|key| !state.buckets.contains_key(*key))
            .count();
        if state.buckets.len().saturating_add(missing) > MAX_RATE_LIMIT_KEYS {
            state.buckets.retain(|_, bucket| {
                now.saturating_duration_since(bucket.window_started) < self.window
            });
        }
        if state.buckets.len().saturating_add(missing) > MAX_RATE_LIMIT_KEYS {
            return Ok(LoginAdmission::Limited {
                retry_after_seconds: duration_seconds_ceil(self.window),
            });
        }

        for key in &keys {
            let bucket = state.buckets.entry(key.clone()).or_insert(RateBucket {
                window_started: now,
                attempts: 0,
                failures: 0,
            });
            if now.saturating_duration_since(bucket.window_started) >= self.window {
                *bucket = RateBucket {
                    window_started: now,
                    attempts: 0,
                    failures: 0,
                };
            }
        }

        let mut retry_after_seconds = 0;
        for key in &keys {
            let Some(bucket) = state.buckets.get(key) else {
                return Err(());
            };
            if bucket.attempts >= self.attempt_limit {
                let elapsed = now.saturating_duration_since(bucket.window_started);
                retry_after_seconds = retry_after_seconds
                    .max(duration_seconds_ceil(self.window.saturating_sub(elapsed)));
            }
        }
        if retry_after_seconds > 0 {
            return Ok(LoginAdmission::Limited {
                retry_after_seconds,
            });
        }
        for key in &keys {
            let Some(bucket) = state.buckets.get_mut(key) else {
                return Err(());
            };
            bucket.attempts = bucket.attempts.saturating_add(1);
        }
        Ok(LoginAdmission::Allowed)
    }

    fn record_failure(&self, peer: IpAddr, account: &str, now: Instant) -> Result<Duration, ()> {
        let keys = [RateKey::Peer(peer), RateKey::Account(account.to_owned())];
        let mut state = self.state.lock().map_err(|_| ())?;
        let mut failures = 0;
        for key in &keys {
            let Some(bucket) = state.buckets.get_mut(key) else {
                return Err(());
            };
            if now.saturating_duration_since(bucket.window_started) < self.window {
                bucket.failures = bucket.failures.saturating_add(1);
                failures = failures.max(bucket.failures);
            }
        }
        Ok(self
            .failure_delay_step
            .saturating_mul(failures)
            .min(self.failure_delay_max))
    }

    fn record_success(&self, account: &str) -> Result<(), ()> {
        let key = RateKey::Account(account.to_owned());
        let mut state = self.state.lock().map_err(|_| ())?;
        let Some(bucket) = state.buckets.get_mut(&key) else {
            return Err(());
        };
        bucket.failures = 0;
        Ok(())
    }
}

fn duration_seconds_ceil(duration: Duration) -> u64 {
    duration.as_secs() + u64::from(duration.subsec_nanos() > 0)
}

fn rate_account_key(username: &str) -> String {
    let normalized = username.trim().to_ascii_lowercase();
    let valid = (1..=64).contains(&normalized.len())
        && normalized
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && normalized
            .bytes()
            .next_back()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && normalized
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        normalized
    } else {
        "invalid-account".to_owned()
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
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
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
    let account = rate_account_key(&payload.username);
    match state.login_guard.admit(peer.ip(), &account, Instant::now()) {
        Ok(LoginAdmission::Allowed) => {}
        Ok(LoginAdmission::Limited {
            retry_after_seconds,
        }) => return rate_limit_problem(LOGIN_PATH, &request_id, retry_after_seconds),
        Err(()) => {
            return problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                LOGIN_PATH,
                &request_id,
            );
        }
    }
    let result = authentication
        .login(&payload.username, &payload.password, &request_id.0)
        .await;
    match result {
        Ok(login) => {
            if state.login_guard.record_success(&account).is_err() {
                tracing::warn!(
                    event_code = "login_rate_limit_state_failed",
                    request_id = %request_id.0,
                    "login succeeded but rate-limit success bookkeeping failed"
                );
            }
            login_response(login, state.auth_config.secure_cookies, &request_id)
        }
        Err(AuthHttpError::InvalidCredentials) => {
            let delay = match state
                .login_guard
                .record_failure(peer.ip(), &account, Instant::now())
            {
                Ok(delay) => delay,
                Err(()) => {
                    return problem(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal_error",
                        LOGIN_PATH,
                        &request_id,
                    );
                }
            };
            tokio::time::sleep(delay).await;
            authentication_problem(AuthHttpError::InvalidCredentials, LOGIN_PATH, &request_id)
        }
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

fn rate_limit_problem(
    path: &'static str,
    request_id: &RequestId,
    retry_after_seconds: u64,
) -> Response {
    let mut response = problem(
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limit_exceeded",
        path,
        request_id,
    );
    if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_guard() -> LoginGuard {
        LoginGuard::with_settings(
            2,
            Duration::from_secs(60),
            Duration::from_millis(10),
            Duration::from_millis(20),
        )
    }

    #[test]
    fn login_limits_are_independent_per_peer_and_account() -> Result<(), ()> {
        let now = Instant::now();
        let first_peer = IpAddr::from([192, 0, 2, 1]);
        let account_guard = test_guard();
        assert!(matches!(
            account_guard.admit(first_peer, "owner", now)?,
            LoginAdmission::Allowed
        ));
        assert!(matches!(
            account_guard.admit(IpAddr::from([192, 0, 2, 2]), "owner", now)?,
            LoginAdmission::Allowed
        ));
        assert!(matches!(
            account_guard.admit(IpAddr::from([192, 0, 2, 3]), "owner", now)?,
            LoginAdmission::Limited { .. }
        ));

        let peer_guard = test_guard();
        assert!(matches!(
            peer_guard.admit(first_peer, "first", now)?,
            LoginAdmission::Allowed
        ));
        assert!(matches!(
            peer_guard.admit(first_peer, "second", now)?,
            LoginAdmission::Allowed
        ));
        assert!(matches!(
            peer_guard.admit(first_peer, "third", now)?,
            LoginAdmission::Limited { .. }
        ));
        Ok(())
    }

    #[test]
    fn failure_delay_increases_is_capped_and_window_resets() -> Result<(), ()> {
        let guard = test_guard();
        let peer = IpAddr::from([192, 0, 2, 10]);
        let now = Instant::now();
        assert!(matches!(
            guard.admit(peer, "owner", now)?,
            LoginAdmission::Allowed
        ));
        assert_eq!(
            guard.record_failure(peer, "owner", now)?,
            Duration::from_millis(10)
        );
        assert_eq!(
            guard.record_failure(peer, "owner", now)?,
            Duration::from_millis(20)
        );
        assert_eq!(
            guard.record_failure(peer, "owner", now)?,
            Duration::from_millis(20)
        );
        assert!(matches!(
            guard.admit(peer, "owner", now + Duration::from_secs(61))?,
            LoginAdmission::Allowed
        ));
        assert_eq!(
            guard.record_failure(peer, "owner", now + Duration::from_secs(61))?,
            Duration::from_millis(10)
        );
        Ok(())
    }

    #[test]
    fn production_limit_cannot_be_disabled() {
        let config = AuthHttpConfig::production();
        assert_eq!(config.login_attempt_limit, 10);
        assert_eq!(config.login_window, Duration::from_secs(60));
    }

    #[test]
    fn account_keys_are_normalized_and_invalid_values_are_bounded() {
        assert_eq!(rate_account_key(" Owner.Admin "), "owner.admin");
        assert_eq!(rate_account_key(&"x".repeat(4_000)), "invalid-account");
        assert_eq!(rate_account_key("öwner"), "invalid-account");
    }
}
