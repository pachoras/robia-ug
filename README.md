# robia-ug

A Rust web server for Robia Labs Ltd, built with [Axum](https://github.com/tokio-rs/axum). Serves a landing page and an authenticated loan application page, backed by PostgreSQL.

## Stack

- **Web framework**: Axum 0.8 (async, multi-threaded via Tokio)
- **Database**: PostgreSQL via SQLx (with compile-time query checking)
- **Templating**: Tera (Jinja2-style HTML templates)
- **Styling**: Sass → CSS (compiled at build time), with SHA-256 cache busting
- **Middleware**: request tracing, CORS, Brotli compression (tower-http)

## Routes

| Method | Path      | Auth required | Description              |
| ------ | --------- | ------------- | ------------------------ |
| GET    | `/`       | No            | Landing page             |
| GET    | `/app`    | Yes (Bearer)  | Loan application page    |
| GET    | `/static` | No            | Static assets (CSS, etc) |

Authentication uses a `Bearer` token passed in the `Authorization` header. Tokens are validated against the `user_auth_tokens` table.

## Database

Migrations are run automatically on startup. The schema (see [migrations/](migrations/)) creates tables

## Prerequisites

- Rust (edition 2024)
- PostgreSQL
- `sass` CLI (`npm install -g sass`)
- S3 Compatible object store. We are using [RustFS](https://rustfs.com/) for this project. You can install it using:

```bash
mkdir -p $HOME/rustfs/data
chown -R 10001:10001 $HOME/rustfs/data

docker run -d \
  --name rustfs_container \
  -p 9000:9000 \
  -p 9001:9001 \
  -v $HOME/rustfs/data:/data \
  -e RUSTFS_ACCESS_KEY=rustfsadmin \
  -e RUSTFS_SECRET_KEY=rustfsadmin \
  -e RUSTFS_CONSOLE_ENABLE=true \
  -e RUSTFS_SERVER_DOMAINS=localhost \
  rustfs/rustfs:latest \
  /data
```

## Development

1. Edit and set the environment variables in [`.env`](.env):

   ```sh
   set -a
   source .env
   set +a
   ```

2. Create the database (also runs migrations)
   ```sh
   cargo sqlx database create
   ```
3. Build and run:

   ```sh
   cargo run && sass --watch src/static/css/styles.scss:src/static/css/styles.css
   ```

   The server listens on `0.0.0.0:8000`. Sass is compiled to CSS automatically during the build via `build.rs`.

## Running tests

```sh
cargo test
```

Tests cover route handlers, error responses, auth token models, and utility functions. Integration tests that hit the database require a running PostgreSQL instance at the `DATABASE_URL`.
