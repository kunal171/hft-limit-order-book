# HFT-Style Systems Roadmap

This roadmap describes how to evolve the project from a correct educational
limit order book into a serious low-latency systems project.

Important wording:

```text
HFT-style means we learn and apply low-latency exchange/trading-system ideas.
It does not mean this project is production-ready trading infrastructure yet.
```

The goal is to make the matching engine measurable, deterministic, allocation
aware, and cleanly separated from slow systems such as AI, databases, dashboards,
and orchestration tools.

## Core Rule

```text
The matching hot path must stay small, deterministic, Rust-only, and measurable.
```

Hot path for this project:

```text
order command
-> validate
-> match
-> update book state
-> emit minimal result
```

Outside the hot path:

```text
AI analysis
Windmill jobs
JSON report generation
file persistence
database writes
debug logs
dashboard updates
slow network calls
reference/oracle price fetches
```

## Current Baseline

Today the engine is correctness-first.

Current strengths:

```text
price-time priority
FIFO per price level
partial fills
cancel and modify
event log
replay
snapshots
simulator
metrics
Criterion benchmarks
Windmill orchestration
deterministic AI analysis foundation
```

Current structures:

```text
BTreeMap<Price, VecDeque<Order>> for bids and asks
HashMap<OrderId, Side> for active order side lookup
Vec<BookEvent> for in-memory event history
Vec<Trade> for returned trades
```

Known HFT concerns:

```text
event log clones orders/trades on every command
event log can grow without bound
cancel still scans inside the price level
BTreeMap is clean but not always the fastest price ladder
JSON output is good for tools but not for the hot path
benchmark baseline numbers are not recorded in docs yet
latency percentiles are not tracked yet
allocation count is not tracked yet
```

## Phase 11.1: Baseline And Safety Net

Goal:

```text
record where the engine is before optimizing
```

Work:

```text
run cargo test
run cargo bench
record benchmark numbers in docs/PERFORMANCE_BASELINE.md
document machine details: CPU, OS, Rust version, build mode
add benchmark notes for add, match, sweep, cancel, modify
confirm replay tests still pass
```

Why first:

```text
Without a baseline, optimization becomes guessing.
```

Interview answer:

```text
I do not optimize the matching engine blindly. I first create correctness tests,
then benchmarks, then compare every optimization against a recorded baseline.
```

Exit criteria:

```text
baseline doc exists
cargo test passes
cargo bench runs successfully
we know which operation is slowest
```

## Phase 11.2: Hot-Path Boundary

Goal:

```text
separate matching from optional observability
```

Work:

```text
introduce engine configuration
make event recording configurable
support an event-log-off mode for benchmarks
keep replay mode available for debugging and audit
avoid forcing clones when events are disabled
```

Why:

```text
An HFT hot path should not always pay for debug/history features.
```

Likely design:

```text
OrderBookConfig {
    event_mode: EventMode
}

EventMode:
    Full
    TradesOnly
    Disabled
```

Trade-off:

```text
Full events are better for replay and debugging.
Disabled events are better for latency benchmarks.
```

Exit criteria:

```text
existing replay behavior still works with Full mode
benchmarks can run with Disabled mode
tests prove both modes behave correctly
```

## Phase 11.3: Bounded Event Storage

Goal:

```text
prevent in-memory event history from growing forever
```

Work:

```text
add bounded event log option
store only the last N events when configured
keep full event persistence available through explicit output paths
document when bounded mode is safe
```

Why:

```text
Vec<BookEvent> can grow until memory becomes a problem during long simulations.
```

Good rule:

```text
full replay/audit run -> full event log
low-latency benchmark -> disabled or bounded event log
```

Exit criteria:

```text
bounded mode never exceeds configured capacity
full mode behavior remains unchanged
```

## Phase 11.4: Better Order Index

Goal:

```text
make cancel and modify closer to direct lookup
```

Current problem:

```text
order_sides tells us Buy/Sell, but cancel still scans price levels.
```

Better index:

```text
order_id -> side + price
```

Later advanced index:

```text
order_id -> side + price + queue position/handle
```

Why:

```text
cancel/modify are common operations in trading systems.
Scanning gets expensive as the book grows.
```

Trade-off:

```text
More indexes make cancel faster, but every add/fill/cancel must keep indexes
correct. This adds correctness risk.
```

Exit criteria:

```text
cancel does not scan both sides
modify uses the index
filled orders are removed from the index
tests cover stale index bugs
benchmarks compare before/after
```

## Phase 11.5: Allocation Reduction

Goal:

```text
reduce heap allocation and unnecessary cloning in the matching path
```

Work:

```text
measure allocations with tooling
avoid event clones when event mode is disabled
reuse trade buffers where practical
pre-allocate vectors for known workloads
prefer small value types for ids/prices/quantities
```

Possible tools:

```text
heaptrack
valgrind massif
DHAT
custom allocation counter later
```

Why:

```text
Small allocations can dominate low-latency code because allocator work is
variable and hurts tail latency.
```

Exit criteria:

```text
allocation notes exist
one measured allocation reduction is implemented
benchmark numbers show the impact
```

## Phase 11.6: Latency Distribution

Goal:

```text
measure p50, p95, p99, and p999 latency, not only throughput
```

Work:

