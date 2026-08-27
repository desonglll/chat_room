use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct WebPushConfig {
    pub enabled: bool,
    pub public_key: String,
    pub private_key: String,
    pub subject: String,
    pub allowed_endpoint_hosts: Vec<String>,
    pub poll_interval_ms: u64,
    pub request_timeout_secs: u64,
    pub max_attempts: i32,
}

impl Default for WebPushConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            public_key: String::new(),
            private_key: String::new(),
            subject: String::new(),
            allowed_endpoint_hosts: vec![
                "fcm.googleapis.com".into(),
                "updates.push.services.mozilla.com".into(),
                "web.push.apple.com".into(),
                "notify.windows.com".into(),
            ],
            poll_interval_ms: 1_000,
            request_timeout_secs: 10,
            max_attempts: 5,
        }
    }
}

impl WebPushConfig {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        if self.poll_interval_ms == 0 || self.poll_interval_ms > 60_000 {
            anyhow::bail!("web_push.poll_interval_ms must be between 1 and 60000");
        }
        if self.request_timeout_secs == 0 || self.request_timeout_secs > 60 {
            anyhow::bail!("web_push.request_timeout_secs must be between 1 and 60");
        }
        if !(1..=20).contains(&self.max_attempts) {
            anyhow::bail!("web_push.max_attempts must be between 1 and 20");
        }
        if self.allowed_endpoint_hosts.is_empty()
            || self.allowed_endpoint_hosts.iter().any(|host| {
                host.trim().is_empty()
                    || host.contains('/')
                    || host.contains(':')
                    || host.chars().any(char::is_whitespace)
            })
        {
            anyhow::bail!("web_push.allowed_endpoint_hosts must contain valid host names");
        }
        if self.enabled {
            if self.public_key.trim().is_empty() || self.private_key.trim().is_empty() {
                anyhow::bail!("web_push public_key and private_key are required when enabled");
            }
            if !(self.subject.starts_with("mailto:") || self.subject.starts_with("https://")) {
                anyhow::bail!("web_push.subject must use mailto: or https://");
            }
            web_push::VapidSignatureBuilder::from_base64_no_sub(&self.private_key).map_err(
                |_| anyhow::anyhow!("web_push.private_key is not a valid URL-safe VAPID key"),
            )?;
        }
        Ok(())
    }
}
