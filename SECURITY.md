# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| `main` branch | ✅ Active |
| Older releases | ❌ No support |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Report security issues by emailing **security@stellarclassic-pulse.io** with:

1. A clear description of the vulnerability
2. Steps to reproduce
3. Potential impact assessment
4. Any suggested mitigations (optional)

You will receive an acknowledgement within **48 hours** and a full response
within **7 days** outlining next steps. If you have not heard back within
48 hours, please follow up to ensure we received your report.

## Disclosure Policy

- We follow responsible disclosure. Please give us **90 days** to patch before
  public disclosure.
- We will credit reporters in the release notes unless you prefer anonymity.
- We do not operate a bug bounty program at this time.

## Scope

The following are in scope:

- `stellarclassic-pulse` Rust binary and library (`src/`)
- Authentication and authorization middleware (`src/middleware/auth.rs`)
- Webhook signing and verification (`src/webhook_signing.rs`, `src/webhook_verification.rs`)
- Encryption and key management (`src/encryption.rs`, `src/reencrypt.rs`)
- API endpoints exposed on port 3000
- Docker image and container configuration

Out of scope:

- Third-party dependencies (report to the upstream maintainer)
- Issues in the `SorobanPulse/` legacy directory
- Social engineering attacks

## Security Best Practices for Deployments

- Always set `WEBHOOK_SECRET`, `ENCRYPTION_KEY`, and `JWT_SECRET` via
  environment variables — never hardcode them.
- Run the container as the non-root `stellarclassic` user (already the default
  in the provided `Dockerfile`).
- Restrict database access to the application network only.
- Enable TLS termination at your load balancer or reverse proxy.
- Rotate keys using the `reencrypt` module before they exceed 90 days.

## Contact

**Email:** security@stellarclassic-pulse.io
**PGP:** Available on request.
