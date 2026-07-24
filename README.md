# ob

A Rust limit-order-book service. The core is deterministic and synchronous:
each command is applied by one `Engine`, while the HTTP layer serializes access
to it. This keeps price-time priority and book mutations easy to reason about.

## Layout

- `src/domain` — order and trade types plus validation.
- `src/matching` — arena-backed FIFO levels, matching rules, and the command
  boundary.
- `src/api` — Axum routes and HTTP error mapping.
- `src/config.rs` — environment-backed runtime configuration.
- `tests` — black-box matching behavior tests.

## Run

```sh
cp .env.example .env
ENV=local cargo run
```

The service listens on `HOST:PORT` (by default `0.0.0.0:8080`). It exposes
`GET /healthz`, `GET /v1/matching/status`, `GET /v1/matching/book`,
`POST /v1/matching/order`, and `DELETE /v1/matching/order/{id}`.

Submit a limit order with:

```json
{
  "id": 1,
  "user_id": 7,
  "quantity": 100,
  "side": "buy",
  "kind": "limit",
  "limit_price": 10000
}
```

## Development

```sh
make fmt
make lint
make test
```

`BOOK_CAPACITY` limits the number of simultaneously resting orders for this
single-book service. Market-order remainders are cancelled (never rested).
