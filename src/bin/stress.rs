//! Reproducible mixed HTTP, WebSocket, and attachment load test.

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use reqwest::{multipart, Client};
use serde_json::{json, Value};
use tokio::{task::JoinSet, time::Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(about = "Mixed chat-room HTTP, WebSocket, and attachment stress test")]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    base_url: String,
    #[arg(long, default_value_t = 30)]
    duration_secs: u64,
    #[arg(long, default_value_t = 24)]
    http_workers: usize,
    #[arg(long, default_value_t = 12)]
    websocket_workers: usize,
    #[arg(long, default_value_t = 4)]
    upload_workers: usize,
    #[arg(long, default_value_t = 64 * 1024)]
    upload_bytes: usize,
    #[arg(long, default_value_t = 20)]
    websocket_interval_ms: u64,
    /// Maximum accepted failed-operation fraction, from 0.0 to 1.0.
    #[arg(long, default_value_t = 0.01)]
    max_error_rate: f64,
}

#[derive(Default)]
struct Metric {
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

    fn snapshot(&self, elapsed: Duration) -> Snapshot {
        let succeeded = self.succeeded.load(Ordering::Relaxed);
        let failed = self.failed.load(Ordering::Relaxed);
        let mut samples = self.samples.lock().expect("metric samples mutex").clone();
        samples.sort_unstable();
        Snapshot {
            succeeded,
            failed,
            operations_per_second: succeeded as f64 / elapsed.as_secs_f64(),
            average_ms: if succeeded == 0 {
                0.0
            } else {
                self.total_micros.load(Ordering::Relaxed) as f64 / succeeded as f64 / 1000.0
            },
            p95_ms: percentile(&samples, 0.95),
            p99_ms: percentile(&samples, 0.99),
            max_ms: self.max_micros.load(Ordering::Relaxed) as f64 / 1000.0,
        }
    }
}

struct Snapshot {
    succeeded: u64,
    failed: u64,
    operations_per_second: f64,
    average_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    max_ms: f64,
}

struct UploadTarget {
    client: Client,
    base: String,
    token: String,
    room_id: String,
    payload: Vec<u8>,
}

fn percentile(samples: &[u64], percentile: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let index = ((samples.len() - 1) as f64 * percentile).ceil() as usize;
    samples[index] as f64 / 1000.0
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.duration_secs == 0 {
        bail!("--duration-secs must be greater than zero");
    }
    if !(0.0..=1.0).contains(&args.max_error_rate) {
        bail!("--max-error-rate must be between 0.0 and 1.0");
    }
    let base = args.base_url.trim_end_matches('/').to_string();
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("build HTTP client")?;
    let run_id = Uuid::new_v4().simple().to_string();
    let (token, room_id) = prepare(&client, &base, &run_id).await?;
    let http = Arc::new(Metric::default());
    let websocket = Arc::new(Metric::default());
    let upload = Arc::new(Metric::default());
    let started = Instant::now();
    let deadline = started + Duration::from_secs(args.duration_secs);
    let mut tasks = JoinSet::new();

    for worker in 0..args.http_workers {
        tasks.spawn(run_http_worker(
            client.clone(),
            base.clone(),
            token.clone(),
            worker,
            deadline,
            http.clone(),
        ));
    }
    for worker in 0..args.websocket_workers {
        tasks.spawn(run_websocket_worker(
            websocket_url(&base, &room_id)?,
            token.clone(),
            run_id.clone(),
            worker,
            args.websocket_interval_ms,
            deadline,
            websocket.clone(),
        ));
    }
    let upload_target = Arc::new(UploadTarget {
        client: client.clone(),
        base: base.clone(),
        token: token.clone(),
        room_id: room_id.clone(),
        payload: vec![0x5a; args.upload_bytes],
    });
    for worker in 0..args.upload_workers {
        tasks.spawn(run_upload_worker(
            upload_target.clone(),
            worker,
            deadline,
            upload.clone(),
        ));
    }

    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            eprintln!("worker task failed: {error}");
        }
    }
    let elapsed = started.elapsed();
    let snapshots = [
        ("HTTP", http.snapshot(elapsed)),
        ("WebSocket", websocket.snapshot(elapsed)),
        ("Upload", upload.snapshot(elapsed)),
    ];
    print_report(elapsed, &snapshots);
    let succeeded: u64 = snapshots.iter().map(|(_, item)| item.succeeded).sum();
    let failed: u64 = snapshots.iter().map(|(_, item)| item.failed).sum();
    let error_rate = failed as f64 / (succeeded + failed).max(1) as f64;
    if error_rate > args.max_error_rate {
        bail!(
            "error rate {:.3}% exceeded limit {:.3}%",
            error_rate * 100.0,
            args.max_error_rate * 100.0
        );
    }
    Ok(())
}

async fn prepare(client: &Client, base: &str, run_id: &str) -> Result<(String, String)> {
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

async fn run_http_worker(
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

async fn run_websocket_worker(
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

async fn run_upload_worker(
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

fn websocket_url(base: &str, room_id: &str) -> Result<String> {
    if let Some(rest) = base.strip_prefix("https://") {
        Ok(format!("wss://{rest}/ws/{room_id}"))
    } else if let Some(rest) = base.strip_prefix("http://") {
        Ok(format!("ws://{rest}/ws/{room_id}"))
    } else {
        bail!("--base-url must start with http:// or https://")
    }
}

fn print_report(elapsed: Duration, rows: &[(&str, Snapshot)]) {
    println!(
        "\nMixed stress test completed in {:.2}s",
        elapsed.as_secs_f64()
    );
    println!(
        "{:<11} {:>9} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Scenario", "Success", "Failed", "Ops/s", "Avg ms", "P95 ms", "P99 ms", "Max ms"
    );
    for (name, item) in rows {
        println!(
            "{name:<11} {:>9} {:>8} {:>10.1} {:>10.2} {:>10.2} {:>10.2} {:>10.2}",
            item.succeeded,
            item.failed,
            item.operations_per_second,
            item.average_ms,
            item.p95_ms,
            item.p99_ms,
            item.max_ms
        );
    }
}
