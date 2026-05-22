ALTER TABLE backtest_runs
ADD COLUMN IF NOT EXISTS parameters JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE backtest_runs br
SET parameters = mv.parameters
FROM model_versions mv
WHERE br.model_version_id = mv.id
  AND br.parameters = '{}'::jsonb;
