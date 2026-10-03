# HFT-Style Limit Order Book

A Rust limit order book and market-systems lab: deterministic price-time
matching, replayable state, measured latency, and a PostgreSQL-backed trading
control plane.

This is a learning project, not production exchange infrastructure. The design
goal is a single-writer matching engine that owns the live book in memory, with
networking, databases, and analytics kept outside the matching hot path.

## Status

| Part | State |
|---|---|
| Matching engine (add, cancel, modify, events, replay) | Done |
| Simulator, metrics, Criterion benchmarks | Done |
| HTTP API: auth, sessions, accounts, admin instruments | Done |
| Observability: Prometheus metrics | In progress |
| API connected to the engine (order entry) | Not started |

The engine and the API are currently two separate halves. Wiring them together
through a sequenced single-writer runtime is the next major phase; see
[docs/HFT_ROADMAP.md](docs/HFT_ROADMAP.md).

## Layout

```text
src/
  domain/         Order, Trade, BookEvent, BookSnapshot, Side, integer Price/Quantity
  engine/         OrderBook API (order_book.rs) and price-time matching (matching.rs)
  replay/         Rebuild a book from events; save/load events as JSON
  simulator/      Predefined scenarios, deterministic generators, scenario runner
  metrics/        Book metrics (spread, mid, depth, imbalance) and trade metrics (VWAP)
  api/            Axum routes: auth, accounts, admin, health
  db/             PostgreSQL connection pool
  observability/  Prometheus recorder and HTTP metrics layer
  main.rs         Simulator CLI
  bin/api.rs              HTTP API server
  bin/bootstrap_admin.rs  One-time first administrator creation

benches/      Criterion benchmarks
migrations/   SQLx PostgreSQL migrations
monitoring/   Prometheus scrape configuration
scripts/, ai/ Run wrappers and deterministic run analysis (paused)
tests/        API integration tests (need PostgreSQL)
```

## How the engine works

- **Price-time priority.** Better price matches first; orders at the same price
  match in arrival order. Trades execute at the resting order's price.
- **Integer prices and quantities.** Values are `u64` ticks and units, never
  floats, so there is no rounding in financial logic.
- **Storage.** Each side is a `BTreeMap<Price, PriceLevel>`. Resting orders
  live in one arena (a `Vec` with reusable slots), and each level is a FIFO
  linked list through that arena plus a cached total quantity. A
  `HashMap<OrderId, Slot>` finds any order directly, so cancel is O(1).
- **Order ids are single-use.** An id that was ever accepted is rejected if it
  is submitted again, even after the order was filled or cancelled.
- **Events and replay.** The book records `OrderAccepted`, `OrderCancelled`,
  `OrderModified`, and `TradeExecuted`. Replaying the events rebuilds the same
  final snapshot. `EventMode` can be `Full`, `TradesOnly`, or `Disabled`.

## Simulator

```bash
cargo run                                   # default scenario: simple-cross
cargo run -- buy-sweeps-asks
cargo run -- cancel-and-modify
cargo run -- two-sided-book
cargo run -- synthetic --count 100          # two-sided resting book, no trades
cargo run -- synthetic-crossing --count 100 # generates trades
```

| Flag | Effect |
|---|---|
| `--json` | Print the run as JSON |
| `--save-events <file>` | Save the event stream |
| `--replay-events <file>` | Rebuild a book from a saved event stream |
| `--output-dir <dir>` | Write `events.json`, `snapshot.json`, `summary.json` |

## API

Start PostgreSQL and Prometheus, apply migrations, then run the server:

```bash
docker compose up -d
sqlx migrate run
cargo run --bin bootstrap_admin -- admin@example.com "Administrator"
cargo run --bin api
```

Configuration is read from `.env`:

```env
DATABASE_URL=postgres://postgres:postgres@localhost:5433/limit_order_book
API_ADDR=127.0.0.1:3000
```

Set `RUST_LOG` to change log levels for one run, for example
`RUST_LOG=limit_order_book=trace cargo run --bin api`. The server finishes
in-flight requests before exiting on Ctrl-C or `SIGTERM`.

| Route | Access | Purpose |
|---|---|---|
| `GET /health` | Public | API and database check |
| `GET /metrics` | Public | Prometheus metrics |
| `POST /auth/signup` | Public | Create a trader |
| `POST /auth/login` | Public | Create a session, returns a bearer token |
| `POST /auth/logout` | Bearer token | Revoke the session |
| `GET /auth/me` | Bearer token | Current user and role |
| `POST /accounts`, `GET /accounts` | Bearer token | Create and list your own accounts |
| `GET /instruments`, `GET /instruments/{id}` | Bearer token | List instruments, or fetch one |
| `POST /admin/users` | Admin | Create a user |
| `POST /admin/instruments` | Admin | Create up to 100 instruments atomically |
| `PATCH /admin/instruments/{id}/status` | Admin | Pause, unpause, or delist |

Passwords are stored as Argon2 hashes. Session tokens are 256-bit random
values; only their SHA-256 digest is stored. Details are in
[docs/AUTHENTICATION.md](docs/AUTHENTICATION.md).

Prometheus runs at `http://localhost:9090` and scrapes the API on port 3000.
Because it runs in Docker, the API must listen on an address the container can
reach, so `API_ADDR=127.0.0.1:3000` is not scraped.

## Tests and benchmarks

```bash
cargo test          # unit tests; integration tests need PostgreSQL running
cargo bench         # Criterion: workloads, hot path, deep books, event modes
cargo clippy --all-targets
```

## More documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — current and target design
- [docs/CURRENT_ARCHITECTURE_FLOWCHARTS.md](docs/CURRENT_ARCHITECTURE_FLOWCHARTS.md) — flow diagrams
- [docs/ROADMAP.md](docs/ROADMAP.md) and [docs/HFT_ROADMAP.md](docs/HFT_ROADMAP.md) — phases and next steps
- [docs/POSTGRES_MARKET_DATA_ROADMAP.md](docs/POSTGRES_MARKET_DATA_ROADMAP.md) — database and pricing plan
- [docs/PERFORMANCE_BASELINE.md](docs/PERFORMANCE_BASELINE.md) — benchmark results
- [docs/AUTHENTICATION.md](docs/AUTHENTICATION.md) — auth design
- [docs/RATE_LIMITING.md](docs/RATE_LIMITING.md) — auth rate limiting and when to move it to Redis
- [docs/WINDMILL.md](docs/WINDMILL.md) — scheduled runs through Windmill
- [docs/AI_WORK_PAUSE.md](docs/AI_WORK_PAUSE.md) — paused AI analysis work
