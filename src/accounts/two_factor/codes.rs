//! Mailed six-digit codes: issue (hash, store, deliver) and verify (bounded attempts).

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use uuid::Uuid;

use super::super::credentials::{hash_password, password_matches};
use super::{
    mailer::{current_mailer, MailerError, RecoveryMail, RecoveryMailPurpose},
    store::CodePurpose,
    wire::{email_pattern, MailedCode},
};
use crate::state::AppState;

const CODE_LIFETIME_MINUTES: i64 = 15;
const MAX_CODE_ATTEMPTS: i32 = 5;

/// Six uniformly random decimal digits from the OS CSPRNG behind `Uuid::new_v4`.
fn six_digit_code() -> String {
    let random = Uuid::new_v4();
    let mut bytes = [0u8; 8];
    // Bytes 0..6 of a v4 UUID are fully random (version/variant bits live later).
    bytes[..6].copy_from_slice(&random.as_bytes()[..6]);
    format!("{:06}", u64::from_le_bytes(bytes) % 1_000_000)
}

fn db_error(error: sqlx::Error) -> StatusCode {
    tracing::error!("two-factor code storage failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

/// Store a fresh code (replacing any earlier one for the same purpose) and mail it.
/// A failed delivery withdraws the code, so nothing unusable lingers.
pub(super) async fn issue(
    state: &AppState,
    user_id: Uuid,
    purpose: CodePurpose,
    email: &str,
) -> Result<MailedCode, StatusCode> {
    let code = six_digit_code();
    let expires_at = Utc::now() + Duration::minutes(CODE_LIFETIME_MINUTES);
    let code_hash = hash_password(code.clone()).await?;
    state
        .store_email_code(user_id, purpose, email, &code_hash, expires_at)
        .await
        .map_err(db_error)?;
    let mail = RecoveryMail {
        to: email.to_string(),
        code,
        purpose: match purpose {
            CodePurpose::VerifyEmail => RecoveryMailPurpose::VerifyEmail,
            CodePurpose::Reset => RecoveryMailPurpose::Reset,
        },
        expires_at,
    };
    if let Err(error) = current_mailer().send(mail).await {
        state
            .delete_email_code(user_id, purpose)
            .await
            .map_err(db_error)?;
        return Err(match error {
            MailerError::NotConfigured => {
                tracing::warn!("two-factor email requested but no mail transport is configured");
                StatusCode::SERVICE_UNAVAILABLE
            }
            MailerError::Delivery => {
                tracing::warn!("two-factor email delivery failed");
                StatusCode::BAD_GATEWAY
            }
        });
    }
    Ok(MailedCode {
        email_pattern: email_pattern(email),
        expires_at,
    })
}

/// Check a presented code. `Ok(email)` consumes the code; `401` spends an attempt;
/// `410` means there is no live code (never sent, expired, used, or exhausted).
pub(super) async fn verify(
    state: &AppState,
    user_id: Uuid,
    purpose: CodePurpose,
    code: String,
) -> Result<String, StatusCode> {
    let Some(row) = state
        .reserve_code_attempt(user_id, purpose, MAX_CODE_ATTEMPTS)
        .await
        .map_err(db_error)?
    else {
        return Err(StatusCode::GONE);
    };
    if !password_matches(code, row.code_hash).await {
        return Err(StatusCode::UNAUTHORIZED);
    }
    state
        .delete_email_code(user_id, purpose)
        .await
        .map_err(db_error)?;
    Ok(row.email)
}

#[cfg(test)]
mod tests {
    #[test]
    fn codes_are_six_digits_and_vary() {
        let codes: std::collections::HashSet<String> =
            (0..32).map(|_| super::six_digit_code()).collect();
        assert!(codes.len() > 1);
        assert!(codes
            .iter()
            .all(|c| c.len() == 6 && c.bytes().all(|b| b.is_ascii_digit())));
    }
}
