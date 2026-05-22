# Storage Migration Preflight

Run this checklist before applying storage migrations to any database that is not a disposable validation database.

The dev-2 validation database can be treated as disposable only when this has been explicitly recorded in the task handoff. For any database that may contain useful historical data, do not run migrations until the checks below are complete.

## Duplicate Tick Check

Migration `20260521000200_storage_contract_fixes.sql` replaces the original tick idempotency index with a sequence-only partial index. Before running it against a non-empty database, check whether sequenced duplicate ticks exist:

```sql
SELECT
    market_id,
    source,
    sequence,
    count(*) AS duplicate_count
FROM ticks
WHERE sequence IS NOT NULL
GROUP BY market_id, source, sequence
HAVING count(*) > 1
ORDER BY duplicate_count DESC;
```

If this query returns rows, stop and decide whether the database is disposable, whether duplicates should be archived, or whether a manual merge is required. Do not let the migration silently discard useful data.

## Suggested Backup

For a non-disposable database, create a backup before storage contract migrations:

```bash
pg_dump "$DATABASE_URL" \
  --format=custom \
  --file="/secure/backup/path/polymarket-before-storage-contract-fixes.dump"
```

The backup path must be outside the repository and must not be committed.

## dev-2 Validation Record

For dev-2, record the duplicate count and whether the database was treated as disposable in the issue or validation document.
