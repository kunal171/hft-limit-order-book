# Performance Baseline

This file records the saved performance baseline for the HFT-style systems
phase.

Purpose:

```text
measure first, optimize second
```

Any future optimization should compare against this file before we decide
whether the change actually helped.

## Latest Run

```text
Date: 2026-09-06
Branch: phase-11-hft-systems
Commit: 134edd0
Rust: rustc 1.97.1 (8bab26f4f 2026-07-14)
Command: cargo bench
Verification: cargo fmt --check, cargo check, cargo test
Test result: 44 passed
```

Criterion reports each result as:

```text
[lower bound, estimate, upper bound]
```

Use the middle number as the main baseline estimate. Lower is better.

| Benchmark | Lower | Estimate | Upper | What It Measures |
| --- | ---: | ---: | ---: | --- |
| `two_sided_1000_orders` | 89.319 us | 91.231 us | 93.545 us | Runs 1,000 deterministic non-crossing orders |
| `crossing_1000_orders` | 86.170 us | 89.582 us | 92.976 us | Runs 1,000 deterministic orders that create trades through `run_scenario` |
| `add_one_resting_order` | 109.31 ns | 110.15 ns | 111.01 ns | Creates a fresh book and adds one order that rests |
| `single_trade_ref` | 57.889 ns | 58.235 ns | 58.621 ns | Matches one crossing order against one resting order using corrected benchmark boundary |
| `multi_level_sweep_ref` | 197.99 ns | 199.43 ns | 201.04 ns | Sweeps multiple price levels with one order using corrected benchmark boundary |
| `cancel_order_ref` | 55.373 ns | 59.782 ns | 66.915 ns | Cancels one resting order using corrected benchmark boundary |
| `modify_order_ref` | 111.87 ns | 116.99 ns | 122.96 ns | Modifies one resting order using corrected benchmark boundary |
| `two_sided_10000_orders` | 995.96 us | 1.0061 ms | 1.0167 ms | Runs 10,000 deterministic non-crossing orders |
| `two_sided_100000_orders` | 15.667 ms | 16.368 ms | 17.116 ms | Runs 100,000 deterministic non-crossing orders |
| `cancel_from_10000_deep_level_ref` | 77.951 ns | 82.610 ns | 87.955 ns | Cancels near the end of a 10,000-order FIFO price level using corrected benchmark boundary |
| `cancel_after_9999_lazy_cancels_ref` | 243.11 ns | 267.19 ns | 294.56 ns | Cancels the last active order after 9,999 lazy cancels left stale queue IDs |
| `modify_from_10000_deep_level_ref` | 3.0558 us | 3.1710 us | 3.3049 us | Modifies near the end of a 10,000-order FIFO price level using corrected benchmark boundary |
| `best_bid_from_price_level_quantity_10000_orders` | 3.0837 ns | 3.1136 ns | 3.1488 ns | Reads best bid from `PriceLevel.total_quantity` |
| `resting_count_from_10000_active_orders` | 247.65 ps | 254.94 ps | 263.13 ps | Reads active order count from the order map length |
| `best_bid_after_9999_same_level_lazy_cancels` | 3.1666 ns | 3.1911 ns | 3.2184 ns | Reads best bid after many same-level lazy cancels |
| `best_bid_after_9999_cancelled_price_levels` | 3.1509 ns | 3.2105 ns | 3.2788 ns | Reads best bid after many cancelled price levels have been cleaned up |
| `multi_symbol_100x1000_orders` | 12.119 ms | 12.355 ms | 12.613 ms | Runs 100 books with 1,000 orders each |
| `crossing_1000_events_full` | 75.784 us | 76.976 us | 78.401 us | Crossing workload with full in-memory event recording |
| `crossing_1000_events_trades_only` | 72.137 us | 72.578 us | 73.047 us | Crossing workload recording only trade events |
| `crossing_1000_events_disabled` | 74.699 us | 75.563 us | 76.486 us | Crossing workload with event recording disabled |

## Event Mode Comparison

Compare these three benchmarks with each other:

```text
crossing_1000_events_full
crossing_1000_events_trades_only
crossing_1000_events_disabled
```

