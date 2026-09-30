//! Pluggable delivery of 2FA email codes.
//!
//! This server has **no mail transport**. Until a deployment installs a real
//! `RecoveryMailer` with `install_recovery_mailer`, the default `UnconfiguredMailer`
//! refuses every send and the recovery-email endpoints answer `503`. Codes are never
//! logged by any implementation here, and `RecoveryMail`'s `Debug` redacts the code so an
//! accidental `{:?}` cannot leak it either.

use std::{
    fmt,
    sync::{Arc, Mutex, OnceLock, RwLock},
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryMailPurpose {
    /// Prove ownership of a new recovery address.
    VerifyEmail,
    /// Reset a forgotten 2FA password during login.
    Reset,
}

#[derive(Clone)]
pub struct RecoveryMail {
    pub to: String,
    pub code: String,
    pub purpose: RecoveryMailPurpose,
    pub expires_at: DateTime<Utc>,
}

impl fmt::Debug for RecoveryMail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecoveryMail")
            .field("purpose", &self.purpose)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MailerError {
    /// No transport installed on this server.
    NotConfigured,
    /// The transport exists but refused or failed the message.
    Delivery,
}

#[async_trait]
pub trait RecoveryMailer: Send + Sync {
    async fn send(&self, mail: RecoveryMail) -> Result<(), MailerError>;
}

/// The default: production mail delivery is not configured.
struct UnconfiguredMailer;

#[async_trait]
impl RecoveryMailer for UnconfiguredMailer {
    async fn send(&self, _mail: RecoveryMail) -> Result<(), MailerError> {
        Err(MailerError::NotConfigured)
    }
}

/// An in-process outbox for tests and local development. Nothing is logged; a caller
/// reads the code back with `take_latest`.
#[derive(Default)]
pub struct MemoryMailer {
    outbox: Mutex<Vec<RecoveryMail>>,
}

impl MemoryMailer {
    /// Remove and return the newest message addressed to `to`.
    pub fn take_latest(&self, to: &str) -> Option<RecoveryMail> {
        let mut outbox = self.outbox.lock().unwrap_or_else(|p| p.into_inner());
        let at = outbox.iter().rposition(|mail| mail.to == to)?;
        Some(outbox.remove(at))
    }
}

#[async_trait]
impl RecoveryMailer for MemoryMailer {
    async fn send(&self, mail: RecoveryMail) -> Result<(), MailerError> {
        self.outbox
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(mail);
        Ok(())
    }
}

type Slot = RwLock<Arc<dyn RecoveryMailer>>;

fn slot() -> &'static Slot {
    static SLOT: OnceLock<Slot> = OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(Arc::new(UnconfiguredMailer)))
}

/// Install the process-wide mail transport (a deployment's SMTP adapter, or a test outbox).
/// Process-wide rather than per-`AppState` so no shared state struct had to change.
pub fn install_recovery_mailer(mailer: Arc<dyn RecoveryMailer>) {
    *slot().write().unwrap_or_else(|p| p.into_inner()) = mailer;
}

pub(super) fn current_mailer() -> Arc<dyn RecoveryMailer> {
    slot().read().unwrap_or_else(|p| p.into_inner()).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_default_mailer_is_unconfigured_and_debug_redacts_the_code() {
        let mail = RecoveryMail {
            to: "a@b.cd".into(),
            code: "123456".into(),
            purpose: RecoveryMailPurpose::Reset,
            expires_at: Utc::now(),
        };
        assert!(!format!("{mail:?}").contains("123456"));
        assert_eq!(
            UnconfiguredMailer.send(mail).await,
            Err(MailerError::NotConfigured)
        );
    }
}
