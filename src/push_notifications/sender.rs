use std::time::Duration;

use async_trait::async_trait;
use web_push::{
    request_builder, ContentEncoding, SubscriptionInfo, VapidSignatureBuilder,
    WebPushMessageBuilder,
};

use super::{
    config::WebPushConfig,
    delivery::{PushSendOutcome, PushSender},
    models::{ClaimedPushJob, PushPayload},
};

pub(crate) struct ProductionPushSender {
    client: reqwest::Client,
    private_key: String,
    subject: String,
}

impl ProductionPushSender {
    pub(crate) fn new(config: &WebPushConfig) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            private_key: config.private_key.clone(),
            subject: config.subject.clone(),
        })
    }

    async fn send_encrypted(
        &self,
        job: &ClaimedPushJob,
        payload: &PushPayload,
    ) -> Result<reqwest::StatusCode, web_push::WebPushError> {
        let subscription = SubscriptionInfo::new(&job.endpoint, &job.p256dh, &job.auth);
        let mut signature = VapidSignatureBuilder::from_base64(&self.private_key, &subscription)?;
        signature.add_claim("sub", self.subject.as_str());
        let signature = signature.build()?;
        let content =
            serde_json::to_vec(payload).map_err(|_| web_push::WebPushError::Unspecified)?;
        let mut message = WebPushMessageBuilder::new(&subscription);
        message.set_payload(ContentEncoding::Aes128Gcm, &content);
        message.set_vapid_signature(signature);
        message.set_ttl(86_400);
        let request = request_builder::build_request::<Vec<u8>>(message.build()?);
        let (parts, body) = request.into_parts();
        let mut outbound = self.client.post(parts.uri.to_string()).body(body);
        for (name, value) in &parts.headers {
            outbound = outbound.header(name.as_str(), value.as_bytes());
        }
        outbound
            .send()
            .await
            .map(|response| response.status())
            .map_err(|_| web_push::WebPushError::Unspecified)
    }
}

#[async_trait]
impl PushSender for ProductionPushSender {
    async fn send(&self, job: &ClaimedPushJob, payload: &PushPayload) -> PushSendOutcome {
        match self.send_encrypted(job, payload).await {
            Ok(status) if status.is_success() => PushSendOutcome::Delivered,
            Ok(reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE) => {
                PushSendOutcome::Expired
            }
            Ok(reqwest::StatusCode::BAD_REQUEST) => PushSendOutcome::Expired,
            Ok(_) => PushSendOutcome::Retryable,
            Err(
                web_push::WebPushError::InvalidUri
                | web_push::WebPushError::MissingCryptoKeys
                | web_push::WebPushError::InvalidCryptoKeys,
            ) => PushSendOutcome::Expired,
            Err(_) => PushSendOutcome::Retryable,
        }
    }
}
