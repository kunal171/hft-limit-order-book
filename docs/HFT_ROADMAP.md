# HFT-Style Systems Roadmap

This roadmap evolves the correct Rust limit order book into a measured,
deterministic low-latency system. "HFT-style" means the project applies real
trading-system principles; it does not claim production-exchange readiness.

The canonical component design is in [`ARCHITECTURE.md`](ARCHITECTURE.md). The
phase-level project plan is in [`ROADMAP.md`](ROADMAP.md).

## Core Rule

```text
The matching thread owns its book and performs only bounded in-memory work.
```

Inside the hot path:

```text
read decoded command
validate local state
match by price-time priority
update the in-memory book
emit compact sequenced events
```

Outside the hot path:

```text
sockets and protocol decoding
PostgreSQL and Kafka
file and console logging
JSON reports
oracle network calls
Windmill and AI
```

## Current Foundation

Completed foundation:

```text
deterministic price-time matching and FIFO
partial fills, cancel, and modify
BTreeMap<Price, PriceLevel> bid/ask ladders
HashMap<OrderId, Order> direct order storage
HashMap<OrderId, OrderLocation> cancellation index
cached active quantity per price level
configurable event recording
snapshots and deterministic replay
simulator, metrics, latency runner, and Criterion benchmarks
PostgreSQL development environment and initial control-plane API
```

Current limitation:

```text
OrderBook is a fast in-memory library but does not yet run behind a dedicated
sequenced single-writer service. Existing JSON event persistence is useful for
simulation; it is not yet the durable production-style journal.
```

## Next 1: Finish The Control Plane

Finish the current PostgreSQL API work for users, accounts, instruments,
permissions, and configuration. Keep SQLx types out of `engine/`.

Why first:

```text
gateways and risk checks need validated account and instrument configuration,
but matching must consume local snapshots rather than query Postgres per order
```

Done when:

```text
admin actions are authorized
instrument tick/lot rules are validated
the engine crate still has no database dependency
```

## Next 2: Commands, Events, And Sequencing

Introduce compact, versioned input/output types:

```text
EngineCommand: Add, Cancel, Modify
EngineEvent: Accepted, Rejected, Trade, Cancelled, Modified
metadata: shard_id, engine_sequence, event_id, schema_version
```

Do not read wall-clock time or generate randomness inside the deterministic
matching decision. The gateway/sequencer supplies required metadata.

Done when the same ordered command stream always produces the same events,
snapshot, and next sequence.

## Next 3: Single-Writer Engine Runtime

Run each shard on a dedicated thread that exclusively owns its `OrderBook`.
Route commands and results through fixed-capacity queues. Prefer an SPSC ring
for each single-producer/single-consumer boundary; introduce MPSC routing before
the shard rather than multiple writers mutating the book.

Measure:

```text
gateway/decode time
queue waiting time
matching time
event handoff time
end-to-end process time
```

Record p50, p95, p99, p999, and maximum. Queue-full behavior must be a tested
rejection or safety halt, never silent loss or unbounded growth.

## Next 4: Journal, Snapshot, And Recovery

Canonical commands/events require an append-only, checksummed, versioned
journal. Periodic snapshots reduce recovery time:

```text
latest snapshot + journal after snapshot sequence = current in-memory state
```

Document and benchmark acknowledgement modes:

```text
memory-only       lowest latency, weakest durability
local-durable     survives process failure
replicated        survives machine failure, adds network latency
```

The system must not acknowledge canonical work and then silently lose it. Test
crashes before/after acknowledgement, partial journal records, checksum
failures, duplicate replay, and snapshot/journal sequence gaps.

## Next 5: Asynchronous Kafka And PostgreSQL

Publish journaled events to Kafka from a separate component. Kafka does not sit
between the gateway and matching engine and is not awaited by the matching
thread.

Use:

```text
partition key     engine shard
idempotency key   shard_id + engine_sequence
offset policy     commit only after database transaction commit
```

PostgreSQL consumers build queryable order, trade, account, and audit
projections. They must safely process duplicate delivery and be rebuildable
from the event stream.

## Next 6: Binary Gateway

Keep HTTP/JSON for the administrative control plane. Add a persistent binary
order-entry connection for the data plane. Benchmark parsing separately from
matching and use `TCP_NODELAY` where small TCP messages should not wait for
batching.

Zero network latency is impossible. Reduce and control it with co-location,
persistent connections, bounded parsing, separate gateway/matching cores, CPU
affinity, and explicit deadlines.

## Next 7: Advanced Optimization

Only optimize after profiling:

```text
remove remaining hot-path allocation/cloning
preallocate command/event buffers
compare BTreeMap with a bounded direct-index price ladder
pin gateway, engine, journal, and publisher threads
test busy polling, cache-line padding, and NUMA placement
research huge pages and kernel bypass when networking is the measured bottleneck
```

Each change must preserve correctness and improve a recorded metric. Throughput
alone is insufficient; tail latency and jitter are the primary signals.

## Later: pgvector And AI

Do not vectorize every order. Consumers aggregate versioned market-state
windows containing features such as:

```text
spread and mid-price movement
bid/ask depth and imbalance
trade rate and aggressor ratio
add/cancel ratio
short-term return and realized volatility
```

Store the source sequence range, feature version, and normalization version with
each vector. Raw events remain authoritative and vectors remain recomputable.
Start with exact pgvector search and introduce HNSW only after corpus-size,
recall, memory, and query-latency benchmarks justify it.

## Definition Of Done

The HFT runtime milestone is complete when:

```text
one thread exclusively owns each matching shard
all runtime queues are bounded
matching performs no network, database, Kafka, AI, or logging I/O
commands and events have deterministic shard sequences
configured acknowledgement guarantees survive tested failures
snapshot plus journal restores identical state
latency reports separate queue, matching, durability, and network stages
PostgreSQL/Kafka outages cannot corrupt or reorder matching state
```
