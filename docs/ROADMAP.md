# Limit Order Book Project Roadmap

Each phase has its own branch. Phases 13-15 are paused while we work on a
binary codec and latency measurements. The current order is in
[`PHASE_16_17_LEARNING_PLAN.md`](PHASE_16_17_LEARNING_PLAN.md).

The planned build order:

```text
Rust matching engine
-> correctness tests
-> replay/simulator
-> benchmarks
-> Windmill orchestration/dashboard
-> HFT-style systems and optimization
-> PostgreSQL control plane
-> sequenced single-writer engine runtime
-> durable journal and recovery
-> Kafka event distribution and query projections
-> binary gateway and advanced latency experiments
-> AI/pgvector analysis layer later
```

Keep this boundary throughout the project:

```text
The matching hot path stays Rust-only, deterministic, and low-latency.
AI, Windmill, databases, reports, and dashboards stay outside the hot path.
```

## Git Workflow

We will use one branch per phase.

Basic flow:

```bash
git switch master
git pull --ff-only
git switch -c phase-01-matching-core

# build only that phase
# test it
# commit it

git switch master
git merge phase-01-matching-core
git branch -d phase-01-matching-core
```

For every phase:

```text
1. Create a new branch from master.
2. Build only the planned scope for that phase.
3. Add tests/docs for that phase.
4. Run tests.
5. Commit with a clear message.
6. Merge into main.
7. Start the next phase from updated main.
```

Branch naming:

```text
phase-01-matching-core
phase-02-correctness-tests
phase-03-cancel-modify
phase-04-event-log-replay
phase-05-market-simulator
phase-06-benchmarks
phase-07-ai-langchain
phase-08-langgraph-agent
phase-09-windmill-integration
phase-10-ai-analysis-foundation
phase-11-hft-systems
phase-12-postgres-market-data
phase-13-engine-runtime
phase-14-durable-journal
phase-15-event-pipeline
phase-16-binary-gateway
phase-17-advanced-latency
```

Current detour:

```text
phase-13-engine-commands   pushed and paused; unmerged processor work
phase-16-binary-protocol   independent codec and benchmark lab from master
phase-17-latency-lab       later branch from master after the codec lab merges
```

## Phases 1-6: The Matching Engine

These phases built the engine and the tools used to check it. The early
`BTreeMap<Price, VecDeque<Order>>` design was later replaced by the arena
described in [the architecture doc](ARCHITECTURE.md). It is not the current
data structure.

### Phase 1: Match Orders

A buy checks the lowest available ask; a sell checks the highest bid. A trade
happens only when their prices cross. Better prices match first, and orders at
the same price match in arrival order. An unfilled remainder rests in the
book. This is price-time priority.

The first version used a sorted `BTreeMap` for price levels and a
`VecDeque` for FIFO order within each level. The sorted map made best-price
lookup simple. The queue kept arrival order. See the current arena layout in
[ARCHITECTURE.md](ARCHITECTURE.md).

### Phase 2: Check Correctness

Tests cover an empty book, resting orders, partial and full fills, best-price
selection, FIFO at one price, sweeps across levels, zero quantity, and reused
order IDs. The aim is reproducibility: the same commands must lead to the
same trades and final book.

### Phase 3: Cancel And Modify

Finding an order by scanning every price level would make cancellation
expensive. The engine added an order-ID index for direct lookup. Today,
modifying an order removes and resubmits it, so even a quantity decrease loses
time priority and the changed order may match immediately.

### Phase 4: Record Events And Replay

The engine records accepted, cancelled, and modified orders plus trades.
Replay rebuilds the book from saved events. That lets us compare a run with
its reconstruction after a crash or code change. Event records in memory are
not, by themselves, a durable journal; Phase 14 covers that gap.

### Phase 5: Simulate Runs

The CLI runs named scenarios and deterministic synthetic workloads. Saved
`events.json`, `snapshot.json`, and `summary.json` make results easy to
inspect and replay. Book and trade metrics include spread, depth, imbalance,
and VWAP.

### Phase 6: Benchmark

Criterion measures matching workloads and book shapes. The results and machine
details are in [PERFORMANCE_BASELINE.md](PERFORMANCE_BASELINE.md). Mean timing
is useful, but Phase 17 adds per-command p50/p95/p99/p999 measurements for
tail latency. Keep setup work out of a timed operation.

## Phases 7-10: Analysis And Orchestration

This work is outside the matching loop. It can be paused without affecting the
engine.

