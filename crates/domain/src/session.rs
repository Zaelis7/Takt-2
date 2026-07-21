//! Pure browser-session lifetime and security-transition rules.

use std::{error::Error, fmt};

use crate::{OrganizationId, SessionId, UserId, UtcTimestamp};

/// Default inactivity limit required by the 0.1 security contract.
pub const DEFAULT_INACTIVITY_TIMEOUT_MICROS: i64 = 12 * 60 * 60 * 1_000_000;
/// Default absolute session lifetime required by the 0.1 security contract.
pub const DEFAULT_ABSOLUTE_TIMEOUT_MICROS: i64 = 7 * 24 * 60 * 60 * 1_000_000;

/// Validated, deterministic lifetime limits for a server-side browser session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionPolicy {
    inactivity_timeout_micros: i64,
    absolute_timeout_micros: i64,
}

impl SessionPolicy {
    /// Creates a policy whose inactivity limit is positive and no longer than
    /// its positive absolute lifetime.
    pub fn new(
        inactivity_timeout_micros: i64,
        absolute_timeout_micros: i64,
    ) -> Result<Self, SessionPolicyError> {
        if inactivity_timeout_micros <= 0 || absolute_timeout_micros <= 0 {
            return Err(SessionPolicyError::NonPositiveTimeout);
        }
        if inactivity_timeout_micros > absolute_timeout_micros {
            return Err(SessionPolicyError::InactivityExceedsAbsolute);
        }
        Ok(Self {
            inactivity_timeout_micros,
            absolute_timeout_micros,
        })
    }

    /// Returns the configured inactivity limit in microseconds.
    #[must_use]
    pub const fn inactivity_timeout_micros(self) -> i64 {
        self.inactivity_timeout_micros
    }

    /// Returns the configured absolute lifetime in microseconds.
    #[must_use]
    pub const fn absolute_timeout_micros(self) -> i64 {
        self.absolute_timeout_micros
    }
}

impl Default for SessionPolicy {
    fn default() -> Self {
        Self {
            inactivity_timeout_micros: DEFAULT_INACTIVITY_TIMEOUT_MICROS,
            absolute_timeout_micros: DEFAULT_ABSOLUTE_TIMEOUT_MICROS,
        }
    }
}

/// Invalid operator-supplied session policy or an unrepresentable expiry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionPolicyError {
    NonPositiveTimeout,
    InactivityExceedsAbsolute,
    TimestampOverflow,
    InvalidStoredWindow,
}

impl fmt::Display for SessionPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NonPositiveTimeout => "session timeouts must be positive",
            Self::InactivityExceedsAbsolute => {
                "session inactivity timeout must not exceed its absolute lifetime"
            }
            Self::TimestampOverflow => "session expiry is outside the supported UTC range",
            Self::InvalidStoredWindow => "stored session time boundaries are inconsistent",
        };
        formatter.write_str(message)
    }
}

impl Error for SessionPolicyError {}

/// Reason that a session can no longer authorize a request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionExpiry {
    Inactive,
    AbsoluteLifetime,
}

impl fmt::Display for SessionExpiry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inactive => formatter.write_str("session expired after inactivity"),
            Self::AbsoluteLifetime => formatter.write_str("session reached its absolute lifetime"),
        }
    }
}

impl Error for SessionExpiry {}

/// Time boundaries for one issued server-side session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionWindow {
    issued_at: UtcTimestamp,
    last_activity_at: UtcTimestamp,
    expires_at: UtcTimestamp,
    absolute_expires_at: UtcTimestamp,
}

impl SessionWindow {
    /// Issues fresh lifetime boundaries from an injected UTC time.
    pub fn issue(
        issued_at: UtcTimestamp,
        policy: SessionPolicy,
    ) -> Result<Self, SessionPolicyError> {
        let inactivity_expiry = checked_add(issued_at, policy.inactivity_timeout_micros())?;
        let absolute_expiry = checked_add(issued_at, policy.absolute_timeout_micros())?;
        Ok(Self {
            issued_at,
            last_activity_at: issued_at,
            expires_at: inactivity_expiry,
            absolute_expires_at: absolute_expiry,
        })
    }

