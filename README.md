# Limit Order Book

A Rust limit order book and market microstructure lab.

The project starts with a deterministic matching engine and grows in phases into
a research platform for event replay, simulation, market metrics, benchmarks,
AI-assisted analysis, Windmill workflows, and later HFT-style performance
experiments.

## Current Status

Completed through:

```text
Phase 8: Benchmarks
Phase 9: Windmill orchestration in progress
Phase 10: AI analysis foundation paused
```

Implemented so far:

```text
price-time priority matching
limit buy and sell orders
partial fills and full fills
best bid and best ask
trade generation
zero quantity validation
duplicate active order id validation
cancel resting orders
modify resting orders
active order side index
book snapshots
event log
event replay
save and load event streams as JSON
predefined simulator scenarios
CLI runner for scenarios
JSON simulator output
book metrics
trade metrics
order book imbalance
deterministic two-sided synthetic order generation
deterministic crossing synthetic order generation
configurable synthetic order count
Criterion benchmark suite
synthetic workload benchmarks
hot-path operation benchmarks
run artifact output directory
release-friendly simulation wrapper script
auto timestamped run directories
Windmill manual run verified
Windmill scheduled run verified
deterministic AI analysis script
combined run-and-analyze wrapper
```

## Core Idea

The matching engine follows this flow:

```text
incoming order
-> validate
-> match against the opposite side
-> emit trades
-> rest any leftover quantity
-> record events
```

AI, Windmill, databases, dashboards, and reports stay outside the matching hot
path. They are useful for orchestration and analysis, but the core book should
remain deterministic and easy to replay.

The current active direction is HFT-style systems work. AI integration is paused
until the matching engine has stronger performance baselines and cleaner
hot-path boundaries.

## Architecture

```text
src/
  domain/
    event.rs        BookEvent output stream
    order.rs        Order model
    snapshot.rs     Serializable book snapshot
    trade.rs        Trade model
    types.rs        OrderId, Price, Quantity, Side

  engine/
    order_book.rs   Public OrderBook API and tests
    matching.rs     Private price-time matching logic

  metrics/
    book_metrics.rs   Spread, mid price, depth, levels, imbalance
    trade_metrics.rs  Volume, notional, last trade price, VWAP

  replay/
    persistence.rs  Save/load events as JSON
    replay.rs       Rebuild book state from events

  simulator/
    generator.rs    Deterministic synthetic order generators
    scenario.rs     ScenarioCommand input enum
    scenarios.rs    Predefined market scenarios
    runner.rs       Runs scenario commands against a fresh book

  error.rs          OrderBookError
  lib.rs            Library exports
  main.rs           CLI demo/simulator entrypoint

benches/
  order_book_bench.rs  Criterion benchmarks for workloads and hot paths

scripts/
  run_simulation.sh    Release-friendly wrapper for orchestration tools
  analyze_run.sh       Runs deterministic analysis for one run directory
  run_and_analyze.sh   Runs simulation, analysis, and prints clean JSON

ai/
  analyze_run.py       Reads run artifacts and writes analysis.md
  README.md            AI module scope and usage
```

The public library exports common types and helpers from `lib.rs`, so users can
write:

```rust
use limit_order_book::{
    calculate_book_metrics, calculate_trade_metrics, Order, OrderBook, Side,
};
```

## Important Concepts

### Price-Time Priority

The engine follows price-time priority:

```text
better price wins first
same price uses FIFO order
```

For buy orders, the best price is the highest bid. For sell orders, the best
price is the lowest ask.

### Integer Prices

Prices use integer ticks instead of floating point values.

```text
good: 10025 ticks
bad: 100.25 as f64
```

This avoids floating point rounding bugs in financial logic.

### Event Replay

The engine records events such as:

```text
OrderAccepted
OrderCancelled
OrderModified
TradeExecuted
```

Replay lets us rebuild book state from historical events:

```text
saved event stream
-> replay events
-> same final book snapshot
```

This is important for debugging, audits, crash recovery, simulation, and later
analytics.

### Market Metrics

Book metrics describe the final visible market:

```text
best bid
best ask
spread
mid price
total bid quantity
total ask quantity
bid and ask price levels
order book imbalance
```

