# Contributing to Soroban Pulse

Thanks for your interest in contributing! This document covers everything you need to get started.

## Development Setup

### Prerequisites

- Rust (stable) — install via [rustup](https://rustup.rs/)
- PostgreSQL 14+
- Docker + Docker Compose (optional, easiest path)

### Local setup

```bash
git clone https://github.com/Soroban-Pulse/SorobanPulse.git
cd SorobanPulse
cp .env.example .env
# Edit .env and fill in your DATABASE_URL and other values
cargo build
```

### Run with Docker Compose

```bash
docker-compose up --build
```

This starts PostgreSQL and the service together. Migrations run automatically on startup.

### Run locally (without Docker)

```bash
# Start a local PostgreSQL instance, then:
cargo run
```

## Running Tests

```bash
cargo test
```

Integration tests use `sqlx::test` and spin up a real PostgreSQL instance. Make sure `DATABASE_URL` is set in your environment or `.env`.

## Branch Naming

| Prefix    | When to use                              |
|-----------|------------------------------------------|
| `feat/`   | New features                             |
| `fix/`    | Bug fixes                                |
| `chore/`  | Maintenance, dependency updates, tooling |
| `docs/`   | Documentation-only changes               |
| `refactor/` | Code changes with no behaviour change  |

Examples: `feat/ledger-range-filter`, `fix/pagination-offset`, `docs/update-readme`

## Commit Message Format

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<optional scope>): <short description> (#<issue>)

[optional body]
```

Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `perf`

Examples:
```
feat(api): add event_type filter to GET /events (#81)
fix(indexer): handle empty ledger range gracefully (#77)
docs: add RUST_LOG troubleshooting note (#79)
```

## Pull Request Process

1. Fork the repo and create your branch from `main`.
2. Make your changes with focused, atomic commits.
3. Ensure `cargo test` passes and `cargo clippy` reports no warnings.
4. Open a PR against `main` — the PR template will guide you through the required fields.
5. A maintainer will review and merge once approved.

## Code Style

- Run `cargo fmt` before committing.
- Address all `cargo clippy` warnings.
- Keep functions small and focused; avoid unnecessary abstractions.
