# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| `main`  | ✅        |

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Please report security issues by emailing **security@ghaaf.org**. Include:

- A description of the vulnerability and its potential impact
- Steps to reproduce or a proof-of-concept
- Any suggested mitigations

You will receive an acknowledgement within 48 hours and a resolution timeline within 7 days. We follow responsible disclosure — please give us reasonable time to address the issue before any public disclosure.

## Scope

In scope:

- Authentication and session management (`apps/api/src/middleware/auth.rs`)
- Authorization / IDOR vulnerabilities in API handlers
- Wallet key handling and Circle API integration
- Cross-chain execution logic

Out of scope:

- Denial-of-service attacks
- Issues in third-party dependencies (report upstream)
- Issues requiring physical access to the server

## Security Architecture

- Sessions are opaque UUIDs stored server-side; no JWTs are issued to clients
- All state-changing API requests require `X-Aegis-Request: 1` (CSRF protection)
- Wallet private keys are injected via environment variables, never persisted to disk or logged
- All database queries use parameterized statements (sqlx `bind()`)
- Secrets are managed via Infisical; the repo contains only placeholder `.env` values