Do not compare them directly with `crossing_1000_orders` as an event-only
measurement. The older `crossing_1000_orders` path also accumulates trades and
copies the event log through `run_scenario`, so it measures a slightly different
end-to-end simulator path.

Current estimate:

```text
Full:        76.976 us
TradesOnly:  72.578 us
Disabled:    75.563 us
```

Interpretation:

```text
Full event logging costs about 1.413 us over Disabled on this 1,000-order
crossing workload.

TradesOnly measured about 2.985 us faster than Disabled in this run, so do not
overinterpret the exact mode ordering. Full costs about 4.398 us over
TradesOnly, so accepted/cancel/modify event storage is still visible in this
workload.
```

## Corrected Operation Benchmarks

The operation-style benchmarks now use `iter_batched_ref` and `_ref` names:

```text
single_trade_ref
multi_level_sweep_ref
cancel_order_ref
modify_order_ref
cancel_from_10000_deep_level_ref
cancel_after_9999_lazy_cancels_ref
modify_from_10000_deep_level_ref
best_bid_from_price_level_quantity_10000_orders
best_bid_after_9999_same_level_lazy_cancels
best_bid_after_9999_cancelled_price_levels
```

These benchmarks use `iter_batched_ref`, where the prepared `OrderBook` is
created outside the measured closure and passed into the measured closure by
mutable reference. This keeps setup outside timing and avoids counting the drop
of the prepared book inside the measured operation.

Corrected hot-path estimates:

```text
single trade:       58.235 ns
multi-level sweep: 199.43 ns
cancel one order:   59.782 ns
modify one order:  116.99 ns
```

Deep cancel is now fast because cancel removes from direct maps and updates
depth without scanning the FIFO queue. Deep modify still physically removes the
old order ID from its queue, so it remains microsecond-level.

```text
deep cancel: 82.610 ns
deep cancel after 9,999 lazy cancels: 267.19 ns
deep modify: 3.1710 us
```

This confirms the lazy-cancel design improves cancel latency dramatically while
keeping active depth correct for trader-facing pressure/volume reads.

## PriceLevel Liquidity Tradeoff

The current design stores active liquidity on the price level itself:

```text
FIFO queues: preserve matching order
orders map: source of truth for active orders
PriceLevel.total_quantity: active liquidity and pressure at that price
```

This makes best-price and pressure reads very cheap:

```text
best_bid_from_price_level_quantity_10000_orders: 3.1136 ns
resting_count_from_10000_active_orders: 254.94 ps
best_bid_after_9999_same_level_lazy_cancels: 3.1911 ns
best_bid_after_9999_cancelled_price_levels: 3.2105 ns
```

Compared with separate `bid_depth`/`ask_depth` maps, this removes duplicate
depth maps and keeps FIFO order plus active quantity together. The cost is that
each price level now carries both queue and quantity state, and cancel may remove
the whole price level when active quantity reaches zero.

```text
add_one_resting_order: 110.15 ns
single_trade_ref: 58.235 ns
two_sided_1000_orders: 91.231 us
cancel_order_ref: 59.782 ns
```

This is the main tradeoff to watch in the next iteration.

## Run Details

```text
Date: 2026-09-04
Branch: phase-11-hft-systems
Commit: 95e3dce
Project: limit_order_book v0.1.0
Rust edition: 2024
Benchmark profile: cargo bench / optimized release benchmark profile
Benchmark tool: Criterion 0.8.2
Plot backend note: Gnuplot not found, Criterion used plotters backend
```

## Machine

```text
OS: Linux Shady 7.0.0-30-generic
Kernel: #30-Ubuntu SMP PREEMPT_DYNAMIC Fri Jul 31 18:22:54 UTC 2026
Architecture: x86_64
CPU: AMD Ryzen 5 5600X 6-Core Processor
CPU threads: 12
Cores per socket: 6
Threads per core: 2
Socket(s): 1
CPU max MHz: 4654.2881
CPU min MHz: 560.7580
L1d cache: 192 KiB (6 instances)
L1i cache: 192 KiB (6 instances)
L2 cache: 3 MiB (6 instances)
L3 cache: 32 MiB (1 instance)
NUMA nodes: 1
```

## Toolchain

