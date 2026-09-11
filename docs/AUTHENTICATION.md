# Authentication And Session Design

Authentication belongs to the HTTP control plane. It must never add database,
Redis, password hashing, or network work to the matching engine's hot path.

## Current Implementation

```text
users                 identity, status, and role
user_credentials      one Argon2 password hash per login-capable user
sessions              hashed bearer token, expiry, and revocation state
```

Implemented endpoints and tools:

```text
cargo run --bin bootstrap_admin   creates the first administrator
POST /auth/signup                 creates an active trader and credentials
POST /auth/login                  verifies credentials and creates a session
GET  /auth/me                     returns the authenticated user and role
POST /auth/logout                 idempotently revokes the supplied session
POST /admin/users                 requires an authenticated administrator
```

Public signup never accepts a role. The SQL statement assigns `trader`, and
Serde rejects unknown request fields. This prevents a caller from submitting
`"role":"admin"` and escalating privileges.

The administrator bootstrap hashes the password before opening its database
transaction. It locks the users table while checking for an existing admin so
simultaneous bootstrap attempts cannot create multiple initial administrators.

## Password Handling

Passwords are processed with Argon2 and a random salt. The original password is
never logged, returned, or stored. Argon2 runs through `spawn_blocking` in HTTP
handlers because it is intentionally CPU-intensive and must not block Tokio's
asynchronous worker threads.

Signup inserts `users` and `user_credentials` in one PostgreSQL transaction:

```text
validate and normalize input
-> hash password outside transaction
-> begin transaction
-> insert active trader
-> insert credentials
-> commit
```

If either insert fails, neither record remains.

## Login And Session Flow

```text
normalize email
-> load user and password hash
-> verify Argon2 password
-> reject inactive account
-> generate 32 random bytes
-> encode as URL-safe Base64
-> hash the encoded token with SHA-256
-> insert hash and 12-hour expiry into sessions
-> return the raw token once
```

Passwords need slow hashing because humans choose guessable values. Session
tokens contain 256 bits of operating-system randomness, so SHA-256 is suitable:
it hides the usable token while allowing fast indexed lookup on every request.

Unknown emails and incorrect passwords return the same `401 Unauthorized`
response to avoid directly revealing account existence. Valid credentials for
an inactive user return `403 Forbidden`.

## Verified Behavior

The local PostgreSQL-backed flow has been exercised with these results:

```text
signup                 201 Created
valid login            200 OK
incorrect password     401 Unauthorized
valid bearer session   200 OK
expired session        401 Unauthorized
revoked session        401 Unauthorized
suspended user         403 Forbidden
trader on admin route  403 Forbidden
admin on admin route   request reaches handler
repeated logout        204 No Content
stored token digest    32 bytes
raw token in database  no
```

Five bearer-parser unit tests and five isolated PostgreSQL integration tests
cover these rules. SQLx creates a temporary migrated database for every
integration test.

## Bearer Middleware And Authorization

Protected requests use this header:

```text
Authorization: Bearer <raw-token>
```

Middleware performs this flow:

```text
parse the header
-> SHA-256 hash the supplied token
-> join sessions with users by user_id
-> require revoked_at IS NULL
-> require expires_at > now()
-> require users.status = active
-> attach authenticated user id and role to request extensions
```

Protected handlers must trust only the authenticated request extension. They
must never trust a user ID or role supplied in JSON or custom headers.

Routes under `/admin/*` apply a second middleware layer that requires the role
loaded from PostgreSQL to be `admin`. The temporary `x-admin-api-key` mechanism
has been removed.

## Logout And Revocation

Logout sets `revoked_at` in PostgreSQL rather than deleting the session.
This keeps a useful audit trail and makes repeated logout requests idempotent.
Administrative account suspension must also make existing sessions unusable.

Expired and revoked sessions can be deleted later by a bounded maintenance job.
Cleanup is not part of request authentication.

## Optional Redis Cache

PostgreSQL remains the source of truth. Redis may be added after middleware is
correct and measurements show that session lookup needs acceleration.

```text
request -> hash token -> Redis lookup
                         | hit: validate cached session
                         | miss/error: query PostgreSQL
                                       -> authorize
                                       -> repopulate Redis when available
```

Rules:

```text
cache key       session:<SHA-256 token hash>
cache value     user_id, role, status/auth version, expiry
cache TTL       remaining PostgreSQL session lifetime
Redis outage    fall back to PostgreSQL
logout          revoke in PostgreSQL, then delete Redis key
role/status     invalidate affected cached sessions
```

Redis is an acceleration layer, never the authority. A cache write failure must
not fail a login whose PostgreSQL transaction succeeded. This Redis instance is
also separate from the matching engine: matching continues to own its book in
local Rust memory.

## Remaining Security Work

```text
login rate limiting
dummy password verification for unknown-email timing resistance
TLS at the deployment boundary
secret management outside local .env files
session and security audit events
integration tests for duplicate signup and invalid credentials
optional Redis cache with PostgreSQL fallback
```
