-- Add migration script here

-- Enum for instrument category.
CREATE TYPE asset_class AS ENUM (
    'crypto',
    'stock',
    'futures',
    'commodities'
);

-- Enum for whether this market can currently accept orders.
CREATE TYPE market_status AS ENUM (
    'active',
    'paused',
    'delisted'
);


-- Instruments are the markets/assets we can trade.
-- Example: BTC-USDT, ETH-USDT, AAPL-USD.

CREATE TABLE instruments (
    id UUID PRIMARY KEY,
    
    -- Human-readable market symbol.
    symbol TEXT NOT NULL UNIQUE,

    -- Example: crypto, stock, futures, etc.
    asset_class asset_class NOT NULL,

    -- Example: BTC in BTC-USDT.
    base_asset TEXT NOT NULL,

    -- Example: USDT in BTC-USDT.
    quote_asset TEXT NOT NULL,

    -- Store money/price as integers, never floats.
    -- If price_scale = 2, then 18234 means 182.34.
    price_scale INTEGER NOT NULL,

    -- If quantity_scale = 6, then 1000000 means 1.000000.
    quantity_scale INTEGER NOT NULL,

    -- Smallest allowed price movement.
    tick_size BIGINT NOT NULL,

    -- Smallest allowed quantity movement.
    lot_size BIGINT NOT NULL,

    -- active, paused, delisted.
    status market_status NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

