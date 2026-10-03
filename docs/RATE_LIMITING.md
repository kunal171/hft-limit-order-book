# Rate Limiting

Signup and login each run an Argon2 hash (about 25 ms of CPU). Without a limit,
one client can flood these routes to exhaust the server, or guess passwords as
fast as the machine allows. This document records the current design, its
known limits, and the decision about when to move it out of process.

## Current Design

Code: `src/api/rate_limit.rs`. State: `AppState::auth_rate_limiter`.

Status: applied to `/auth/signup` and `/auth/login`. The server is started with
connect info in `src/bin/api.rs`, and `repeated_login_attempts_are_rate_limited`
in `tests/auth_api.rs` covers it end to end. `/auth/logout` and `/auth/me` are
not limited.

Algorithm: token bucket per client IP address.

```text
burst              10 requests
refill             0.2 tokens per second (one request every 5 seconds)
over the limit     429 Too Many Requests
client key         peer IP from ConnectInfo<SocketAddr>
no peer address    one shared bucket (requests sent straight into the router,
                   as integration tests do)
```

A person who mistypes a password a few times is never blocked. A script is
held to about 12 attempts a minute.

Rejections are counted in `lob_auth_rate_limited_total`. Rejected requests
never reach the login handler, so they do not appear in
`lob_auth_login_attempts_total`.

Implementation choices:

```text
lazy refill        no timers; each request adds elapsed * rate, capped at burst
injected clock     try_acquire(client, now) takes `now`, so tests move time
                   without sleeping
std Mutex          held for a few arithmetic operations, never across .await
bounded memory     above 10,000 tracked clients, fully refilled buckets are
                   dropped (a full bucket is the same as no bucket)
configurable       AppState::with_auth_rate_limit(db, config); tests use a
                   zero refill so the result does not depend on test speed
```

## Known Limits

```text
restart            buckets live in process memory, so a restart gives every
                   client one fresh burst
several instances  each process keeps its own buckets; N instances behind a
                   load balancer allow roughly N times the limit
reverse proxy      behind a proxy every client appears as the proxy's IP, so
                   all clients would share one bucket
one account        many IPs guessing one account's password are not slowed;
                   that needs a second limit keyed by email
```

The restart gap is small: an attacker cannot trigger restarts, and one extra
burst per restart is a few hundred guesses a day at most. The
several-instances gap is the one that matters.

## Decision: When To Move Out Of Process

Keep the in-memory limiter while the API runs as a single process.

Move to Redis when either of these becomes true:

```text
the API runs as more than one process
Redis is added for the session cache (see AUTHENTICATION.md)
```

Redis gives every instance one shared counter. Surviving restarts is a side
effect, not the main reason.

What the Redis version must handle:

```text
atomicity          read, compute, write back must be one atomic step: a Lua
                   script for the token bucket, or INCR plus EXPIRE for a
                   simpler fixed window
outage policy      decide explicitly: fail open (no protection while Redis is
                   down) or fail closed (nobody can sign in); record the choice
latency            one network round trip per limited request, roughly
                   0.1-1 ms; small next to Argon2 but measure it
operations         one more service to run, secure, and monitor
```

Keep the swap cheap: the middleware only calls `try_acquire`, so the storage
behind it can change without touching routes or tests that build `AppState`.

## Edge Limiting

In a real deployment, a reverse proxy or load balancer (nginx `limit_req`,
Caddy, a cloud WAF) should throttle floods before they reach the API. The
in-app limit stays, because only the app knows what a login attempt is and,
later, which account it targets.

When the API moves behind a proxy, change the client key to the forwarded
client IP, and accept that header only from the trusted proxy's address.

## Follow-Up List

```text
per-email limit on login, alongside the per-IP limit
Retry-After header on 429 responses
Redis-backed limiter when the API runs as more than one process
forwarded-IP client key once deployed behind a trusted proxy
```