```text
rustc 1.97.1 (8bab26f4f 2026-07-14)
cargo 1.97.1 (c980f4866 2026-06-30)
```

## Correctness Check

Command:

```bash
cargo test
```

Result:

```text
42 passed
0 failed
0 ignored
0 measured
0 filtered out
```

Interpretation:

```text
The correctness safety net passes before starting HFT-style optimization work.
```

## Benchmark Results

Command:

```bash
cargo bench
```

Benchmark scope:

```text
small hot-path operations
1,000 order synthetic workloads
10,000 and 100,000 order larger workloads
deep price-level cancel/modify operations
100-symbol synthetic workload
```

Criterion reports each result as:

```text
[lower bound, estimate, upper bound]
```

Use the middle number as the main baseline estimate.

| Benchmark | Lower | Estimate | Upper | What It Measures |
| --- | ---: | ---: | ---: | --- |
| `two_sided_1000_orders` | 54.130 us | 54.422 us | 54.754 us | Runs 1,000 deterministic non-crossing orders |
| `crossing_1000_orders` | 55.790 us | 58.317 us | 61.072 us | Runs 1,000 deterministic orders that create trades |
| `add_one_resting_order` | 82.056 ns | 83.538 ns | 85.425 ns | Adds one order that rests in the book |
| `single_trade` | 104.18 ns | 112.93 ns | 125.15 ns | Matches one crossing order against one resting order |
| `multi_level_sweep` | 234.73 ns | 240.97 ns | 248.52 ns | Sweeps multiple price levels with one order |
| `cancel_order` | 102.31 ns | 103.91 ns | 105.49 ns | Cancels one resting order |
| `modify_order` | 156.69 ns | 159.55 ns | 163.19 ns | Modifies one resting order |
| `two_sided_10000_orders` | 635.09 us | 649.02 us | 665.09 us | Runs 10,000 deterministic non-crossing orders |
| `two_sided_100000_orders` | 7.7559 ms | 8.2079 ms | 8.7132 ms | Runs 100,000 deterministic non-crossing orders |
| `cancel_from_10000_deep_level` | 4.0124 us | 4.0805 us | 4.1527 us | Cancels near the end of a 10,000-order FIFO price level |
| `modify_from_10000_deep_level` | 4.0671 us | 4.2050 us | 4.3777 us | Modifies near the end of a 10,000-order FIFO price level |
| `multi_symbol_100x1000_orders` | 7.1372 ms | 7.3221 ms | 7.5349 ms | Runs 100 books with 1,000 orders each |

## How To Read `cargo bench` Output

Example:

```text
two_sided_1000_orders   time:   [54.130 us 54.422 us 54.754 us]
                        change: [-5.3404% -4.1267% -2.9448%] (p = 0.00 < 0.05)
                        Performance has improved.
```

Meaning:

```text
two_sided_1000_orders
name of the benchmark function

time: [54.130 us 54.422 us 54.754 us]
estimated runtime range from Criterion

54.130 us
lower bound

54.422 us
middle estimate, usually the number we track

54.754 us
upper bound

change: [-5.3404% -4.1267% -2.9448%]
comparison against previous Criterion history on this same machine

p = 0.00 < 0.05
Criterion thinks the change is statistically significant

Performance has improved
the new run was faster than Criterion's previous local baseline
```

Important:

```text
lower time is better
ns is nanoseconds
us is microseconds
ms is milliseconds
1 us = 1,000 ns
1 ms = 1,000 us
```

When reading output, focus on:

```text
1. benchmark name
2. middle estimate
3. units: ns/us/ms
4. change direction
5. whether Criterion says it is noise or significant
6. outlier count
```

Do not overreact to one run:

```text
small benchmarks can move because of CPU frequency, scheduler noise, thermals,
background processes, and laptop/desktop power behavior.
```

## Throughput Estimate

These are rough estimates from the Criterion middle value.

| Benchmark | Approximate Throughput |
| --- | ---: |
| `two_sided_1000_orders` | 18.37 million orders/sec |
| `crossing_1000_orders` | 17.15 million orders/sec |
| `two_sided_10000_orders` | 15.41 million orders/sec |
| `two_sided_100000_orders` | 12.18 million orders/sec |
| `multi_symbol_100x1000_orders` | 13.66 million orders/sec |

