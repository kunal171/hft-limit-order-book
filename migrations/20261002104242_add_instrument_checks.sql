-- Enforce in the database what the API already validates, so no other
-- write path can store an instrument the engine cannot trade.
ALTER TABLE instruments
    ADD CONSTRAINT instruments_price_scale_range CHECK (price_scale BETWEEN 0 AND 18),
    ADD CONSTRAINT instruments_quantity_scale_range CHECK (quantity_scale BETWEEN 0 AND 18),
    ADD CONSTRAINT instruments_tick_size_positive CHECK (tick_size > 0),
    ADD CONSTRAINT instruments_lot_size_positive CHECK (lot_size > 0),
    ADD CONSTRAINT instruments_assets_differ CHECK (base_asset <> quote_asset);
