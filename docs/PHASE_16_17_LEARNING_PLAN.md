# Phase 16 And 17: Protocol And Latency Work

We are working on the codec and measurements before finishing Phases 13-15.
That does not make the gateway usable for live orders: it still needs the
engine runtime and a clear durability policy.

## Paused Work And Branches

The pushed `phase-13-engine-commands` branch contains the command/event types
and a synchronous `EngineProcessor`. Its dedicated engine thread, bounded
queues, multi-instrument routing, and timing work remain open. Phases 14
(journal/recovery) and 15 (Kafka/PostgreSQL projections) have not started.

`phase-16-binary-protocol` branches independently from `master`. Its first
mergeable slice is an offline binary codec plus tests and benchmarks; it does
not depend on the unfinished Phase 13 branch. After merging that slice, create
a separate `phase-17-latency-lab` branch from updated `master` for experiments.
Later TCP and network experiments get their own short branches from updated
`master`. Leave the paused branch intact until we resume it.

The target order for these labs is:

```text
Phase 16A: fixed-layout request codec and validation
-> Phase 17A: baseline, profiling, allocation and SIMD experiments
-> Phase 16B: local decode-only TCP transport experiment
-> Phase 17B: network and CPU-placement experiments
-> resume Phase 13: single writer, bounded queues, instrument routing
-> Phase 14: journal and durable acknowledgement
-> Phase 15: Kafka and database projections
-> complete the production-style Phase 16 gateway and Phase 17 evaluation
```

## How To Work Through It

For each step, note what it does, why it is needed, and how to test it.
Finish one small change before starting another. Measure a bottleneck before
adding a library or unsafe optimization to address it.

## Phase 16A: Codec (Independent, First)

Read [`BINARY_ORDER_PROTOCOL.md`](BINARY_ORDER_PROTOCOL.md) before coding. Its
56-byte request frame fixes byte order, offsets, message variants, and strict
validation rules. Do not use Rust struct layout as a wire format.

| Step | What to build | Why it matters | Done when |
|---|---|---|---|
| 1 | Header constants and typed `MessageType` | Gives one canonical byte contract | All offsets and sizes match the protocol document |
| 2 | `decode(&[u8]) -> Result<WireCommand, DecodeError>` | Keeps untrusted parsing isolated from the engine | Add/Cancel/Modify and malformed inputs are covered |
| 3 | `encode(&WireCommand) -> [u8; 56]` | Makes round trips and byte fixtures possible | Encoder emits canonical reserved fields and header |
| 4 | Round-trip and golden-byte tests | Detects accidental protocol drift | Known bytes decode and re-encode identically |
| 5 | Binary vs JSON decode benchmarks | Quantifies the cost of representation | Inputs are equivalent and setup is outside timed loops |

Know why these matter: TCP is a byte stream; network byte order;
fixed offsets versus `repr(C)` padding; checked length before slicing;
`u64::from_be_bytes`; rejecting unknown versions; canonical unused bytes;
and the difference between a wire version and an engine event schema version.

An instrument UUID stays in `WireCommand`. The current `EngineCommand` has
no instrument field, so do not invent a hard-coded shard mapping in the
codec. The future gateway maps a validated instrument to its owning shard.

## Phase 17A: Measure The Existing Engine

Use [`PERFORMANCE_BASELINE.md`](PERFORMANCE_BASELINE.md) as historical context,
then capture a new baseline with the same machine, build profile, workload,
event mode, and measurement method before and after each experiment.

1. Measure add, single trade, multi-level sweep, deep cancel/modify, and a
   larger book. Record the workload shape and whether event recording is on.
2. Add a repeatable latency distribution runner for p50, p95, p99, p999, max,
   and throughput. Criterion's aggregate timing alone is not enough for
   per-command tail latency. Warm up first and separate setup from operation.
3. Profile CPU hotspots and allocation counts. Change one thing at a time;
   rerun correctness tests and the same benchmark before keeping a change.
4. Try preallocated output/event buffers if allocation is significant.
   Record how buffer capacity affects memory and tail latency.
5. Test SIMD only on a measured batch-oriented operation such as validating
   many independent fields or processing market-data arrays. Matching is
   branch-heavy and order-dependent; SIMD there needs evidence, not a slogan.

Review cache locality, allocations, false sharing, branch
prediction, benchmark setup costs, throughput versus tail latency, CPU
frequency/noise, and why a result from one workload does not generalize to
every book shape. A GPU is not part of the order-by-order matching hot path.

## Phase 16B: Local TCP Transport Experiment

After the pure codec works, make one persistent local TCP connection that
reads the header and then exactly the remaining payload bytes. A successful
response in this independent experiment means **decoded successfully**, not
"order executed" or "durable". Define a versioned response frame before
coding that response. Compare direct decode time with TCP receive/decode/
response time and test fragmented and coalesced writes.

Multiple clients, authentication, ownership, backpressure, and actual order
execution wait for the Phase 13 runtime and gateway integration. Keep the
prototype bound to loopback and clearly mark it as a transport lab.

## Phase 17B: Advanced Experiments

Once a TCP baseline exists, investigate CPU affinity, isolated cores, NUMA,
busy polling, cache-line padding, and allocation limits one at a time. Record
both improvement and downside. SIMD remains workload-specific. Huge pages
need a measured memory/TLB bottleneck. Kernel bypass/DPDK is research until
ordinary TCP is shown to be a network bottleneck and suitable NIC/hardware
access is available. It is not a prerequisite for learning low-latency Rust.

Do not compare an in-process matching nanosecond result to end-to-end network
latency. Report receive, decode, queue (once Phase 13 exists), matching, and
response stages separately.

## What The Labs Do Not Finish

Phase 16's original gateway exit criteria require a real bounded engine queue
and a response with meaningful execution/acknowledgement semantics. Phase
17's full evaluation requires that path plus the Phase 14 durability modes.
The labs can be merged earlier, but they do not complete either phase.

At each lab merge: run formatting, Clippy, focused tests and relevant
benchmarks; record machine/workload details; update the protocol or baseline
document; and keep unrelated features off the branch.
