# Ripple Knowledge Service

The Knowledge Service is the shared LAN backend for Ripple knowledge sources. It is intentionally separate from the Tauri desktop process and from the legacy local `ripple.db` database.

## Current service API

Health endpoints remain unauthenticated:

- `GET /health/live` — process liveness; independent of PostgreSQL.
- `GET /health/ready` — PostgreSQL, pgvector, and migration-ledger readiness.
- `GET /health/dependencies` — sanitized dependency states.

All non-health routes are versioned beneath `/api/v1`:

- `POST /bootstrap` — one-time initial server-admin creation; requires the startup bootstrap secret in the request body and permanently consumes it after success.
- `POST /auth/login`, `POST /auth/refresh`, `POST /auth/logout`, `GET /auth/me` — opaque access/refresh session lifecycle.
- `POST /users` — server-admin-only local user creation.
- `POST`/`GET /collections` and collection membership routes — authenticated collection ACL administration.

Credentials are random opaque values. The service persists only SHA-256 digests for access and refresh credentials and Argon2id hashes for passwords. Credentials must not be copied into logs, support bundles, shell history, or source control.

Every response includes `x-request-id`. Errors use a versioned JSON envelope and deliberately omit database URLs, credentials, SQL errors, source text, prompts, and provider output.

Upload, folder-sync, URL-import, retrieval, and desktop login integration remain future phases.

## Local development prerequisites

- Rust toolchain used by the Tauri workspace.
- PostgreSQL 15 or newer with the `vector` extension installed in an isolated development database.
- An absolute data-root path controlled by the service.

The service does not use the desktop SQLite database, desktop API-key storage, or desktop log/debug configuration.

```powershell
./scripts/knowledge-service/run-dev.ps1 `
  -DatabaseUrl 'postgres://knowledge_user:password@127.0.0.1:5432/ripple_knowledge' `
  -BootstrapToken '<generate-and-store-outside-the-repository>' `
  -DataRoot 'D:\RippleKnowledgeData'
```

The PowerShell helper sets:

- `RIPPLE_KNOWLEDGE_DATABASE_URL`
- `RIPPLE_KNOWLEDGE_DATA_ROOT`
- `RIPPLE_KNOWLEDGE_LISTEN_ADDR` (defaults to `127.0.0.1:8787`)
- `RIPPLE_KNOWLEDGE_MAX_CONNECTIONS` (defaults to `5`)

It is loopback-only by default. A LAN listener must be explicitly supplied through `RIPPLE_KNOWLEDGE_LISTEN_ADDR`; production TLS, Windows Service installation, firewall policy, accounts, and collection ACLs are added before any LAN deployment is supported.

## Isolated auth/ACL integration verification

On a Docker-capable development host, run:

```powershell
./scripts/knowledge-service/test-auth-acl.ps1
```

The script uses [docker-compose.knowledge-test.yml](../docker-compose.knowledge-test.yml) to create a disposable pgvector database, destroys its container/volume and temporary service data in `finally`, and never reads `ripple.db` or a host PostgreSQL database. It exits with a visible skip when Docker is unavailable; it never falls back to real user data.

