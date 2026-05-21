# dev-2 Deployment Notes

Target deployment host: `dev-2`.

Recommended deployment layout:

```text
/opt/polymarket-signal-cockpit/
  docker-compose.yml
  .env
  data/
  logs/
```

Initial services:

```text
postgres
backend
web
```

PostgreSQL should run in a container with persistent volume. The backend should run as a container built from this repository. The web app can be served by the backend initially or deployed as a separate container once the cockpit UI is implemented.

Required environment variables:

```text
DATABASE_URL=postgres://...
FEISHU_WEBHOOK_URL=...
RUST_LOG=info
```

The current Python prototype can be used on dev-2 only as a reference runner. The target architecture should replace it with a Rust realtime backend and a Web cockpit.
