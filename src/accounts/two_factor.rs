//! TG-506: two-step verification ("cloud password").
//!
//! A second argon2 password guards login after the account password. Login becomes two
//! stages: the account password earns a short-lived *pending token* (not a session), and
//! the 2FA password exchanges it for a real session. A verified recovery email can replace
//! a forgotten 2FA password, but only for a caller that already holds a pending token, so
//! recovery never skips the account password and never skips proof of the second factor.
//!
//! Session policy (docs/tg/decisions.md D-010): enabling 2FA and completing a recovery
//! reset terminate every other device session of the account; the current one survives.
//!
//! Layout: `wire` (request/response shapes + validation), `store` (persistence on
//! `AppState`), `mailer` (pluggable code delivery), `login` and `settings` (handlers).

mod codes;
pub(crate) mod login;
mod mailer;
pub(crate) mod settings;
mod store;
mod wire;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::SharedState;

pub(crate) use login::challenge_response;
pub use mailer::{
    install_recovery_mailer, MailerError, MemoryMailer, RecoveryMail, RecoveryMailPurpose,
    RecoveryMailer,
};
pub use wire::{
    ChangeTwoFactorRequest, CompleteTwoFactorLoginRequest, ConfirmRecoveryEmailRequest,
    ConfirmRecoveryRequest, DisableTwoFactorRequest, EnableTwoFactorRequest, MailedCode,
    RecoveryEmailRequest, RecoveryRequest, TwoFactorChallenge, TwoFactorStatus,
};

use login::{complete_login, confirm_recovery, request_recovery};
use settings::{change, confirm_recovery_email, disable, enable, request_recovery_email, status};

/// Every TG-506 route. Mounted once from `src/routes.rs`.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/users/login/two-factor", post(complete_login))
        .route(
            "/api/users/login/two-factor/recovery",
            post(request_recovery),
        )
        .route(
            "/api/users/login/two-factor/recovery/confirm",
            post(confirm_recovery),
        )
        .route(
            "/api/users/me/two-factor",
            get(status).post(enable).put(change).delete(disable),
        )
        .route(
            "/api/users/me/two-factor/recovery-email",
            post(request_recovery_email),
        )
        .route(
            "/api/users/me/two-factor/recovery-email/confirm",
            post(confirm_recovery_email),
        )
}
