use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use reqwest::{multipart, Client};
use serde_json::{json, Value};
use tokio::time::Instant;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::stress_report::ScenarioSummary;

#[derive(Default)]
pub struct Metric {
    succeeded: AtomicU64,
    failed: AtomicU64,
    total_micros: AtomicU64,
    max_micros: AtomicU64,
    samples: Mutex<Vec<u64>>,
}

impl Metric {
    fn success(&self, started: Instant) {
        let micros = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        self.succeeded.fetch_add(1, Ordering::Relaxed);
        self.total_micros.fetch_add(micros, Ordering::Relaxed);
        self.max_micros.fetch_max(micros, Ordering::Relaxed);
        let mut samples = self.samples.lock().expect("metric samples mutex");
        if samples.len() < 200_000 {
            samples.push(micros);
        }
    }

    fn failure(&self) {
        self.failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts(&self) -> (u64, u64) {
        (
            self.succeeded.load(Ordering::Relaxed),
            self.failed.load(Ordering::Relaxed),
        )
    }

    pub fn snapshot(&self, name: &str, elapsed: Duration) -> ScenarioSummary {
        let succeeded = self.succeeded.load(Ordering::Relaxed);
        let failed = self.failed.load(Ordering::Relaxed);
        let mut samples = self.samples.lock().expect("metric samples mutex").clone();
        samples.sort_unstable();
        ScenarioSummary {
            name: name.into(),
            succeeded,
            failed,
            operations_per_second: (succeeded + failed) as f64 / elapsed.as_secs_f64(),
            average_ms: if succeeded == 0 {
                0.0
            } else {
                self.total_micros.load(Ordering::Relaxed) as f64 / succeeded as f64 / 1000.0
            },
            p50_ms: percentile(&samples, 0.50),
            p95_ms: percentile(&samples, 0.95),
            p99_ms: percentile(&samples, 0.99),
            max_ms: self.max_micros.load(Ordering::Relaxed) as f64 / 1000.0,
        }
    }
}

pub struct UploadTarget {
    pub client: Client,
    pub base: String,
    pub token: String,
    pub room_id: String,
    pub payload: Vec<u8>,
}

fn percentile(samples: &[u64], percentile: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let index = ((samples.len() - 1) as f64 * percentile).ceil() as usize;
    samples[index] as f64 / 1000.0
}

pub async fn prepare(client: &Client, base: &str, run_id: &str) -> Result<(String, String)> {
    let username = format!("stress-{}", &run_id[..12]);
    let response = client
        .post(format!("{base}/api/users/register"))
        .json(&json!({ "username": username, "password": "stress-test-password" }))
        .send()
        .await
        .context("register stress-test account")?;
    let status = response.status();
    let session: Value = response
        .json()
        .await
        .context("decode registration response")?;
    if !status.is_success() {
        bail!("registration returned {status}: {session}");
    }
    let token = session["token"]
        .as_str()
        .context("registration response has no token")?
        .to_string();
    let response = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&token)
        .json(&json!({
            "name": format!("stress-{}", &run_id[..12]),
            "password": "",
            "join_policy": "open"
        }))
        .send()
        .await
        .context("create stress-test room")?;
    let status = response.status();
    let room: Value = response.json().await.context("decode room response")?;
    if !status.is_success() {
        bail!("room creation returned {status}: {room}");
    }
    let room_id = room["id"]
        .as_str()
        .context("room response has no id")?
        .to_string();
    Ok((token, room_id))
}

pub async fn run_http_worker(
    client: Client,
    base: String,
    token: String,
    worker: usize,
    deadline: Instant,
    metric: Arc<Metric>,
) {
    let mut sequence = worker;
    while Instant::now() < deadline {
        let path = if sequence.is_multiple_of(2) {
            "/api/config"
        } else {
            "/api/rooms"
        };
        let started = Instant::now();
        match client
            .get(format!("{base}{path}"))
            .bearer_auth(&token)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => metric.success(started),
            _ => metric.failure(),
        }
        sequence += 1;
    }
}

pub async fn run_websocket_worker(
    url: String,
    token: String,
    run_id: String,
    worker: usize,
    interval_ms: u64,
    deadline: Instant,
    metric: Arc<Metric>,
) {
    let Ok((mut socket, _)) = connect_async(&url).await else {
        metric.failure();
        return;
    };
    if socket
        .send(Message::Text(
            json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .is_err()
        || !wait_for_type(&mut socket, "auth_ok").await
    {
        metric.failure();
        return;
    }
    let mut sequence = 0_u64;
    while Instant::now() < deadline {
        let content = format!("stress-{run_id}-{worker}-{sequence}");
        let started = Instant::now();
        if socket
            .send(Message::Text(
                json!({ "type": "message", "content": content }).to_string(),
            ))
            .await
            .is_err()
        {
            metric.failure();
            break;
        }
        if wait_for_broadcast(&mut socket, &content, deadline).await {
            metric.success(started);
        } else if Instant::now() >= deadline {
            break;
        } else {
            metric.failure();
            break;
        }
        sequence += 1;
        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }
    let _ = socket.close(None).await;
}

async fn wait_for_type<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    expected: &str,
) -> bool
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(frame) = socket.next().await {
            if let Ok(Message::Text(text)) = frame {
                let body: Value = serde_json::from_str(&text).ok()?;
                if body["type"] == expected {
                    return Some(());
                }
                if body["type"] == "auth_fail" {
                    return None;
                }
            }
        }
        None
    })
    .await;
    matches!(result, Ok(Some(())))
}

async fn wait_for_broadcast<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    content: &str,
    deadline: Instant,
) -> bool
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let remaining = deadline.saturating_duration_since(Instant::now());
    let timeout = remaining.min(Duration::from_secs(5));
    let result = tokio::time::timeout(timeout, async {
        while let Some(frame) = socket.next().await {
            match frame {
                Ok(Message::Text(text)) => {
                    let body: Value = serde_json::from_str(&text).ok()?;
                    if body["type"] == "broadcast" && body["content"] == content {
                        return Some(());
                    }
                }
                Ok(Message::Close(_)) | Err(_) => return None,
                _ => {}
            }
        }
        None
    })
    .await;
    matches!(result, Ok(Some(())))
}

pub async fn run_upload_worker(
    target: Arc<UploadTarget>,
    worker: usize,
    deadline: Instant,
    metric: Arc<Metric>,
) {
    let mut sequence = 0_u64;
    while Instant::now() < deadline {
        let part = match multipart::Part::bytes(target.payload.clone())
            .file_name(format!("stress-{worker}-{sequence}.bin"))
            .mime_str("application/octet-stream")
        {
            Ok(part) => part,
            Err(_) => {
                metric.failure();
                return;
            }
        };
        let started = Instant::now();
        let response = target
            .client
            .post(format!(
                "{}/api/rooms/{}/attachments",
                target.base, target.room_id
            ))
            .bearer_auth(&target.token)
            .multipart(multipart::Form::new().part("file", part))
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => metric.success(started),
            _ => metric.failure(),
        }
        sequence += 1;
    }
}

pub fn websocket_url(base: &str, room_id: &str) -> Result<String> {
    if let Some(rest) = base.strip_prefix("https://") {
        Ok(format!("wss://{rest}/ws/{room_id}"))
    } else if let Some(rest) = base.strip_prefix("http://") {
        Ok(format!("ws://{rest}/ws/{room_id}"))
    } else {
        bail!("--base-url must start with http:// or https://")
    }
}
