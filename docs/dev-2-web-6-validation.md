# WEB-6 dev-2 Validation

This validation must run on `dev-2`, not on the local Mac.

## Prepare

```bash
ssh dev-2
cd /opt/polymarket-signal-cockpit
git fetch origin codex/web-6-storage-foundation
git checkout codex/web-6-storage-foundation
```

## Start PostgreSQL

```bash
podman-compose up -d postgres
podman-compose ps
podman-compose logs --tail=80 postgres
```

## Run Tests

Use the dev-2 database URL from the dev-2 `.env` file. The example below matches the current dev-2 local validation password.

```bash
export DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@127.0.0.1:5432/polymarket
cargo test -p polymarket-backend storage_migrations_tests -- --nocapture
cargo test -p polymarket-backend storage_repository_tests -- --nocapture
cargo test -p polymarket-backend storage_writer_tests -- --nocapture
cargo test -p polymarket-backend
```

## Container Runtime Check

```bash
podman-compose up -d --build backend
curl -fsS http://192.168.103.157:8080/healthz
podman-compose logs --tail=120 backend
```

## Cleanup

If the environment was started only for validation:

```bash
podman-compose down
```

If volumes were created only for disposable validation:

```bash
podman volume ls | grep polymarket
```

Delete only volumes confirmed to belong to this temporary validation run. If the PostgreSQL/backend containers are intentionally retained as the shared dev-2 environment, record that in Linear instead of claiming cleanup.
