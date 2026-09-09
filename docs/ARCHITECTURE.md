# HFT-Style Order Book Architecture

This is the source of truth for the target runtime architecture. The project is
an HFT-style learning system: it applies low-latency exchange design principles,
but it is not yet production trading infrastructure.

## Design Goals

```text
deterministic price-time matching
bounded and measurable latency
single ownership of mutable book state
recoverable acknowledged commands
slow systems isolated from the matching path
explicit overload and durability policies
```

## Current And Target State

The current engine already keeps its active book in process memory:

```text
bids              BTreeMap<Price, PriceLevel>
asks              BTreeMap<Price, PriceLevel>
orders            HashMap<OrderId, Order>
order_locations   HashMap<OrderId, OrderLocation>
```

The `OrderBook` owns these collections. Their entries live in the Rust
process's RAM while the engine is running. `BTreeMap` keeps prices sorted, each
`PriceLevel` maintains FIFO order IDs, and the hash maps provide direct lookup.

The target architecture adds a dedicated engine runtime, command sequencing,
durable journaling, and asynchronous event distribution around this core.

## Runtime Architecture

```text
                              CONTROL PLANE

Admin HTTP API ---- users/accounts/instruments ---- PostgreSQL
                                      |
                                      v
                         immutable validation snapshots


                               HOT PATH

Trader connection
      |
      v
Order gateway --> decode/auth/risk --> sequencer --> bounded SPSC queue
                                                     |
                                                     v
                                          single-writer engine thread
                                                     |
                                                     v
                                           in-memory OrderBook shard
                                                     |
                         +---------------------------+------------------+
                         |                                              |
                         v                                              v
                 execution response                          sequenced engine events
                                                                        |
                                                                        v
                                                     append-only durable journal


                          ASYNCHRONOUS DATA PATH

Durable journal --> publisher --> Kafka --> idempotent consumers
                                        |          |
                                        |          +--> PostgreSQL projections
                                        |          +--> metrics/monitoring
                                        |          +--> feature windows --> pgvector
                                        +--------------> archive/research
```

## Hot-Path Boundary

The hot path is intentionally small:

```text
receive decoded command
-> validate against local state
-> process its sequence
-> match
-> update in-memory state
-> emit compact result
```

The matching thread must not perform PostgreSQL queries, Kafka publication,
HTTP calls, oracle-provider calls, JSON serialization, AI inference, console or
file logging, unbounded allocation, or blocking waits without a deadline.

Network I/O belongs to gateway threads. The matching thread receives decoded
commands through a bounded in-process queue, so matching does not wait on a
socket or database.

## Single-Writer Ownership

One thread owns and mutates one order-book shard. Other threads communicate
with it using commands and execution events.

```text
gateway producers -> shard router -> one producer per engine SPSC queue
engine thread      -> one producer per output SPSC queue -> consumers
```

This avoids `Arc<Mutex<OrderBook>>`, preserves deterministic command ordering,
and reduces lock contention and cache movement. Multiple instruments scale by
sharding them across independent engine threads, not by allowing many threads
to mutate one book.

## Durability And Recovery

RAM is the live state, not the recovery mechanism:

```text
latest snapshot + journal entries after snapshot sequence = restored book
```

Every canonical command/event receives a `shard_id`, `engine_sequence`,
`event_id`, and `schema_version`.

| Policy | Acknowledge after | Trade-off |
|---|---|---|
| Memory only | In-memory update | Lowest latency; acknowledged work can be lost |
| Local durable | Journal persistence | Survives process failure, not machine loss |
| Replicated durable | Quorum/replica confirmation | Stronger recovery, higher latency |

The configured acknowledgement point is deliberate. A volatile queue followed
by Kafka is not sufficient for canonical orders or trades because the process
can fail before Kafka receives them.

## PostgreSQL, Kafka, And pgvector

PostgreSQL is the control-plane and query store. It holds users, accounts,
instruments, permissions, configurations, and asynchronous order/trade
projections. It is never queried by the matching algorithm.

Kafka distributes journaled events to independent consumers. Ordering is only
guaranteed within a partition, so events are keyed by authoritative engine
shard. Consumers use `(shard_id, engine_sequence)` as a unique key and commit
Kafka offsets only after their database transaction commits.

pgvector is a downstream research feature. It stores versioned vectors built
from market-state windows such as spread, imbalance, depth, trade rate,
cancellation rate, returns, and volatility. Raw sequenced events remain the
source of truth, so vectors can be recomputed.

## Reference Prices And Risk

Oracle/reference-price adapters run outside the matching thread. They publish
validated, timestamped updates into an in-memory risk snapshot. Order checks
read that local immutable snapshot and reject stale data according to policy.
Reference prices support limits and valuation; they never override price-time
matching or change an execution price.

## Backpressure

All queues are bounded:

| Data | Full/unavailable behavior |
|---|---|
| Orders and canonical executions | Reject new work or halt safely; never drop silently |
| Journal/replication | Stop acknowledgement until policy is satisfied |
| Derived metrics | Sample, coalesce, drop, or recompute |
| AI feature vectors | Recompute later from Kafka/journal |
| Debug logs | Sample or drop |

An unbounded queue hides overload until memory exhaustion. A bounded queue
makes capacity, waiting time, and rejection behavior measurable.

## Network-Latency Strategy

Zero network latency is impossible. The design controls it using persistent
connections, fixed-layout binary order messages, `TCP_NODELAY` where
appropriate, separate gateway and matching cores, bounded queues, deadlines,
and later CPU affinity and NUMA-aware placement. Kernel bypass comes only after
measurement proves the kernel is the bottleneck.

Capture timestamps at gateway receive, decode completion, queue admission,
matching start/end, and response send. Report p50, p95, p99, p999, and maximum
for each stage instead of relying only on average latency.

## Non-Negotiable Invariants

```text
one authoritative sequence per engine shard
one writer mutates a book shard
same valid command stream produces the same result
prices and quantities use integer units
no silent loss of acknowledged canonical events
no database, Kafka, AI, or external network call inside matching
all queues and retry policies are bounded
every asynchronous projection is idempotent and rebuildable
```
