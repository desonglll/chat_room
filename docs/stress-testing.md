# Stress testing

The `stress` binary runs HTTP reads, authenticated WebSocket message round
trips, and identical-content attachment uploads at the same time. The upload
scenario deliberately shares one payload across workers so it also exercises
content-hash locking and physical-object deduplication.

Start the target server, then run the repository script with a host and port:

```sh
./scripts/stress-test \
  --host 127.0.0.1 \
  --port 3000 \
  --duration-secs 30 \
  --http-workers 24 \
  --websocket-workers 12 \
  --upload-workers 4 \
  --upload-bytes 65536
```

The tool creates a unique account and public room for every run. It prints
successful and failed operations, throughput, average latency, P50, P95, P99,
and maximum latency for each scenario. It also samples the run every second and
writes a self-contained HTML report with throughput, P95 latency, and error-rate
curves, plus the raw JSON and CSV data, to `stress-reports/`. Change the location
with `--report-dir` or the sampling interval with `--sample-interval-ms`.

It exits nonzero when the aggregate error rate exceeds `--max-error-rate`
(default `0.01`, meaning 1%). Use `0` in CI when no failed operation is
acceptable. `--base-url https://service.example` remains available and overrides
`--host`, `--port`, and `--scheme`.

Useful profiles:

```sh
# Quick smoke test
./scripts/stress-test --duration-secs 5 --http-workers 8 \
  --websocket-workers 4 --upload-workers 2 --max-error-rate 0

# WebSocket-heavy test without uploads
./scripts/stress-test --duration-secs 60 \
  --http-workers 4 --websocket-workers 100 --upload-workers 0

# Concurrent hash/deduplication pressure
./scripts/stress-test --duration-secs 30 \
  --http-workers 0 --websocket-workers 0 --upload-workers 24 \
  --upload-bytes 1048576
```

Run against an isolated database when possible. The generated messages and
logical attachment records are intentionally retained so post-run database,
dashboard, and storage metrics can be inspected.
