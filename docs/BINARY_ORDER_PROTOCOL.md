# Binary Order Protocol Lab (Version 1)

This document specifies the first wire format to implement in Phase 16. It is
an offline codec and benchmark contract. No TCP order-entry service or live
trading is implemented by this document.

The format is deliberately small and fixed-size so we can learn byte parsing,
validate untrusted input, and compare binary decoding with JSON. The field
layout is a learning protocol, not a claim of exchange interoperability.

## Boundary And Assumptions

```text
TCP bytes -> frame decoder -> validated command -> future gateway/sequencer
                                             -> future engine runtime
```

The codec does not authenticate users, assign engine sequence numbers, decide
whether an account may trade, or acknowledge order execution. The existing
`EngineCommand` describes an operation after decoding. Its `SequencedCommand`
wrapper receives `shard_id` and `sequence` from later runtime components;
neither field is sent by a client.
The current `Order` has no account owner, so this lab must not be exposed as a
public order-entry API. The prototype in this phase, if built, is local and
single-connection. Its response can acknowledge **decoding only**; it cannot
claim that an order was executed or stored durably.

## Byte Layout

Every request is exactly **56 bytes**. Multibyte integers are unsigned and
**big-endian** (network byte order). Offsets are zero-based; the ranges below
are inclusive. There is no Rust struct-to-bytes cast or platform-dependent
padding. Decode fields explicitly from byte slices.

| Bytes | Size | Field | Meaning |
|---|---:|---|---|
| 0-1 | 2 | magic | `0x4C4F` (`LO` in ASCII) |
| 2 | 1 | wire_version | `1` |
| 3 | 1 | message_type | `1` Add, `2` Cancel, `3` Modify |
| 4-5 | 2 | frame_length | `56`, including header and payload |
| 6-7 | 2 | flags | `0`; reserved for a later version |
| 8-23 | 16 | instrument_id | UUID bytes in canonical `Uuid::as_bytes()` order |
| 24-31 | 8 | order_id | Nonzero `u64`; unique within its instrument |
| 32-39 | 8 | price | Integer price units, `u64` |
| 40-47 | 8 | quantity | Integer quantity units, `u64` |
| 48 | 1 | side | `1` Buy, `2` Sell, `0` not applicable |
| 49-55 | 7 | reserved | Must all be zero |

`instrument_id` corresponds to a UUID in the PostgreSQL control plane. Use
`Uuid::as_bytes()` when encoding and `Uuid::from_bytes()` when decoding; this
also handles the version 7 UUIDs currently created by the API. The
codec only checks its shape and rejects the nil UUID; a later gateway checks
that the instrument exists, is active, and has a local configuration snapshot.
Using UUID bytes avoids an undefined mapping between a short wire ID and the
database UUID. This is a correctness-first format; a smaller stable numeric
handle is a later measured optimization.

The first eight bytes of an Add request are:

```text
4C 4F 01 01 00 38 00 00
|magic|v|kind|length|flags|
```

`0x0038` is 56 in hexadecimal. For example, a price of `100_000` is
`00 00 00 00 00 01 86 A0` on the wire. Decimal prices, floating-point
values, and text symbols are not transmitted in this frame.

## Message Rules

| Type | Required fields | Canonical unused fields |
|---|---|---|
| Add (`1`) | Valid side (`1`/`2`), price > 0, quantity > 0 | None |
| Cancel (`2`) | Existing instrument/order IDs | price = 0, quantity = 0, side = 0 |
| Modify (`3`) | Existing instrument/order IDs, price > 0, quantity > 0 | side = 0; original side comes from the resting order |

Every message requires nonzero `order_id`, non-nil `instrument_id`, zero flags,
and zero reserved bytes. Reject unknown types, versions, sides, lengths, or
noncanonical unused fields. Never silently ignore a nonzero reserved field:
that could make two endpoints interpret the same bytes differently.

The wire codec validates representation and basic field ranges. Tick size,
lot size, market status, ownership, replay/duplicate policy, and risk limits
require instrument/account state and belong to later gateway validation. The
existing `OrderBook` has its own narrower validation; do not treat it as a
complete network-input validator.

The wire version is independent of `ENGINE_SCHEMA_VERSION`. A protocol can
change without changing engine events, and engine events can change without
changing the client wire format. Version 1 supports only the fields above;
future incompatible layouts use a new version, not a changed interpretation
of these 56 bytes.

## TCP Framing And Decoder Policy

TCP is a byte stream: one `read` may return half a frame or several frames.
Read exactly eight header bytes, validate magic/version/length/flags, then
read exactly the remaining 48 bytes. Do not allocate based on an untrusted
length. For a malformed header or payload, return a typed decode error and
close the prototype connection; there is no attempt to resynchronize.

The first implementation should be a pure function from `&[u8]` to a typed
validated wire command. It should do no I/O, database queries, logging, or
matching. A future encoder should produce the exact same canonical bytes.
Do not use `unsafe`, `transmute`, or `repr(C)` for this codec. Explicit
`from_be_bytes`/`to_be_bytes` makes the format independent of CPU endianness
and Rust struct layout.

## Learning And Verification Order

1. Define constants for the 8-byte header and 56-byte frame, plus typed
   `MessageType`, `WireCommand`, and `DecodeError`.
2. Decode the header and each payload field by fixed offsets. Convert to
   `EngineCommand` only after validation. Retain `instrument_id` separately:
   `EngineCommand` does not yet carry it.
3. Implement a matching encoder and check `decode(encode(command)) == command`
   for Add, Cancel, and Modify.
4. Test short buffers, extra bytes, wrong magic/version/length/flags, invalid
   type/side, nil instrument, zero order ID, zero Add/Modify price or quantity,
   and nonzero reserved/unused fields.
5. Benchmark binary decode, JSON decode, and encode separately with prepared
   input outside the timed loop. Verify both inputs represent the same command.
6. Only after the codec is stable, add a local single-connection TCP experiment.
   Document its response format and decode-only acknowledgement before using
   it to measure request/response time.

Keep codec tests deterministic. No socket, database, or Docker service is
required to run them. Record throughput and p50/p95/p99/p999 for the later
network experiment; a Criterion mean alone does not describe tail latency.

## Deferred Dependencies

- Phase 13: a dedicated single-writer thread, bounded queues, queue-full
  policy, instrument-to-book routing, and stage-level queue timing.
- Phase 14: a durable journal and crash-safe acknowledgement semantics.
- Phase 15: asynchronous Kafka publication and PostgreSQL projections.
- Phase 17: CPU/NUMA placement, allocation/false-sharing experiments, and
  kernel-bypass research after ordinary TCP is measured.

The lab can be merged independently. It must not be described as completing
the full Phase 16 gateway until the deferred runtime boundaries are in place.
