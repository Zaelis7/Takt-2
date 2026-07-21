#![forbid(unsafe_code)]

use std::error::Error;

use takt_domain::{
    UtcTimestamp,
    session::{
        BrowserWriteRejection, CsrfProof, SessionExpiry, SessionPolicy, SessionPolicyError,
        SessionSecurityAction, SessionSecurityEvent, SessionWindow,
    },
};

const HOUR_MICROS: i64 = 60 * 60 * 1_000_000;
const DAY_MICROS: i64 = 24 * HOUR_MICROS;

fn timestamp(micros: i64) -> UtcTimestamp {
    UtcTimestamp::from_unix_micros(micros)
}

// PRD-IAM-001: defaults and activity refresh are deterministic.
#[test]
fn prd_iam_001_session_defaults_and_refresh_respect_absolute_expiry() -> Result<(), Box<dyn Error>>
{
    let policy = SessionPolicy::default();
    assert_eq!(policy.inactivity_timeout_micros(), 12 * HOUR_MICROS);
    assert_eq!(policy.absolute_timeout_micros(), 7 * DAY_MICROS);

    let issued_at = timestamp(1_000_000);
    let session = SessionWindow::issue(issued_at, policy)?;
    assert_eq!(session.issued_at(), issued_at);
    assert_eq!(session.last_activity_at(), issued_at);
    assert_eq!(
        session.expires_at(),
        timestamp(issued_at.unix_micros() + 12 * HOUR_MICROS)
    );
    assert_eq!(
        session.absolute_expires_at(),
        timestamp(issued_at.unix_micros() + 7 * DAY_MICROS)
    );

    let clamped_policy = SessionPolicy::new(12 * HOUR_MICROS, 13 * HOUR_MICROS)?;
    let short_session = SessionWindow::issue(issued_at, clamped_policy)?;
    let late_activity = timestamp(issued_at.unix_micros() + 11 * HOUR_MICROS);
    let refreshed = short_session.record_activity(late_activity, clamped_policy)?;
    assert_eq!(refreshed.last_activity_at(), late_activity);
    assert_eq!(refreshed.expires_at(), short_session.absolute_expires_at());
    assert_eq!(
        refreshed.absolute_expires_at(),
        short_session.absolute_expires_at()
    );
    Ok(())
}

// PRD-IAM-001: neither activity nor CSRF can revive an expired session.
#[test]
fn prd_iam_001_expired_session_is_rejected_at_the_exact_boundary() -> Result<(), Box<dyn Error>> {
    let policy = SessionPolicy::default();
    let session = SessionWindow::issue(timestamp(0), policy)?;
    let boundary = session.expires_at();

    assert_eq!(session.expiry_at(boundary), Some(SessionExpiry::Inactive));
    assert_eq!(
        session.record_activity(boundary, policy),
        Err(SessionExpiry::Inactive)
    );
    assert_eq!(
        session.authorize_browser_write(boundary, CsrfProof::VerifiedForCurrentSession),
        Err(BrowserWriteRejection::Expired(SessionExpiry::Inactive))
    );
    Ok(())
}

// PRD-IAM-001: browser mutations require proof bound to the current session.
#[test]
fn prd_iam_001_browser_writes_require_session_bound_csrf() -> Result<(), Box<dyn Error>> {
    let session = SessionWindow::issue(timestamp(0), SessionPolicy::default())?;
    let now = timestamp(HOUR_MICROS);

    assert_eq!(
        session.authorize_browser_write(now, CsrfProof::Missing),
        Err(BrowserWriteRejection::MissingCsrf)
    );
    assert_eq!(
        session.authorize_browser_write(now, CsrfProof::InvalidOrFromAnotherSession),
        Err(BrowserWriteRejection::InvalidCsrf)
    );
    assert_eq!(
        session.authorize_browser_write(now, CsrfProof::VerifiedForCurrentSession),
        Ok(())
    );
    Ok(())
}

// PRD-IAM-001: login and privilege changes rotate; logout and recovery revoke.
#[test]
fn prd_iam_001_security_events_have_explicit_session_actions() {
    assert_eq!(
        SessionSecurityEvent::Login.required_action(),
        SessionSecurityAction::RotateSession
    );
    assert_eq!(
        SessionSecurityEvent::PermissionsChanged.required_action(),
        SessionSecurityAction::RotateSession
    );
    assert_eq!(
        SessionSecurityEvent::SensitiveAction.required_action(),
        SessionSecurityAction::RotateSession
    );
    assert_eq!(
        SessionSecurityEvent::Logout.required_action(),
        SessionSecurityAction::RevokeCurrentSession
    );
    assert_eq!(
        SessionSecurityEvent::PasswordRecoveryCompleted.required_action(),
        SessionSecurityAction::RevokeAllUserSessions
    );
}

#[test]
fn prd_iam_001_session_policy_rejects_non_positive_or_inverted_windows() {
    assert!(SessionPolicy::new(0, DAY_MICROS).is_err());
    assert!(SessionPolicy::new(HOUR_MICROS, 0).is_err());
    assert!(SessionPolicy::new(DAY_MICROS + 1, DAY_MICROS).is_err());
    assert_eq!(
        SessionWindow::issue(timestamp(i64::MAX), SessionPolicy::default()),
        Err(SessionPolicyError::TimestampOverflow)
    );
}

#[test]
fn prd_iam_001_absolute_expiry_wins_when_both_boundaries_match() -> Result<(), Box<dyn Error>> {
    let policy = SessionPolicy::new(HOUR_MICROS, HOUR_MICROS)?;
    let session = SessionWindow::issue(timestamp(0), policy)?;
    assert_eq!(
        session.expiry_at(timestamp(HOUR_MICROS)),
        Some(SessionExpiry::AbsoluteLifetime)
    );
    Ok(())
}
