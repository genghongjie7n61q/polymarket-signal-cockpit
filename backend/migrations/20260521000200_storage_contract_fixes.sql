DROP INDEX IF EXISTS idx_raw_market_events_idempotency;
DROP INDEX IF EXISTS idx_ticks_idempotency;

WITH ranked_sequence_ticks AS (
    SELECT
        id,
        row_number() OVER (
            PARTITION BY market_id, source, sequence
            ORDER BY received_at DESC, source_ts DESC, id DESC
        ) AS row_rank
    FROM ticks
    WHERE sequence IS NOT NULL
)
DELETE FROM ticks
USING ranked_sequence_ticks
WHERE ticks.id = ranked_sequence_ticks.id
  AND ranked_sequence_ticks.row_rank > 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_raw_market_events_source_event_id
    ON raw_market_events (source, source_event_id)
    WHERE source_event_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_ticks_sequence_idempotency
    ON ticks (market_id, source, sequence)
    WHERE sequence IS NOT NULL;