```text
add a latency runner separate from Criterion
record per-command timings
produce latency_report.json
track min, max, p50, p95, p99, p999
run the same synthetic workload repeatedly
```

Why:

```text
Average latency hides the bad cases. Trading systems care deeply about tail
latency because one slow order can matter.
```

Exit criteria:

```text
latency report is generated
tail latency is documented
HFT changes compare p99/p999 before and after
```

## Phase 11.7: Command API And Sequencing

Goal:

```text
make order input look more like an exchange gateway
```

Work:

```text
create OrderCommand enum
add sequence numbers
add timestamps outside the core matching decision
return ExecutionReport enum
make add/cancel/modify share one command-processing entrypoint
```

Why:

```text
Real systems process ordered command streams. A command API makes replay,
testing, journaling, and gateway integration cleaner.
```

Likely design:

```text
enum OrderCommand {
    Add(Order),
    Cancel { order_id: OrderId },
    Modify { order_id: OrderId, new_price: Price, new_quantity: Quantity },
}

enum ExecutionReport {
    Accepted,
    Rejected,
    Cancelled,
    Modified,
    Trades(Vec<Trade>),
}
```

Exit criteria:

```text
existing public methods can call the command API
simulator can run command streams
replay remains deterministic
```

## Phase 11.8: Alternative Price Ladder

Goal:

```text
compare BTreeMap with a faster structure for bounded tick ranges
```

Current choice:

```text
BTreeMap<Price, VecDeque<Order>>
```

Why it was good:

```text
simple
correct
sorted
great for learning
works for sparse prices
```

HFT alternative:

```text
array/vector price ladder for bounded price range
```

Why it can be faster:

```text
direct indexing can avoid tree traversal
data can be more cache friendly
```

Trade-off:

```text
An array ladder needs known tick size and price bounds. It can waste memory for
sparse markets.
```

Exit criteria:

```text
keep BTreeMap engine as baseline
prototype alternate ladder behind a separate module or feature
benchmark both with the same workloads
document when each structure wins
```

## Phase 11.9: Single-Writer Engine

Goal:

```text
model realistic concurrency without locking the book from many threads
```

Design:

```text
one matching thread owns the OrderBook
many producers send commands to it
one or more consumers receive execution reports
```

Why:

```text
The order book itself is mutable state. A single-writer design avoids Mutex
contention inside matching and preserves deterministic ordering.
```

Possible tools:

```text
std::sync::mpsc for learning
crossbeam-channel later
lock-free ring buffer later
```

Exit criteria:

```text
matching engine does not need Arc<Mutex<OrderBook>>
command order is deterministic
throughput and latency are measured
```

## Phase 11.10: Binary Protocol Experiment

Goal:

```text
learn why trading systems avoid JSON on low-latency paths
```

Work:

```text
define fixed-size binary command format
write parser/encoder
benchmark binary parse vs JSON parse
keep JSON for tooling and reports
```

Why:

```text
JSON is human-friendly but expensive to parse. Binary protocols reduce parsing
cost and message size.
```

Exit criteria:

```text
binary command parser exists
tests cover encoding/decoding
benchmark compares protocol costs
```

## Phase 11.11: Persistence Outside Hot Path

Goal:

```text
support audit/recovery without blocking matching
```

Work:

```text
append command/event journal outside the direct matching function
batch disk writes
document fsync trade-offs
make replay rebuild from the journal
```

Why:

```text
Durability matters, but synchronous disk writes in the hot path can destroy
latency.
```

Exit criteria:

```text
journal format is documented
replay from journal works
hot-path matching code does not write files directly
```

## Phase 11.12: Production-Style Guardrails

Goal:

```text
make the project look like serious trading infrastructure practice
```

Work:

```text
reject invalid prices and quantities
add max order size checks
add symbol/instrument model
add user/session/trader id outside matching core
add deterministic error responses
add structured logs outside hot path
add CI checks for test/fmt/clippy
add benchmark regression notes
```

Why:

```text
Real trading systems need correctness, risk checks, observability, and
repeatable operations, not just fast matching.
```

Exit criteria:

```text
guardrails are tested
CI verifies normal quality checks
README explains HFT-style architecture
```

## Advanced Later

These are for learning after the core project is strong:

```text
CPU pinning
NUMA awareness
huge pages
busy polling
kernel bypass concepts
DPDK/io_uring research
lock-free ring buffers
custom memory pools
cache-line alignment
false-sharing experiments
branch prediction notes
market data feed handler
FIX-like gateway
multi-symbol sharding
risk engine
paper-trading simulator
Postgres run history
stock/crypto reference price ingestion
```

## After HFT: Postgres And Market Data

After the hot path is cleaner, add Postgres and reference/oracle pricing as the
next major product/backend phase.

Why after HFT cleanup:

```text
The database layer should persist and analyze engine output, not shape the core
matching design.
```

Next detailed roadmap:

```text
docs/POSTGRES_MARKET_DATA_ROADMAP.md
```

## What To Build First

Start here:

```text
1. docs/PERFORMANCE_BASELINE.md
2. event recording config
3. bounded/disabled event log modes
4. cancel/modify index improvement
5. latency distribution runner
```

This order is intentional:

```text
measure first
remove obvious hot-path overhead second
improve data structures third
then add realistic systems architecture
```