Trade metrics describe what happened during execution:

```text
trade count
total traded quantity
total notional
last trade price
VWAP
```

VWAP means volume-weighted average price:

```text
total traded notional / total traded quantity
```

### Synthetic Workloads

Synthetic generators create repeatable order flows for testing, metrics, replay,
and later benchmarks.

The current generators are deterministic:

```text
synthetic
-> creates a two-sided resting book without trades

synthetic-crossing
-> builds ask liquidity and then sends crossing buy orders
-> creates trades and execution metrics
```

Deterministic means the same config creates the same sequence every time. That
is useful before adding randomness because tests and benchmark comparisons stay
stable.

### Benchmarks

Benchmarks measure both full workloads and individual hot-path operations.

Full workload benchmarks:

```text
two_sided_1000_orders
crossing_1000_orders
```

Hot-path benchmarks:

```text
add_one_resting_order
single_trade
multi_level_sweep
cancel_order
modify_order
```

The full workload benchmarks show end-to-end scenario cost. The hot-path
benchmarks isolate specific order book operations so later optimizations have a
clear baseline.

## Run

Run tests:

```bash
cargo test
```

Format code:

```bash
cargo fmt
```

Run the default scenario:

```bash
cargo run
```

Run a specific scenario:

```bash
cargo run -- simple-cross
cargo run -- buy-sweeps-asks
cargo run -- cancel-and-modify
cargo run -- two-sided-book
cargo run -- synthetic --count 100
cargo run -- synthetic-crossing --count 100
```

Print simulator output as JSON:

```bash
cargo run -- buy-sweeps-asks --json
```

Save generated events:

```bash
cargo run -- buy-sweeps-asks --save-events events.json
```

Replay saved events:

```bash
cargo run -- --replay-events events.json
```

Write run artifacts:

```bash
cargo run -- synthetic-crossing --count 100 --output-dir runs/run-001
```

This creates:

```text
runs/run-001/events.json
runs/run-001/snapshot.json
runs/run-001/summary.json
```

`events.json` stores the full event stream for replay/debugging.
`snapshot.json` stores the final order book state.
`summary.json` stores small metrics that tools can read quickly.

Run benchmarks:

```bash
cargo bench
```

## Orchestration Wrapper

Build the release binary:

```bash
cargo build --release
```

Run a simulation through the wrapper:

```bash
./scripts/run_simulation.sh synthetic-crossing 100 runs/windmill-test
```

Create a fresh timestamped output folder automatically:

```bash
./scripts/run_simulation.sh synthetic-crossing 100 auto
```

Choose a custom prefix for automatically created folders:

```bash
RUN_PREFIX=windmill-scheduled ./scripts/run_simulation.sh synthetic-crossing 100 auto
```

The wrapper:

```text
uses target/release/limit_order_book
creates the output directory
writes each auto run to a fresh timestamped folder
writes verbose command output to run.log
prints only summary.json to stdout
```

Generated files:

```text
runs/windmill-test/events.json
runs/windmill-test/run.log
runs/windmill-test/snapshot.json
runs/windmill-test/summary.json
```

This is useful for Windmill, CI, local automation, and later AI analysis. The
matching engine remains independent from orchestration code.

Generated run folders under `runs/` are ignored by Git.

## AI Analysis Status

AI integration is paused while the project shifts toward HFT-style systems work.

What exists today:

```text
ai/analyze_run.py
scripts/analyze_run.sh
scripts/run_and_analyze.sh
```

The current AI-facing work is deterministic. It reads saved artifacts such as
`summary.json`, `events.json`, and `snapshot.json`, then writes `analysis.md`.
It does not call an LLM yet.

Pause/resume notes:

```text
docs/AI_WORK_PAUSE.md
```

## HFT-Style Roadmap

The next work is focused on making the engine a stronger low-latency systems
project.

Main priorities:

```text
record benchmark baselines
separate the matching hot path from optional event logging
make event storage configurable and bounded
improve cancel/modify indexes
measure latency percentiles
reduce allocations and cloning
compare BTreeMap with alternate price-ladder designs
add single-writer command processing
keep AI/Windmill outside the hot path
```

