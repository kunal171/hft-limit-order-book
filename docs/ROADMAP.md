# Limit Order Book Project Roadmap

This project will be built in phases. Each phase gets its own Git branch, and we merge it before starting the next phase.

The long-term vision is:

```text
Rust matching engine
-> correctness tests
-> replay/simulator
-> benchmarks
-> Windmill orchestration/dashboard
-> HFT-style systems and optimization
-> AI analysis layer
-> LangGraph experiment workflows
```

Important project rule:

```text
The matching hot path stays Rust-only, deterministic, and low-latency.
AI, Windmill, databases, reports, and dashboards stay outside the hot path.
```

## Git Workflow

We will use one branch per phase.

Basic flow:

```bash
git checkout main
git pull
git checkout -b phase-01-matching-core

# build only that phase
# test it
# commit it

git checkout main
git merge phase-01-matching-core
git branch -d phase-01-matching-core
```

For every phase:

```text
1. Create a new branch from main.
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
```

## Current Starting Point

The project already has a small Rust order book structure:

```text
src/lib.rs
src/types.rs
src/order.rs
src/trade.rs
src/order_book.rs
src/main.rs
```

Current concepts already introduced:

```text
Order
Trade
Side
Price
Quantity
OrderBook
BTreeMap
VecDeque
best_bid
best_ask
price-time priority
partial fills
FIFO at same price
```

Before starting the next phase, understand this flow:

```text
incoming order
-> check opposite side
-> match if prices cross
-> generate trades
-> rest leftover quantity in the book
```

## Phase 1: Matching Core

Branch:

```text
phase-01-matching-core
```

Goal:

```text
Build a correct basic limit order book.
```

Features:

```text
add limit buy order
add limit sell order
match buy against asks
match sell against bids
partial fills
full fills
best bid
best ask
trade generation
FIFO matching at same price
```

Important algorithm: price-time priority.

Meaning:

```text
Better price wins first.
If price is equal, older order wins first.
```

Why it matters:

```text
This is the standard matching rule for many limit order books.
It makes matching deterministic and fair.
```

Data structures:

```text
BTreeMap<Price, VecDeque<Order>>
```

Why `BTreeMap`:

```text
It keeps prices sorted.
Bids need highest price first.
Asks need lowest price first.
```

Why `VecDeque`:

```text
It preserves FIFO order at the same price level.
We can pop from the front and push to the back efficiently.
```

Exit criteria:

```text
cargo test passes
basic matching examples work
you can explain price-time priority without reading notes
```

## Phase 2: Correctness Tests

Branch:

```text
phase-02-correctness-tests
```

Goal:

```text
Make the engine trustworthy before making it fast.
```

Test cases:

```text
empty book
buy rests when no ask exists
sell rests when no bid exists
buy matches lowest ask
sell matches highest bid
partial fill leaves remaining order
full fill removes order
same-price FIFO
better price beats older worse price
multiple price levels swept by one order
zero quantity rejected
duplicate order id rejected
```

Important concept: deterministic behavior.

Meaning:

```text
Same input event sequence must always produce the same trades and final book state.
```

Why it matters:

```text
Trading systems must be replayable, debuggable, and auditable.
```

Exit criteria:

```text
tests describe business behavior clearly
edge cases are covered
no optimization work yet
```

## Phase 3: Cancel And Modify Orders

Branch:

```text
phase-03-cancel-modify
```

Goal:

```text
Support real exchange-like order lifecycle operations.
```

Features:

```text
cancel_order(order_id)
modify_order(order_id, new_price, new_quantity)
query_order(order_id)
reject unknown order id
reject modifying filled order
preserve or reset priority depending on modify rule
```

Important data-structure issue:

```text
price -> queue of orders
```

is good for matching, but bad for cancellation.

Why:

```text
To cancel order 5000, we may need to scan every price level and every order.
That is too slow for a serious order book.
```

Likely extra index:

```text
order_id -> side + price + location
```

Why:

```text
It allows direct lookup by order id.
```

Special concept: priority loss on modification.

Usually:

```text
Reducing quantity may keep priority.
Increasing quantity or changing price usually loses priority.
```

Exit criteria:

```text
cancel works
modify works
all behavior is tested
you can explain why cancellation needs an index
```

## Phase 4: Event Log And Replay

Branch:

```text
phase-04-event-log-replay
```

Goal:

```text
Make the order book replayable and explainable.
```

Events:

```text
OrderAccepted
OrderRejected
OrderCancelled
OrderModified
TradeExecuted
BookSnapshot
```

Important concept: event sourcing.

Meaning:

```text
Instead of only storing current state, store every event that changed the state.
```

Why it matters:

```text
debugging
auditing
replaying market sessions
rebuilding book state
feeding AI explanations later
```

Exit criteria:

```text
events are emitted for important actions
book state can be rebuilt from events
tests verify replay gives same final state
```

## Phase 5: Market Data Simulator

Branch:

```text
phase-05-market-simulator
```

Goal:

```text
Feed market events into the order book and observe behavior.
```

Inputs:

```text
CSV file
JSON file
synthetic generated events
```

Outputs:

```text
trades.json
book_snapshots.json
metrics.json
latency_report.json
```

Market microstructure concepts:

```text
bid
ask
spread
mid price
slippage
liquidity
order book imbalance
VWAP
market impact
```

Why this phase matters:

```text
The order book becomes more than unit tests.
We can replay scenarios, generate metrics, and later let AI analyze runs.
```

Exit criteria:

```text
simulator can replay a file
outputs are saved in a stable format
basic metrics are produced
```

## Phase 6: Benchmarks

Branch:

```text
phase-06-benchmarks
```

Goal:

```text
Measure performance before optimizing.
```

Metrics:

```text
orders per second
average latency
p50 latency
p95 latency
p99 latency
p999 latency
memory usage
allocation count
```

Important concept: tail latency.

Meaning:

```text
Average latency is not enough.
p99 and p999 show the slowest important requests.
```

Example:

```text
average: 10 microseconds
p99: 2 milliseconds
```

This means most events are fast, but some are dangerously slow.

Exit criteria:

```text
benchmarks run locally
baseline numbers are recorded
no optimization without measurement
```

## Phase 7: AI Layer With LangChain

Branch:

```text
phase-07-ai-langchain
```

Status:

```text
paused after deterministic analysis foundation
```

Goal:

```text
Use AI to analyze and explain order book runs.
```

Important rule:

```text
AI does not decide matching.
AI only analyzes deterministic output from Rust.
```

Features:

```text
AI backtest report generator
AI order explanation assistant
AI edge-case scenario generator
AI natural-language query layer over run outputs
AI post-mortem generator for bad runs
```

Important concept: tool calling.

Meaning:

```text
The AI calls deterministic tools instead of guessing.
```

Example tools:

```text
get_order(order_id)
get_trades(order_id)
get_book_snapshot(event_id)
get_metrics(run_id)
```

Why it matters:

```text
Trading explanations must be grounded in actual data.
The AI should explain evidence, not invent behavior.
```

Exit criteria:

```text
AI can summarize a run
AI can explain an order from logs/events
AI uses tools or structured data instead of guessing
```

Pause notes:

```text
docs/AI_WORK_PAUSE.md
```

## Phase 8: LangGraph Research Agent

Branch:

```text
phase-08-langgraph-agent
```

Status:

```text
deferred until after HFT/system-strengthening work
```

Goal:

```text
Create a stateful AI workflow for repeated experiments.
```

Workflow:

```text
AI proposes experiment
-> Rust simulator runs it
-> metrics are collected
-> AI analyzes result
-> AI suggests next experiment
-> human approves or edits
-> workflow continues
```

Important concept: human-in-the-loop.

Meaning:

```text
The AI can suggest actions, but important decisions require human approval.
```

Why LangGraph:

```text
It is useful for long-running, stateful, resumable agent workflows.
```

Possible agents:

```text
strategy experiment planner
adversarial tester
latency investigation assistant
market scenario generator
```

Exit criteria:

```text
agent workflow has clear state
human approval exists before next experiment
results are persisted between steps
```

## Phase 9: Windmill Integration

Branch:

```text
phase-09-windmill-integration
```

Goal:

```text
Use Windmill as the orchestration and dashboard layer.
```

Windmill should run:

```text
scheduled backtests
parameter sweeps
AI report generation
benchmark jobs
dataset ingestion
dashboard views
manual experiment triggers
```

Important rule:

```text
Windmill is not inside the matching hot path.
Windmill coordinates jobs around the Rust engine.
```

Important concept: orchestration.

Meaning:

```text
Windmill coordinates multiple steps, workers, schedules, and outputs.
```

Example flow:

```text
select dataset
-> run Rust simulation
-> store metrics
-> run LangChain report
-> display dashboard
```

Why Windmill:

```text
It gives us workflows, schedules, job logs, dashboards, and worker execution.
```

Exit criteria:

```text
Windmill can trigger a backtest
Windmill can show or link to results
AI report can be part of a Windmill flow
```

## Phase 10: AI Analysis Foundation

Branch:

```text
phase-10-ai-langgraph-analysis
```

Status:

```text
paused
```

What exists:

```text
deterministic Python analyzer
run-and-analyze shell wrapper
AI module README
pause/resume documentation
```

Pause notes:

```text
docs/AI_WORK_PAUSE.md
```

Why pause:

```text
The Rust matching engine should become a stronger low-latency systems project
before we continue with LLM/LangGraph work.
```

## Phase 11: HFT-Style Systems And Optimization

Branch:

```text
phase-11-hft-systems
```

Current priority:

```text
resume here before continuing LangChain/LangGraph
```

Goal:

```text
Learn low-latency engineering after correctness and measurement exist.
```

Topics:

```text
cache-friendly data layout
pre-allocation
avoiding heap allocations
single-writer design
lock-free queues
CPU pinning
batching
binary protocols
zero-copy parsing
memory pools
branch prediction
NUMA awareness
kernel bypass concepts
```

Important concept: hot path.

Meaning:

```text
The small section of code where every nanosecond matters.
```

For this project:

```text
receive event
-> validate
-> match
-> emit trade/event
```

Avoid in hot path:

```text
AI calls
database calls
network calls
string-heavy logging
heap allocation
locks
unbounded queues
dynamic dispatch unless justified
```

Exit criteria:

```text
benchmark baseline exists
optimization improves measured numbers
correctness tests still pass
trade-offs are documented
```

Detailed HFT roadmap:

```text
docs/HFT_ROADMAP.md
```

## Phase 12: Postgres, Users, Instruments, And Pricing

Branch:

```text
phase-12-postgres-market-data
```

Goal:

```text
persist completed runs and add product-level trading concepts
```

Scope:

```text
Postgres connection with sqlx
users and accounts
instruments such as BTC-USDT, ETH-USD, AAPL-USD
simulation runs
orders and trades
book snapshots
run metrics
reference/oracle prices
recent-run queries for dashboards
```

Important rule:

```text
Postgres is outside the matching hot path.
```

Reference/oracle price rule:

```text
Oracle prices are used for risk checks, mark price, slippage analysis, and
analytics. They do not override price-time matching.
```

Why it matters:

```text
This turns the project from only a matching engine into a backend trading
system with durable history, user ownership, instruments, market pricing, and
queryable analytics.
```

Detailed database roadmap:

```text
docs/POSTGRES_MARKET_DATA_ROADMAP.md
```

## Phase Merge Checklist

Before merging any phase branch:

```text
cargo test passes
new behavior has tests
README/docs updated if concepts changed
no unrelated changes
branch name matches phase
commit message explains what changed
you can explain every special algorithm/data structure used
```

Suggested commit message style:

```text
phase 01: add matching core
phase 02: expand correctness tests
phase 03: support cancel and modify orders
phase 04: add event log and replay
phase 05: add market simulator
phase 06: add benchmark baseline
phase 07: add AI run analysis
phase 08: add LangGraph experiment workflow
phase 09: add Windmill orchestration
phase 10: pause AI foundation and document resume plan
phase 11: add HFT-style systems improvements
phase 12: add Postgres market data persistence
```

## Learning Rule For This Project

Whenever we introduce something new, we must answer:

```text
What is it?
Why did we choose it?
What problem does it solve?
What trade-off does it create?
Is it in the hot path or outside the hot path?
How would I explain it in an interview?
```

This is the main learning standard for the project.