| Phase | State | What it covers |
|---|---|---|
| 7. LangChain analysis | Paused | Ideas for explaining completed runs using saved facts; no LLM integration yet |
| 8. LangGraph workflow | Deferred | A resumable experiment loop with human approval before the next run |
| 9. Windmill | Working wrapper | Scheduled or manual simulations through the Rust CLI; see [WINDMILL.md](WINDMILL.md) |
| 10. Deterministic analyzer | Paused | A Python report from run artifacts; see [AI_WORK_PAUSE.md](AI_WORK_PAUSE.md) |

An explanation assistant must read actual orders, trades, snapshots, and
metrics. It must not invent trades or decide how the book matches. Windmill
starts jobs and stores results; it does not participate in each order's
matching decision.

## Phase 11: Low-Latency Engine Work

The engine's arena and price-level lists replaced lazy cancellation. Its
benchmarks are recorded in [PERFORMANCE_BASELINE.md](PERFORMANCE_BASELINE.md).
Further optimization needs a measured bottleneck and a correctness test.
Matching remains in memory; databases, logging, and AI stay outside it.

See [HFT_ROADMAP.md](HFT_ROADMAP.md) for the full list of experiments.

## Phase 12: PostgreSQL Control Plane

The API now has migrations, admin bootstrap, signup and login, hashed sessions,
account ownership, instrument management, rate limiting, metrics, and graceful
shutdown. The engine is not connected to API order entry. The next runtime
step needs a validated in-memory instrument snapshot, so matching never waits
for a PostgreSQL query.

Reference pricing and Redis caching are deferred. Neither is needed to match
the current in-memory orders. [POSTGRES_MARKET_DATA_ROADMAP.md](POSTGRES_MARKET_DATA_ROADMAP.md)
covers the proposed tables, reference prices, and later projections.
[AUTHENTICATION.md](AUTHENTICATION.md) and [RATE_LIMITING.md](RATE_LIMITING.md)
cover the implemented API behavior.

## Phase 13: Single-Writer Runtime (Paused)

The unmerged `phase-13-engine-commands` branch contains command/event types
and a synchronous processor. It still needs:

- A dedicated writer for each book shard and a monotonic sequence per shard.
- Bounded queues with an explicit queue-full response.
- Instrument routing and independent queue-wait and matching measurements.
- Tests showing that replaying the same sequence gives the same result.

One writer avoids a lock around the book. A bounded queue keeps overload from
turning into unbounded memory use. Do not describe this runtime as implemented
on `master`.

## Phase 14: Journal And Recovery (Not Started)

Before promising durable order acceptance, define the acknowledgement point.
Append commands or events to a versioned journal, record shard and sequence
numbers, and recover from a snapshot plus later journal entries. Test
truncation, corruption, and crash-restart behavior. PostgreSQL projections
cannot replace this durability boundary.

## Phase 15: Kafka And Database Projections (Not Started)

Publish committed engine events from the journal, not from the matching call.
Consumers write idempotently to PostgreSQL using shard and sequence as their
identity. Commit offsets only after the database transaction commits. Monitor
lag and failed records. A Kafka or PostgreSQL outage must not change matching.

Later, build market-state windows and versioned analytics features. Add
pgvector only after ordinary queries and exact search have been measured.

## Phase 16: Binary Protocol (Current Lab)

Start with the 56-byte offline codec, malformed-frame tests, round trips, and
binary-versus-JSON decode benchmarks in
[BINARY_ORDER_PROTOCOL.md](BINARY_ORDER_PROTOCOL.md). A later loopback TCP test
may report that a command decoded, **not** that it executed or was saved.
A real gateway needs Phase 13 routing and ownership first.

Keep socket reads and decoding outside the matching thread. Reject malformed
frames before queuing them. Measure receive, decode, queue, match, and response
time separately once those stages exist.

## Phase 17: Latency Experiments (After The Codec Lab)

Record a fresh baseline before changing the engine. Measure p50, p95, p99,
p999, throughput, and allocations for the same workload before and after each
change. Profile first; then try one change at a time, such as buffer reuse,
CPU affinity, or queue placement. SIMD needs a batch-shaped workload. Kernel
bypass is research until network measurements and suitable hardware justify
it. See [PHASE_16_17_LEARNING_PLAN.md](PHASE_16_17_LEARNING_PLAN.md).

A matching benchmark measured in nanoseconds is not an end-to-end order
latency claim.

## Branch Checklist

Before a merge, run formatting, tests, Clippy, and any relevant benchmark.
Check that the branch contains only its intended change. Update the README
and design docs when behavior changes, then merge and start the next branch
from updated `master`.

For an unfamiliar technique, write down the reason for using it, where it
runs, its cost, and the test or measurement that would prove it helps.
