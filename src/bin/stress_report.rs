use std::{fmt::Write as _, fs, path::Path};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;

const SCENARIOS: [&str; 3] = ["HTTP", "WebSocket", "Upload"];
const COLORS: [&str; 3] = ["#0f8f76", "#e05d44", "#536dfe"];

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioSummary {
    pub name: String,
    pub succeeded: u64,
    pub failed: u64,
    pub operations_per_second: f64,
    pub average_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeriesPoint {
    pub elapsed_secs: f64,
    pub throughput: [f64; 3],
    pub p95_ms: [f64; 3],
    pub error_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct ReportMetadata {
    pub target: String,
    pub started_at: DateTime<Utc>,
    pub duration_secs: f64,
    pub http_workers: usize,
    pub websocket_workers: usize,
    pub upload_workers: usize,
    pub upload_bytes: usize,
    pub max_error_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct RunReport {
    pub metadata: ReportMetadata,
    pub scenarios: Vec<ScenarioSummary>,
    pub series: Vec<SeriesPoint>,
    pub aggregate_error_rate: f64,
}

pub struct ReportPaths {
    pub html: std::path::PathBuf,
    pub json: std::path::PathBuf,
    pub csv: std::path::PathBuf,
}

pub fn print_summary(elapsed: std::time::Duration, rows: &[ScenarioSummary]) {
    println!(
        "\nMixed stress test completed in {:.2}s",
        elapsed.as_secs_f64()
    );
    println!(
        "{:<11} {:>9} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Scenario", "Success", "Failed", "Ops/s", "Avg ms", "P50 ms", "P95 ms", "P99 ms", "Max ms"
    );
    for item in rows {
        println!(
            "{:<11} {:>9} {:>8} {:>10.1} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>10.2}",
            item.name,
            item.succeeded,
            item.failed,
            item.operations_per_second,
            item.average_ms,
            item.p50_ms,
            item.p95_ms,
            item.p99_ms,
            item.max_ms
        );
    }
}

pub fn write_report(directory: &Path, report: &RunReport) -> Result<ReportPaths> {
    fs::create_dir_all(directory)
        .with_context(|| format!("create report directory {}", directory.display()))?;
    let stem = format!(
        "stress-{}",
        report.metadata.started_at.format("%Y%m%d-%H%M%S")
    );
    let paths = ReportPaths {
        html: directory.join(format!("{stem}.html")),
        json: directory.join(format!("{stem}.json")),
        csv: directory.join(format!("{stem}.csv")),
    };
    fs::write(&paths.html, render_html(report))
        .with_context(|| format!("write HTML report {}", paths.html.display()))?;
    fs::write(&paths.json, serde_json::to_vec_pretty(report)?)
        .with_context(|| format!("write JSON report {}", paths.json.display()))?;
    fs::write(&paths.csv, render_csv(&report.series))
        .with_context(|| format!("write CSV report {}", paths.csv.display()))?;
    Ok(paths)
}

fn render_csv(points: &[SeriesPoint]) -> String {
    let mut output = String::from(
        "elapsed_secs,http_ops_s,websocket_ops_s,upload_ops_s,http_p95_ms,websocket_p95_ms,upload_p95_ms,error_rate\n",
    );
    for point in points {
        let _ = writeln!(
            output,
            "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.6}",
            point.elapsed_secs,
            point.throughput[0],
            point.throughput[1],
            point.throughput[2],
            point.p95_ms[0],
            point.p95_ms[1],
            point.p95_ms[2],
            point.error_rate
        );
    }
    output
}

fn render_html(report: &RunReport) -> String {
    let total_operations: u64 = report
        .scenarios
        .iter()
        .map(|row| row.succeeded + row.failed)
        .sum();
    let total_throughput: f64 = report
        .scenarios
        .iter()
        .map(|row| row.operations_per_second)
        .sum();
    let rows = report
        .scenarios
        .iter()
        .map(|row| {
            format!(
                "<tr><th>{}</th><td>{}</td><td>{}</td><td>{:.1}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td></tr>",
                escape_html(&row.name), row.succeeded, row.failed, row.operations_per_second, row.average_ms,
                row.p50_ms, row.p95_ms, row.p99_ms, row.max_ms
            )
        })
        .collect::<String>();
    let throughput_chart = multi_chart(
        "每秒吞吐量",
        &report.series,
        |point, index| point.throughput[index],
        "ops/s",
    );
    let latency_chart = multi_chart(
        "累计 P95 延迟",
        &report.series,
        |point, index| point.p95_ms[index],
        "ms",
    );
    let error_chart = single_chart(
        "区间错误率",
        &report.series,
        |point| point.error_rate * 100.0,
        "%",
        "#c23b3b",
    );
    let status = if report.aggregate_error_rate <= report.metadata.max_error_rate {
        "通过"
    } else {
        "未通过"
    };
    let template = r#"<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Chat Room 压力测试报告</title><style>
:root{color:#1e2927;background:#f5f8f7;font:14px/1.5 system-ui,sans-serif}body{margin:0}main{max-width:1120px;margin:auto;padding:32px 20px 56px}h1{margin:0;font-size:28px;letter-spacing:0}.meta{color:#667572;margin:6px 0 24px}.cards{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:12px}.card,.panel{border:1px solid #dce5e2;border-radius:6px;background:#fff}.card{padding:16px}.card span{display:block;color:#667572;font-size:12px}.card strong{display:block;margin-top:7px;font-size:22px}.panel{margin-top:16px;padding:18px;overflow:auto}h2{margin:0 0 14px;font-size:16px}table{width:100%;border-collapse:collapse;white-space:nowrap}th,td{padding:9px 12px;border-bottom:1px solid #edf1f0;text-align:right}th:first-child{text-align:left}thead th{color:#667572;font-size:12px}svg{display:block;width:100%;min-width:680px;height:auto}.legend{display:flex;gap:18px;margin-bottom:8px;color:#667572;font-size:12px}.legend i{display:inline-block;width:10px;height:3px;margin-right:6px;vertical-align:middle}.pass{color:#08765f}.fail{color:#b52e2e}@media(max-width:720px){.cards{grid-template-columns:repeat(2,minmax(0,1fr))}main{padding:20px 12px}}
</style></head><body><main>
<h1>Chat Room 压力测试报告</h1><p class="meta">{{TARGET}} · {{STARTED}} · 持续 {{DURATION}} 秒 · HTTP/WS/上传线程 {{WORKERS}}</p>
<section class="cards"><div class="card"><span>测试结论</span><strong class="{{STATUS_CLASS}}">{{STATUS}}</strong></div><div class="card"><span>总操作数</span><strong>{{TOTAL}}</strong></div><div class="card"><span>总吞吐量</span><strong>{{THROUGHPUT}}</strong></div><div class="card"><span>总错误率 / 阈值</span><strong>{{ERROR_RATE}}</strong></div></section>
<section class="panel"><h2>场景汇总</h2><table><thead><tr><th>场景</th><th>成功</th><th>失败</th><th>ops/s</th><th>平均 ms</th><th>P50 ms</th><th>P95 ms</th><th>P99 ms</th><th>最大 ms</th></tr></thead><tbody>{{ROWS}}</tbody></table></section>
<section class="panel">{{THROUGHPUT_CHART}}</section><section class="panel">{{LATENCY_CHART}}</section><section class="panel">{{ERROR_CHART}}</section>
</main></body></html>"#;
    template
        .replace("{{TARGET}}", &escape_html(&report.metadata.target))
        .replace("{{STARTED}}", &report.metadata.started_at.to_rfc3339())
        .replace(
            "{{DURATION}}",
            &format!("{:.2}", report.metadata.duration_secs),
        )
        .replace(
            "{{WORKERS}}",
            &format!(
                "{}/{}/{}",
                report.metadata.http_workers,
                report.metadata.websocket_workers,
                report.metadata.upload_workers
            ),
        )
        .replace(
            "{{STATUS_CLASS}}",
            if status == "通过" { "pass" } else { "fail" },
        )
        .replace("{{STATUS}}", status)
        .replace("{{TOTAL}}", &total_operations.to_string())
        .replace("{{THROUGHPUT}}", &format!("{total_throughput:.1} ops/s"))
        .replace(
            "{{ERROR_RATE}}",
            &format!(
                "{:.3}% / {:.3}%",
                report.aggregate_error_rate * 100.0,
                report.metadata.max_error_rate * 100.0
            ),
        )
        .replace("{{ROWS}}", &rows)
        .replace("{{THROUGHPUT_CHART}}", &throughput_chart)
        .replace("{{LATENCY_CHART}}", &latency_chart)
        .replace("{{ERROR_CHART}}", &error_chart)
}

fn multi_chart<F>(title: &str, points: &[SeriesPoint], value: F, unit: &str) -> String
where
    F: Fn(&SeriesPoint, usize) -> f64,
{
    let mut max_y = 1.0_f64;
    for point in points {
        for index in 0..3 {
            max_y = max_y.max(value(point, index));
        }
    }
    let mut body = chart_frame(title, max_y, unit);
    body.push_str("<div class=\"legend\">");
    for (name, color) in SCENARIOS.iter().zip(COLORS) {
        let _ = write!(
            body,
            "<span><i style=\"background:{}\"></i>{}</span>",
            color, name
        );
    }
    body.push_str("</div><svg viewBox=\"0 0 800 260\" role=\"img\">");
    body.push_str(&grid(max_y, unit));
    for (index, color) in COLORS.iter().enumerate() {
        body.push_str(&polyline(points, |point| value(point, index), max_y, color));
    }
    body.push_str("</svg>");
    body
}

fn single_chart<F>(title: &str, points: &[SeriesPoint], value: F, unit: &str, color: &str) -> String
where
    F: Fn(&SeriesPoint) -> f64,
{
    let max_y = points.iter().map(&value).fold(1.0_f64, f64::max);
    let mut body = chart_frame(title, max_y, unit);
    body.push_str("<svg viewBox=\"0 0 800 260\" role=\"img\">");
    body.push_str(&grid(max_y, unit));
    body.push_str(&polyline(points, value, max_y, color));
    body.push_str("</svg>");
    body
}

fn chart_frame(title: &str, max_y: f64, unit: &str) -> String {
    format!(
        "<h2>{} <small style=\"color:#667572;font-weight:400\">峰值 {:.2} {}</small></h2>",
        title, max_y, unit
    )
}

fn grid(max_y: f64, unit: &str) -> String {
    let mut output = String::new();
    for step in 0..=4 {
        let y = 220.0 - step as f64 * 50.0;
        let label = max_y * step as f64 / 4.0;
        let _ = write!(output, "<line x1=\"56\" y1=\"{y}\" x2=\"780\" y2=\"{y}\" stroke=\"#e7edeb\"/><text x=\"48\" y=\"{}\" text-anchor=\"end\" fill=\"#71807d\" font-size=\"11\">{label:.1}{unit}</text>", y + 4.0);
    }
    output
}

fn polyline<F>(points: &[SeriesPoint], value: F, max_y: f64, color: &str) -> String
where
    F: Fn(&SeriesPoint) -> f64,
{
    let max_x = points
        .last()
        .map(|point| point.elapsed_secs)
        .unwrap_or(1.0)
        .max(1.0);
    let coordinates = points
        .iter()
        .map(|point| {
            let x = 56.0 + point.elapsed_secs / max_x * 724.0;
            let y = 220.0 - value(point) / max_y * 200.0;
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("<polyline points=\"{coordinates}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2.5\" stroke-linejoin=\"round\"/>")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
