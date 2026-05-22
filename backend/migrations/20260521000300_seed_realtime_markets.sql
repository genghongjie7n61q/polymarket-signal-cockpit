WITH btc_asset AS (
    INSERT INTO assets (symbol, name)
    VALUES ('BTC', 'Bitcoin')
    ON CONFLICT (symbol) DO UPDATE SET name = EXCLUDED.name
    RETURNING id
),
eth_asset AS (
    INSERT INTO assets (symbol, name)
    VALUES ('ETH', 'Ethereum')
    ON CONFLICT (symbol) DO UPDATE SET name = EXCLUDED.name
    RETURNING id
)
INSERT INTO markets (market_key, asset_id, symbol, interval_seconds, source)
SELECT 'btc5m', id, 'BTC-USD', 300, 'polymarket'
FROM btc_asset
ON CONFLICT (market_key) DO UPDATE SET
    asset_id = EXCLUDED.asset_id,
    symbol = EXCLUDED.symbol,
    interval_seconds = EXCLUDED.interval_seconds,
    source = EXCLUDED.source;

WITH eth_asset AS (
    SELECT id FROM assets WHERE symbol = 'ETH'
)
INSERT INTO markets (market_key, asset_id, symbol, interval_seconds, source)
SELECT 'eth15m', id, 'ETH-USD', 900, 'polymarket'
FROM eth_asset
ON CONFLICT (market_key) DO UPDATE SET
    asset_id = EXCLUDED.asset_id,
    symbol = EXCLUDED.symbol,
    interval_seconds = EXCLUDED.interval_seconds,
    source = EXCLUDED.source;
