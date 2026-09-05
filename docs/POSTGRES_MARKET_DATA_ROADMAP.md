# Postgres, Users, And Market Pricing Roadmap

This document explains how PostgreSQL, users/traders, instruments, and
reference/oracle pricing should fit into the limit order book project.

Core rule:

```text
Postgres is for persistence, analytics, dashboards, replay, and audit.
Postgres is not used for every matching decision in the hot path.
```

The matching engine should stay in memory. After an order is processed, the
system can persist commands, trades, snapshots, metrics, and run metadata.

## Why Add Postgres

Postgres makes the project feel closer to a real backend/trading system.

Use Postgres for:

```text
simulation run history
orders submitted during a run
executed trades
book snapshots
benchmark and latency reports
users/traders/accounts
instruments such as BTC-USDT or AAPL-USD
reference/oracle prices
Windmill dashboards
AI analysis inputs later
```

Do not use Postgres for:

```text
best bid lookup
best ask lookup
price-time matching
cancel/modify hot-path lookup
per-order synchronous logging in low-latency mode
```

Why:

```text
A database call has network, lock, and disk costs. The order book needs
predictable in-memory latency.
```

## Important Concepts

### User, Trader, Account, Session

A real trading system usually separates these ideas:

```text
user      -> human or organization identity
account   -> trading account or portfolio
trader    -> actor allowed to submit orders for an account
session   -> connection/login/API key session submitting commands
```

For this project, MVP can start smaller:

```text
users
accounts
orders.user_id
orders.account_id
```

Why we need this:

```text
orders and trades should show who submitted them
risk limits are applied per user/account
analytics can answer who traded what
later APIs need authentication and ownership checks
```

### Instrument

An instrument is the thing being traded.

Examples:

```text
BTC-USDT
ETH-USD
AAPL-USD
SOL-USDC
```

Why this matters:

```text
Different instruments have different tick sizes, quantity steps, symbols,
price scales, and trading status.
```

Important fields:

```text
symbol
asset_class
base_asset
quote_asset
price_scale
quantity_scale
tick_size
lot_size
status
```

### Reference Price / Oracle Price

The order book price comes from orders and trades.

The reference/oracle price comes from an external trusted source.

Use oracle/reference price for:

```text
mark price
slippage analysis
sanity checks
risk checks
fair-value comparison
portfolio valuation
detecting stale or abnormal markets
```

Do not use oracle/reference price to:

```text
override price-time matching
change the trade price after a match
force the book to match at oracle price
```

Why:

```text
Matching must follow the submitted limit orders. The oracle is supporting data,
not the matching rule.
```

For stocks, "oracle" usually means:

```text
reference market-data provider
exchange feed
NBBO/consolidated quote style data
```

For crypto, "oracle" can mean:

```text
exchange market-data feed
aggregated price feed
on-chain oracle such as a smart-contract price feed later
```

## Integer Pricing

Do not store prices as floating point values.

Good:

```text
price_value = 1002500
price_scale = 4
meaning = 100.2500
```

Bad:

```text
price = 100.25 as f64
```

Why:

```text
floating point values can introduce rounding errors
financial systems need exact integer arithmetic
different assets need different decimal precision
```

For stocks:

```text
AAPL at 182.34 USD can be stored as 18234 with price_scale = 2
```

For crypto:

```text
BTC-USDT at 64123.4567 can be stored as 641234567 with price_scale = 4
```

For this project:

```text
OrderBook still uses integer ticks internally.
Postgres stores the scale needed to display/interpret those ticks.
```

## Suggested Tables

This is a design guide. Do not add all tables at once. Build one thin slice at a
time.

### users

Purpose:

```text
store who can submit orders
```

Sketch:

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY,
    display_name TEXT NOT NULL,
    email TEXT UNIQUE,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Why:

```text
orders need ownership
future APIs need authentication and authorization
```

### accounts

Purpose:

```text
represent a trading account owned by a user
```

Sketch:

```sql
CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Why separate from users:

```text
one user can have multiple accounts
risk limits and balances are usually account-level
```

### instruments

Purpose:

```text
store what can be traded
```

Sketch:

```sql
CREATE TABLE instruments (
    id BIGSERIAL PRIMARY KEY,
    symbol TEXT NOT NULL UNIQUE,
    asset_class TEXT NOT NULL,
    base_asset TEXT NOT NULL,
    quote_asset TEXT NOT NULL,
    price_scale INTEGER NOT NULL,
    quantity_scale INTEGER NOT NULL,
    tick_size BIGINT NOT NULL,
    lot_size BIGINT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Why:

```text
the same engine can later support many markets
tick_size prevents invalid prices
lot_size prevents invalid quantities
```

### runs

Purpose:

```text
store one simulation/backtest/benchmark run
```

Sketch:

```sql
CREATE TABLE runs (
    id UUID PRIMARY KEY,
    scenario TEXT NOT NULL,
    instrument_id BIGINT REFERENCES instruments(id),
    order_count BIGINT NOT NULL,
    status TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ
);
```

Why:

```text
all orders/trades/snapshots/metrics should belong to a run
```

### orders

Purpose:

```text
persist order commands/results for audit and analysis
```

Sketch:

```sql
CREATE TABLE orders (
    id BIGINT PRIMARY KEY,
    run_id UUID REFERENCES runs(id),
    user_id UUID REFERENCES users(id),
    account_id UUID REFERENCES accounts(id),
    instrument_id BIGINT NOT NULL REFERENCES instruments(id),
    side TEXT NOT NULL,
    price_ticks BIGINT NOT NULL,
    original_quantity BIGINT NOT NULL,
    remaining_quantity BIGINT NOT NULL,
    status TEXT NOT NULL,
    sequence_number BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Useful index:

```sql
CREATE INDEX orders_run_sequence_idx ON orders (run_id, sequence_number);
CREATE INDEX orders_account_created_idx ON orders (account_id, created_at);
```

Why:

```text
sequence_number gives deterministic replay order
account index helps query a user's trading activity
```

### trades

Purpose:

```text
persist executions produced by the matching engine
```

Sketch:

```sql
CREATE TABLE trades (
    id BIGSERIAL PRIMARY KEY,
    run_id UUID REFERENCES runs(id),
    instrument_id BIGINT NOT NULL REFERENCES instruments(id),
    maker_order_id BIGINT NOT NULL,
    taker_order_id BIGINT NOT NULL,
    price_ticks BIGINT NOT NULL,
    quantity BIGINT NOT NULL,
    sequence_number BIGINT NOT NULL,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Useful index:

```sql
CREATE INDEX trades_run_sequence_idx ON trades (run_id, sequence_number);
CREATE INDEX trades_instrument_time_idx ON trades (instrument_id, executed_at);
```

Why:

```text
trades are the main output of the matching engine
time and sequence indexes support analytics
```

### book_snapshots

Purpose:

```text
store final or periodic book state for dashboards/debugging
```

Sketch:

```sql
CREATE TABLE book_snapshots (
    id BIGSERIAL PRIMARY KEY,
    run_id UUID REFERENCES runs(id),
    instrument_id BIGINT NOT NULL REFERENCES instruments(id),
    best_bid_ticks BIGINT,
    best_ask_ticks BIGINT,
    spread_ticks BIGINT,
    total_bid_quantity BIGINT NOT NULL,
    total_ask_quantity BIGINT NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Why:

```text
dashboards should not have to rebuild every book state from raw events
```

### run_metrics

Purpose:

```text
store summary metrics for one run
```

Sketch:

```sql
CREATE TABLE run_metrics (
    run_id UUID PRIMARY KEY REFERENCES runs(id),
    trade_count BIGINT NOT NULL,
    total_traded_quantity BIGINT NOT NULL,
    total_notional BIGINT NOT NULL,
    last_trade_price_ticks BIGINT,
    vwap_ticks BIGINT,
    imbalance_numerator BIGINT,
    imbalance_denominator BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Why store VWAP as ticks:

```text
keeps metric output integer-based and avoids f64 rounding in persisted data
```

### reference_prices

Purpose:

```text
store stock/crypto reference prices from external sources
```

Sketch:

```sql
CREATE TABLE reference_prices (
    id BIGSERIAL PRIMARY KEY,
    instrument_id BIGINT NOT NULL REFERENCES instruments(id),
    source TEXT NOT NULL,
    price_ticks BIGINT NOT NULL,
    confidence_bps INTEGER,
    observed_at TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    status TEXT NOT NULL
);
```

Useful index:

```sql
CREATE INDEX reference_prices_latest_idx
ON reference_prices (instrument_id, source, observed_at DESC);
```

Why:

```text
we can compare executed prices against a trusted external market price
we can detect stale or delayed prices
we can build slippage and mark-price analysis later
```

Important validation:

```text
if received_at - observed_at is too large, mark the price as stale
if source reports bad status, do not use it for risk decisions
if price deviates too much from recent values, flag it for review
```

## Application Architecture

Suggested module layout later:

```text
src/
  engine/              hot-path order book
  domain/              shared order/trade/instrument types
  persistence/
    postgres.rs        writes completed run data to Postgres
    models.rs          database row structs
  market_data/
    oracle.rs          reference price trait
    provider.rs        provider implementations later
  risk/
    checks.rs          tick size, lot size, max order size, stale price checks
```

Important boundary:

```text
engine does not depend on sqlx
persistence depends on engine/domain output
market_data feeds reference prices into risk/analytics
```

This dependency direction keeps the core order book simple:

```text
engine -> no database knowledge
persistence -> knows how to store engine output
```

## Oracle/Reference Price Interface

Start with a trait before picking a provider:

```rust
pub trait ReferencePriceProvider {
    fn latest_price(&self, instrument: &str) -> Result<ReferencePrice, PriceError>;
}

pub struct ReferencePrice {
    pub symbol: String,
    pub price_ticks: i64,
    pub source: String,
    pub observed_at_ms: i64,
}
```

Why trait first:

```text
tests can use fake prices
crypto can use one provider
stocks can use another provider
the engine does not care where the reference price came from
```

Later async version:

```rust
#[async_trait::async_trait]
pub trait AsyncReferencePriceProvider {
    async fn latest_price(&self, instrument: &str) -> Result<ReferencePrice, PriceError>;
}
```

Keep this outside matching:

```text
price feed updates reference_prices table
risk/analytics reads latest reference price
matching engine still matches submitted orders by price-time priority
```

## Build Order

Do not build everything at once.

Recommended order:

```text
1. Add docs and schema design.
2. Add sqlx + Postgres connection.
3. Add migrations for instruments and runs.
4. Persist run summary after simulation completes.
5. Persist trades for a run.
6. Add users and accounts.
7. Attach orders/trades to user/account.
8. Add reference_prices table.
9. Add fake oracle provider for tests.
10. Add real provider integration later.
11. Add Windmill dashboard queries.
12. Let AI read Postgres run history later.
```

Why this order:

```text
first persist completed runs
then add ownership
then add external pricing
then build dashboards/AI on top
```

## First MVP Slice

The first Postgres slice should be intentionally small:

```text
connect to Postgres
create instruments table
create runs table
create trades table
persist one completed simulation run
query recent runs
```

Avoid initially:

```text
authentication
real money balances
real-time oracle providers
production-grade market data feeds
complex risk engine
```

This keeps the project moving and still teaches the right backend ideas.

## Interview Explanation

Good answer:

```text
The order book itself stays in memory because matching needs deterministic low
latency. Postgres stores the durable record: users, instruments, runs, orders,
trades, snapshots, metrics, and reference prices. External prices are used for
risk and analytics, not for changing the matching result.
```

Key trade-off:

```text
Putting Postgres in the hot path gives stronger immediate durability, but it
adds latency and lock contention. For this project, we keep matching in memory
and persist results outside the direct matching function.
```
