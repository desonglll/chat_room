use serde::Deserialize;

use anyhow::{bail, Result};

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub session_lifetime_days: i64,
    /// Public account creation policy: open, invite_only, or disabled.
    pub registration_mode: String,
    pub rate_limit_window_secs: u64,
    pub rate_limit_ip_attempts: u64,
    pub rate_limit_account_attempts: u64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_lifetime_days: 30,
            registration_mode: "open".into(),
            rate_limit_window_secs: 60,
            rate_limit_ip_attempts: 60,
            rate_limit_account_attempts: 10,
        }
    }
}

impl AuthConfig {
    pub(super) fn validate(&self) -> Result<()> {
        if self.session_lifetime_days <= 0 {
            bail!("auth.session_lifetime_days must be greater than zero");
        }
        if self.rate_limit_window_secs == 0
            || self.rate_limit_ip_attempts == 0
            || self.rate_limit_account_attempts == 0
        {
            bail!("auth rate limit values must be greater than zero");
        }
        if !matches!(
            self.registration_mode.as_str(),
            "open" | "invite_only" | "disabled"
        ) {
            bail!("auth.registration_mode must be open, invite_only, or disabled");
        }
        Ok(())
    }
}
