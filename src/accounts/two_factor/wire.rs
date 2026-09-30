//! TG-506 request/response shapes and the input rules they carry.

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::super::credentials::MAX_PASSWORD_CHARS;

pub(super) const MAX_HINT_CHARS: usize = 64;
const MAX_EMAIL_CHARS: usize = 254;

/// `428` body of `POST /api/users/login` for an account with 2FA on. Clients that predate
/// TG-506 treat it as a failed login; they keep working for accounts without 2FA.
#[derive(Debug, Serialize, ToSchema)]
pub struct TwoFactorChallenge {
    /// Always `"two_factor_required"`.
    pub error: &'static str,
    /// Exchange for a session at `POST /api/users/login/two-factor`. Not a bearer token.
    pub pending_token: Uuid,
    pub hint: String,
    pub has_recovery_email: bool,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CompleteTwoFactorLoginRequest {
    pub pending_token: Uuid,
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RecoveryRequest {
    pub pending_token: Uuid,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfirmRecoveryRequest {
    pub pending_token: Uuid,
    pub code: String,
}

/// `202` body when a code was handed to the mailer.
#[derive(Debug, Serialize, ToSchema)]
pub struct MailedCode {
    /// Masked destination, e.g. `a***@example.com`.
    pub email_pattern: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Default, Serialize, ToSchema)]
pub struct TwoFactorStatus {
    pub enabled: bool,
    pub hint: String,
    /// Verified recovery address, or null.
    pub recovery_email: Option<String>,
    /// Address waiting for its mailed code, or null.
    pub pending_recovery_email: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct EnableTwoFactorRequest {
    /// The account (first-factor) password; a stolen session alone cannot turn 2FA on.
    pub account_password: String,
    pub password: String,
    #[serde(default)]
    pub hint: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeTwoFactorRequest {
    pub current_password: String,
    pub new_password: Option<String>,
    pub hint: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DisableTwoFactorRequest {
    pub current_password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RecoveryEmailRequest {
    pub current_password: String,
    pub email: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfirmRecoveryEmailRequest {
    pub code: String,
}

/// A presented password may be any non-empty string up to the account-password ceiling.
pub(super) fn presented_password(password: &str) -> Result<(), StatusCode> {
    let length = password.chars().count();
    if (1..=MAX_PASSWORD_CHARS).contains(&length) {
        Ok(())
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

/// A hint is short, printable, and must not give the password away.
pub(super) fn valid_hint(hint: &str, password: &str) -> Result<String, StatusCode> {
    let hint = hint.trim();
    if hint.chars().count() > MAX_HINT_CHARS
        || hint.chars().any(char::is_control)
        || (!hint.is_empty() && hint.to_lowercase().contains(&password.to_lowercase()))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(hint.to_string())
}

pub(super) fn normalize_email(email: &str) -> Result<String, StatusCode> {
    let email = email.trim().to_lowercase();
    let valid = email.chars().count() <= MAX_EMAIL_CHARS
        && !email.chars().any(|c| c.is_whitespace() || c.is_control())
        && email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && !domain.contains('@')
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        });
    valid.then_some(email).ok_or(StatusCode::BAD_REQUEST)
}

/// `alice@example.com` → `a***@example.com`.
pub(super) fn email_pattern(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) => {
            let first: String = local.chars().take(1).collect();
            format!("{first}***@{domain}")
        }
        None => "***".into(),
    }
}

/// Mailed codes are exactly six ASCII digits.
pub(super) fn well_formed_code(code: &str) -> Result<String, StatusCode> {
    let code = code.trim();
    (code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()))
        .then(|| code.to_string())
        .ok_or(StatusCode::BAD_REQUEST)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_may_not_reveal_the_password() {
        assert!(valid_hint("my cat", "hunter2-long").is_ok());
        assert!(valid_hint("it is Hunter2-LONG", "hunter2-long").is_err());
        assert!(valid_hint(&"x".repeat(65), "pw").is_err());
        assert_eq!(valid_hint("  ", "pw").unwrap(), "");
    }

    #[test]
    fn emails_are_normalized_and_checked() {
        assert_eq!(normalize_email(" A@Ex.com ").unwrap(), "a@ex.com");
        for bad in ["a@b", "@b.c", "a b@c.d", "a@@c.d", "a@.cd", "a@cd."] {
            assert!(normalize_email(bad).is_err(), "{bad}");
        }
        assert_eq!(email_pattern("alice@example.com"), "a***@example.com");
    }

    #[test]
    fn codes_are_six_digits() {
        assert_eq!(well_formed_code(" 012345 ").unwrap(), "012345");
        assert!(well_formed_code("12345").is_err());
        assert!(well_formed_code("12345a").is_err());
    }
}
