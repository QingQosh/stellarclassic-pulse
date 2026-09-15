# StellarClassicPulse Backend

The Rust core of StellarClassicPulse — a real-time event streaming and observability platform for Soroban smart contracts on the Stellar blockchain.

## What's in here

| Path | Purpose |
|---|---|
| `src/` | Rust library + binary (Axum HTTP server, indexer, all subsystems) |
| `migrations/` | SQLx PostgreSQL migration files |
| `benches/` | Criterion benchmarks |
| `tests/` | Integration and unit tests |
| `fuzz/` | Fuzz targets |
| `examples/` | Runnable Rust examples |
| `bin/` | Top-level shell/utility scripts |
| `notification_templates/` | Templates used by the notification delivery pipeline |
| `k8s/` | Kubernetes manifests |
| `helm/` | Helm charts |
| `terraform/` | Terraform infrastructure code |
| `gitops/` | GitOps deployment configuration |
| `edge/` | Edge deployment configuration |
| `Dockerfile` | Multi-stage cargo-chef build → minimal Debian image |
| `docker-compose.yml` | Local dev stack (Postgres, app, Prometheus, Grafana, Redis) |

## Related repo

Client tooling (dashboard, CLI, SDKs, VS Code extension) lives in **[stellarclassicpulse-clients](../stellarclassicpulse-clients)**.

The dashboard and CLI both connect to this backend over HTTP (default port **3000**).

## Quick start

```bash
# Copy and fill in environment variables
cp .env.example .env

# Start the full local stack (requires Docker)
docker compose up

# Or run the binary directly (requires a running Postgres)
cargo run --release
```

## Building

```bash
# Check the code compiles
cargo check

# Full release build
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench
```

## Architecture

Data pipeline: `Stellar Soroban RPC → indexer (Tokio task) → PostgreSQL → REST / SSE / Webhooks → clients`

Key feature flags (passed via `--features`):

| Flag | Enables |
|---|---|
| `kafka` | Kafka producer |
| `sqs` | AWS SQS |
| `pubsub` | Google Cloud Pub/Sub |
| `kinesis` | AWS Kinesis |
| `graphql` | async-graphql endpoint |
| `quantum-crypto` | Post-quantum cryptography |
| `full-observability` | Jaeger / OpenTelemetry tracing |

Health check endpoint: `GET /healthz/ready`
Metrics endpoint: `GET /metrics` (Prometheus format)

## Environment variables

See `.env.example`, `.env.staging.example`, and `.env.production.example` for the full list of supported configuration variables.
