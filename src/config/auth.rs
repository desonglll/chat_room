use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub session_lifetime_days: i64,
    /// Public account creation policy: open, invite_only, or disabled.
    pub registration_mode: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_lifetime_days: 30,
            registration_mode: "open".into(),
        }
    }
}