Detailed roadmap:

```text
docs/HFT_ROADMAP.md
```

Postgres, users, instruments, and stock/crypto reference pricing are planned as
a separate persistence and market-data phase. They will store durable history
and analytics data without putting database calls inside matching.

Database and pricing roadmap:

```text
docs/POSTGRES_MARKET_DATA_ROADMAP.md
```

## Windmill Usage

The current Windmill integration runs the Rust engine as an external job. The
Windmill script changes into the mounted project directory, calls the wrapper,
and returns only `summary.json` as the job result.

Use this Bash script in Windmill:

```bash
scenario="$1"
count="$2"
output_dir="$3"

# Trim spaces/newlines from Windmill inputs.
scenario="$(echo "$scenario" | xargs)"
count="$(echo "$count" | xargs)"
output_dir="$(echo "$output_dir" | xargs)"

# Defaults if the user leaves inputs empty.
scenario="${scenario:-synthetic-crossing}"
count="${count:-100}"
output_dir="${output_dir:-auto}"

cd /workspace/limit_order_book

RUN_PREFIX=windmill-scheduled ./scripts/run_simulation.sh "$scenario" "$count" "$output_dir"
```

Recommended Windmill inputs:

```text
scenario: synthetic-crossing
count: 100
output_dir: auto
```

Why `output_dir: auto` matters:

```text
fixed folder -> every scheduled run overwrites the previous artifacts
auto folder  -> every scheduled run gets a fresh timestamped folder
```

For Windmill schedules, use a six-field cron expression. The first field is
seconds:

```text
*/5 * * * * *   every 5 seconds
0 */5 * * * *   every 5 minutes
```

Verified scheduled runs create folders like:

```text
runs/windmill-scheduled-20260901-171000/events.json
runs/windmill-scheduled-20260901-171000/run.log
runs/windmill-scheduled-20260901-171000/snapshot.json
runs/windmill-scheduled-20260901-171000/summary.json
```

## Synthetic Examples

Build a two-sided resting book:

```bash
cargo run -- synthetic --count 20
```

This creates buy orders below the base price and sell orders above the base
price. It is useful for book metrics such as spread, mid price, depth, and
imbalance.

Generate trades:

```bash
cargo run -- synthetic-crossing --count 20
```

This first builds ask liquidity, then sends buy orders that cross the spread.
It is useful for trade metrics such as traded quantity, notional, last trade
price, and VWAP.

For crossing scenarios, the final `snapshot.json` may be empty because all
resting orders can be fully matched. Use `events.json` to inspect what happened
during the run.

## Example Metrics

```bash
cargo run -- two-sided-book
```

This scenario leaves both bids and asks resting in the book, so spread, mid
price, depth, and imbalance are visible.

Expected market shape:

```text
best bid: 100
best ask: 105
spread: 5
mid price: 102.5
total bid quantity: 15
total ask quantity: 10
imbalance: 0.6
```

## Roadmap

The project is built in phases:

```text
Phase 1: Matching core
Phase 2: Correctness tests
Phase 3: Cancel and modify orders
Phase 4: Event log and replay
Phase 5: Market data simulator
Phase 6: Market data metrics
Phase 7: Synthetic order generator
Phase 8: Benchmarks
Phase 9: Windmill orchestration
Phase 10: AI/LangChain/LangGraph analysis foundation paused
Phase 11: HFT-style systems and optimization
Phase 12: Postgres persistence, users, instruments, and pricing
```

Next focus:

```text
Phase 11: HFT-style systems and optimization
```

Detailed roadmap:

```text
docs/ROADMAP.md
docs/HFT_ROADMAP.md
docs/POSTGRES_MARKET_DATA_ROADMAP.md
```

## Long-Term Direction

The final project direction:

```text
Rust matching engine
-> event replay and market simulation
-> market data metrics
-> synthetic workloads
-> benchmark reports
-> HFT-style optimization experiments
-> Postgres persistence, users, instruments, and reference pricing
-> Windmill scheduled runs and dashboards
-> AI scenario analysis and LangGraph research workflows
```

The hot path remains Rust-only.
