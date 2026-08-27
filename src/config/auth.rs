use serde::Deserialize;

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