    /// Reconstructs persisted boundaries while rejecting impossible ordering.
    pub fn from_persistence(
        issued_at: UtcTimestamp,
        last_activity_at: UtcTimestamp,
        expires_at: UtcTimestamp,
        absolute_expires_at: UtcTimestamp,
    ) -> Result<Self, SessionPolicyError> {
        if last_activity_at < issued_at
            || expires_at <= last_activity_at
            || absolute_expires_at < expires_at
        {
            return Err(SessionPolicyError::InvalidStoredWindow);
        }
        Ok(Self {
            issued_at,
            last_activity_at,
            expires_at,
            absolute_expires_at,
        })
    }

    #[must_use]
    pub const fn issued_at(self) -> UtcTimestamp {
        self.issued_at
    }

    #[must_use]
    pub const fn last_activity_at(self) -> UtcTimestamp {
        self.last_activity_at
    }

    #[must_use]
    pub const fn expires_at(self) -> UtcTimestamp {
        self.expires_at
    }

    #[must_use]
    pub const fn absolute_expires_at(self) -> UtcTimestamp {
        self.absolute_expires_at
    }

    /// Returns an expiry reason at the exact boundary or later.
    #[must_use]
    pub fn expiry_at(self, now: UtcTimestamp) -> Option<SessionExpiry> {
        if now >= self.absolute_expires_at {
            Some(SessionExpiry::AbsoluteLifetime)
        } else if now >= self.expires_at {
            Some(SessionExpiry::Inactive)
        } else {
            None
        }
    }

    /// Records authenticated activity without ever extending the absolute
    /// lifetime. Expired sessions cannot be revived.
    pub fn record_activity(
        self,
        now: UtcTimestamp,
        policy: SessionPolicy,
    ) -> Result<Self, SessionExpiry> {
        if let Some(expiry) = self.expiry_at(now) {
            return Err(expiry);
        }
        let refreshed_micros = now
            .unix_micros()
            .saturating_add(policy.inactivity_timeout_micros())
            .min(self.absolute_expires_at.unix_micros());
        Ok(Self {
            last_activity_at: now,
            expires_at: UtcTimestamp::from_unix_micros(refreshed_micros),
            ..self
        })
    }

    /// Applies the domain authorization rule for a browser state change after
    /// the API boundary has verified whether the proof belongs to this session.
    pub fn authorize_browser_write(
        self,
        now: UtcTimestamp,
        csrf: CsrfProof,
    ) -> Result<(), BrowserWriteRejection> {
        if let Some(expiry) = self.expiry_at(now) {
            return Err(BrowserWriteRejection::Expired(expiry));
        }
        match csrf {
            CsrfProof::Missing => Err(BrowserWriteRejection::MissingCsrf),
            CsrfProof::InvalidOrFromAnotherSession => Err(BrowserWriteRejection::InvalidCsrf),
            CsrfProof::VerifiedForCurrentSession => Ok(()),
        }
    }
}

fn checked_add(
    timestamp: UtcTimestamp,
    delta_micros: i64,
) -> Result<UtcTimestamp, SessionPolicyError> {
    timestamp
        .unix_micros()
        .checked_add(delta_micros)
        .map(UtcTimestamp::from_unix_micros)
        .ok_or(SessionPolicyError::TimestampOverflow)
}

/// Result of verifying the CSRF input at the secret-handling boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CsrfProof {
    Missing,
    InvalidOrFromAnotherSession,
    VerifiedForCurrentSession,
}

/// Stable domain rejection classes for a browser mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserWriteRejection {
    Expired(SessionExpiry),
    MissingCsrf,
    InvalidCsrf,
}

/// Security-sensitive events that have an explicit session consequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionSecurityEvent {
    Login,
    PermissionsChanged,
    SensitiveAction,
    Logout,
    PasswordRecoveryCompleted,
}

impl SessionSecurityEvent {
    /// Returns the mandatory action without consulting storage or a clock.
    #[must_use]
    pub const fn required_action(self) -> SessionSecurityAction {
        match self {
            Self::Login | Self::PermissionsChanged | Self::SensitiveAction => {
                SessionSecurityAction::RotateSession
            }
            Self::Logout => SessionSecurityAction::RevokeCurrentSession,
            Self::PasswordRecoveryCompleted => SessionSecurityAction::RevokeAllUserSessions,
        }
    }
}

/// Required storage/application action for a session security event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionSecurityAction {
    RotateSession,
    RevokeCurrentSession,
    RevokeAllUserSessions,
}

/// Persisted, secret-free representation of a server-side browser session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrowserSession {
    pub id: SessionId,
    pub organization_id: OrganizationId,
    pub user_id: UserId,
    pub window: SessionWindow,
    pub revoked_at: Option<UtcTimestamp>,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub version: i64,
}