Interpretation:

```text
The large synthetic workloads scale reasonably well for the current
correctness-first design.
```

Important caution:

```text
These are in-process benchmarks. They do not include networking, protocol
parsing, risk checks, persistence, thread handoff, or p99/p999 latency.
```

## Benchmark Notes

Criterion also printed local comparison messages:

```text
two_sided_1000_orders: Performance has improved
crossing_1000_orders: Change within noise threshold
add_one_resting_order: Performance has regressed
single_trade: No change in performance detected
multi_level_sweep: Change within noise threshold
cancel_order: Performance has improved
modify_order: Performance has improved
two_sided_10000_orders: Performance has improved
two_sided_100000_orders: Change within noise threshold
cancel_from_10000_deep_level: Performance has improved
modify_from_10000_deep_level: Performance has improved
multi_symbol_100x1000_orders: Performance has improved
```

Important:

```text
Those messages compare against previous local Criterion history on this machine.
For this phase, the table above is the saved baseline.
```

Outliers were present in multiple benchmarks. That is expected on a normal
developer machine because background processes, CPU frequency scaling, scheduler
noise, and thermals can affect nanosecond-level measurements.

The `1_000_000` resting-order workload is not part of this saved baseline yet.
That should be added later as a separate heavy benchmark so normal local
benchmark runs do not become too slow.

## Baseline Observations

Large workloads:

```text
1,000 non-crossing orders: 54.422 us
10,000 non-crossing orders: 649.02 us
100,000 non-crossing orders: 8.2079 ms
```

This is roughly linear, which is a good sign for the current synthetic workload.

Deep cancel/modify:

```text
cancel near end of 10,000-order level: 4.0805 us
modify near end of 10,000-order level: 4.2050 us
```

This confirms the expected weakness:

```text
order_sides finds the side quickly, but remove_order_from_side still scans
inside the FIFO queue at that price level.
```

Multi-symbol:

```text
100 symbols x 1,000 orders: 7.3221 ms
```

This is a simple simulation where each symbol is represented by an independent
`OrderBook`. It is useful as a baseline, but it is not a real multi-symbol
gateway or sharded matching architecture yet.

## Current Hot-Path Concerns

The current engine is correctness-first, not HFT-optimized.

Known concerns:

```text
add_order records events on every accepted order
event recording clones orders and trades
events: Vec<BookEvent> can grow without bound
cancel_order uses order_sides to find the side, but still scans inside the price level
modify_order removes and re-adds the order, which is simple but not yet optimized
BTreeMap is clean and correct, but may not be the fastest price ladder later
JSON output is useful for tools, but must stay outside the matching hot path
```

## Current Event Mode Status

Event mode config is implemented and tested.

```text
EventMode::Full records accepted/cancel/modify/trade events.
EventMode::TradesOnly records only trade events.
EventMode::Disabled records no events.
```

Why:

```text
Event replay is useful for debugging, audit, Windmill, and AI analysis.
But low-latency benchmarks should not always pay for event clones and an
unbounded in-memory event log.
```

Current design:

```text
OrderBookConfig {
    event_mode: EventMode
}

EventMode:
    Full
    TradesOnly
    Disabled
```

Expected result:

```text
Full mode keeps current replay/debug behavior.
Disabled mode gives a cleaner hot-path benchmark.
TradesOnly mode keeps execution output without recording every accepted command.
```

## Next Measurement

Next benchmark target:

```text
reduce add/match overhead in the PriceLevel design without losing correct active
liquidity reads
```

Reason:

```text
lazy cancel fixed deep cancel latency
PriceLevel.total_quantity fixed trader-facing pressure reads without separate
depth maps
add/match workloads still pay for BTreeMap price-level updates and HashMap order
storage
```

After implementation, run:

```bash
cargo test
cargo bench
```

Then compare especially:

```text
add_one_resting_order
single_trade_ref
multi_level_sweep_ref
two_sided_1000_orders
crossing_1000_orders
best_bid_from_price_level_quantity_10000_orders
```

The goal is to keep fast active-liquidity reads while reducing the cost paid on
every order add and every fill.
