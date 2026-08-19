//! Reproducible mixed HTTP, WebSocket, and attachment load test.

use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::{bail, Context, Result};
use clap::Parser;
use reqwest::Client;
use tokio::{task::JoinSet, time::Instant};
use uuid::Uuid;

#[path = "stress_report.rs"]
mod stress_report;
#[path = "stress_scenarios.rs"]
mod stress_scenarios;

use stress_report::{write_report, ReportMetadata, RunReport, SeriesPoint};
use stress_scenarios::{
    prepare, run_http_worker, run_upload_worker, run_websocket_worker, websocket_url, Metric,
    UploadTarget,
};

#[derive(Debug, Parser)]
#[command(about = "Mixed chat-room HTTP, WebSocket, and attachment stress test")]
struct Args {
    /// Full target URL. Overrides --host, --port, and --scheme.
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 3000)]
    port: u16,
    #[arg(long, default_value = "http", value_parser = ["http", "https"])]
    scheme: String,
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
    /// Directory for the self-contained HTML, JSON, and CSV reports.
    #[arg(long, default_value = "stress-reports")]
    report_dir: PathBuf,
    #[arg(long, default_value_t = 1000)]
    sample_interval_ms: u64,
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
    if args.sample_interval_ms == 0 {
        bail!("--sample-interval-ms must be greater than zero");
    }
    if args.http_workers + args.websocket_workers + args.upload_workers == 0 {
        bail!("at least one worker must be enabled");
    }
    let base = target_url(&args)?;
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

    let series = collect_series(
        [&http, &websocket, &upload],
        started,
        deadline,
        Duration::from_millis(args.sample_interval_ms),
    )
    .await;

    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            eprintln!("worker task failed: {error}");
        }
    }
    let elapsed = started.elapsed();
    let snapshots = [
        http.snapshot("HTTP", elapsed),
        websocket.snapshot("WebSocket", elapsed),
        upload.snapshot("Upload", elapsed),
    ];
    stress_report::print_summary(elapsed, &snapshots);
    let succeeded: u64 = snapshots.iter().map(|item| item.succeeded).sum();
    let failed: u64 = snapshots.iter().map(|item| item.failed).sum();
    let error_rate = failed as f64 / (succeeded + failed).max(1) as f64;
    let report = RunReport {
        metadata: ReportMetadata {
            target: base,
            started_at: chrono::Utc::now() - chrono::Duration::from_std(elapsed)?,
            duration_secs: elapsed.as_secs_f64(),
            http_workers: args.http_workers,
            websocket_workers: args.websocket_workers,
            upload_workers: args.upload_workers,
            upload_bytes: args.upload_bytes,
            max_error_rate: args.max_error_rate,
        },
        scenarios: snapshots.to_vec(),
        series,
        aggregate_error_rate: error_rate,
    };
    let paths = write_report(&args.report_dir, &report)?;
    println!("\nHTML report: {}", paths.html.display());
    println!("JSON data:   {}", paths.json.display());
    println!("CSV series:  {}", paths.csv.display());
    if error_rate > args.max_error_rate {
        bail!(
            "error rate {:.3}% exceeded limit {:.3}%",
            error_rate * 100.0,
            args.max_error_rate * 100.0
        );
    }
    Ok(())
}

fn target_url(args: &Args) -> Result<String> {
    let target = if let Some(base_url) = &args.base_url {
        base_url.trim_end_matches('/').to_string()
    } else {
        let host = args.host.trim();
        if host.is_empty() || host.contains("//") || host.contains('/') {
            bail!("--host must be a hostname or IP address without a URL scheme or path");
        }
        let host = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host.to_string()
        };
        format!("{}://{}:{}", args.scheme, host, args.port)
    };
    if !target.starts_with("http://") && !target.starts_with("https://") {
        bail!("--base-url must start with http:// or https://");
    }
    Ok(target)
}

async fn collect_series(
    metrics: [&Arc<Metric>; 3],
    started: Instant,
    deadline: Instant,
    interval: Duration,
) -> Vec<SeriesPoint> {
    let mut points = Vec::new();
    let mut previous_counts = [(0_u64, 0_u64); 3];
    let mut previous_at = started;
    loop {
        let sample_at = (previous_at + interval).min(deadline);
        tokio::time::sleep_until(sample_at).await;
        let now = Instant::now();
        let period = now.duration_since(previous_at).as_secs_f64().max(0.001);
        let counts = metrics.map(|metric| metric.counts());
        let throughput = std::array::from_fn(|index| {
            let current = counts[index].0 + counts[index].1;
            let previous = previous_counts[index].0 + previous_counts[index].1;
            current.saturating_sub(previous) as f64 / period
        });
        let elapsed = now.duration_since(started);
        let summaries = [
            metrics[0].snapshot("HTTP", elapsed),
            metrics[1].snapshot("WebSocket", elapsed),
            metrics[2].snapshot("Upload", elapsed),
        ];
        let interval_succeeded: u64 = (0..3)
            .map(|index| counts[index].0.saturating_sub(previous_counts[index].0))
            .sum();
        let interval_failed: u64 = (0..3)
            .map(|index| counts[index].1.saturating_sub(previous_counts[index].1))
            .sum();
        points.push(SeriesPoint {
            elapsed_secs: elapsed.as_secs_f64(),
            throughput,
            p95_ms: summaries.map(|summary| summary.p95_ms),
            error_rate: interval_failed as f64
                / (interval_succeeded + interval_failed).max(1) as f64,
        });
        previous_counts = counts;
        previous_at = now;
        if now >= deadline {
            break;
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_accepts_host_and_port_or_a_complete_url() {
        let args = Args::try_parse_from(["stress", "--host", "10.0.0.5", "--port", "8088"])
            .expect("host and port arguments");
        assert_eq!(target_url(&args).unwrap(), "http://10.0.0.5:8088");

        let args = Args::try_parse_from(["stress", "--base-url", "https://chat.example.test/"])
            .expect("base URL argument");
        assert_eq!(target_url(&args).unwrap(), "https://chat.example.test");
    }

    #[test]
    fn report_writer_creates_chart_and_raw_data_files() {
        let directory = std::env::temp_dir().join(format!("stress-report-{}", Uuid::new_v4()));
        let report = RunReport {
            metadata: ReportMetadata {
                target: "http://127.0.0.1:3000".into(),
                started_at: chrono::Utc::now(),
                duration_secs: 1.0,
                http_workers: 1,
                websocket_workers: 1,
                upload_workers: 1,
                upload_bytes: 1024,
                max_error_rate: 0.01,
            },
            scenarios: vec![Metric::default().snapshot("HTTP", Duration::from_secs(1))],
            series: vec![SeriesPoint {
                elapsed_secs: 1.0,
                throughput: [10.0, 2.0, 1.0],
                p95_ms: [3.0, 5.0, 8.0],
                error_rate: 0.0,
            }],
            aggregate_error_rate: 0.0,
        };

        let paths = write_report(&directory, &report).unwrap();
        let html = std::fs::read_to_string(paths.html).unwrap();
        let csv = std::fs::read_to_string(paths.csv).unwrap();
        assert!(html.contains("每秒吞吐量") && html.contains("<polyline"));
        assert!(csv.contains("http_ops_s") && csv.contains("10.000"));
        assert!(paths.json.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
