# ob

A Rust multi-instrument limit-order-book service. The matching core is deterministic and synchronous; each configured symbol has a separate engine consumer and a fixed-capacity multi-producer Disruptor command ring. HTTP handlers publish commands to the appropriate ring and never mutate a book directly.

## Layout

- `src/domain` — order, symbol, price, quantity, and trade types plus validation.
- `src/matching` — arena-backed FIFO levels, matching rules, per-symbol engines, and exchange routing.
- `src/api.rs` — Axum routes, HTTP mapping, per-symbol command-ring routing, and replies.
- `src/main.rs` — runtime wiring, configured symbols, and process lifecycle.
- `tests` — black-box matching and exchange behavior tests.

## Run

```sh
cargo run
```

The service listens on `0.0.0.0:8080`. It configures `BTC-USD` at startup and exposes:

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

`BOOK_CAPACITY` is currently configured at startup as 100,000 resting orders per symbol. Market-order remainders are cancelled (never rested). Each symbol has a 1,024-slot Disruptor command ring; a full ring returns HTTP `503`. Ring capacity must be a power of two and at least 64. The development consumer wait strategy sleeps for one millisecond while idle.
