# ob

A Rust multi-instrument limit-order-book service. The matching core is deterministic and synchronous; each configured symbol has a separate engine consumer and a fixed-capacity multi-producer Disruptor command ring. HTTP handlers publish commands to the appropriate ring through the `service` layer and never construct or mutate a book directly.

## Layout

- `src/domain` — order, symbol, price, quantity, and trade types plus validation.
- `src/matching` — arena-backed FIFO levels, matching rules, per-symbol engines, and exchange routing. Pure and synchronous: no transport, concurrency, or config knowledge.
- `src/service` — the only module outside `matching` allowed to hold an `Engine`/`Exchange`. Owns the per-symbol Disruptor command rings and exposes a transport-agnostic, async `ExchangeClient` (`book`/`submit`/`cancel`).
- `src/http` — Axum routes, request/response DTOs, and HTTP status mapping. Talks only to `service::ExchangeClient`.
- `src/config.rs` — the only module that reads `std::env`. Loads an optional `.env` file and returns a validated `Config`.
- `src/app.rs` — the composition root: loads `Config`, wires `matching::Exchange` → `service::spawn_exchange` → `http::router`, and runs the server.
- `src/main.rs` — process entrypoint; delegates to `app::run()`.
- `tests` — black-box matching and exchange behavior tests against `domain`/`matching` directly.

## Configuration

All configuration is read from the environment (see `.env.example`); nothing is hardcoded. Copy `.env.example` to `.env` and adjust as needed — `.env` is loaded automatically (missing file is fine) and explicit process environment variables always win.

| Variable | Default | Meaning |
| --- | --- | --- |
| `HOST` | `0.0.0.0` | Listen address |
| `PORT` | `8080` | Listen port |
| `SERVICE_NAME` | `ob` | Logged at startup |
| `LOG_LEVEL` | `info` | Logged at startup |
| `BOOK_CAPACITY` | `100000` | Resting-order capacity per symbol |
| `RING_CAPACITY` | `1024` | Per-symbol Disruptor command ring capacity; must be a power of two and at least 64 |
| `SYMBOLS` | `BTC-USD` | Comma-separated symbols prepared at startup |

## Run

```sh
cargo run
```

With the default configuration, the service listens on `0.0.0.0:8080` and configures `BTC-USD` at startup. It exposes:

- `GET /healthz`
- `GET /v1/matching/book?symbol=BTC-USD&depth=top`
- `GET /v1/matching/book?symbol=BTC-USD&depth=10`
- `GET /v1/matching/book?symbol=BTC-USD&depth=full`
- `POST /v1/matching/order`
- `DELETE /v1/matching/order/{id}?symbol=BTC-USD`

Submit a limit order with:

```json
{
  "symbol": "BTC-USD",
  "id": 1,
  "user_id": 7,
  "original_qty": 100,
  "open_qty": 100,
  "accepted_sequence": 0,
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

Market-order remainders are cancelled (never rested). A full command ring returns HTTP `503`. The development consumer wait strategy sleeps for one millisecond while idle.
