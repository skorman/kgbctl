# kgbctl

## Setup

```sh
nix develop    # rustc, cargo, sqlx-cli, sqlite
```

Without nix: install Rust via [rustup](https://rustup.rs), then `cargo install sqlx-cli --no-default-features --features sqlite`.

## Database

```sh
echo 'DATABASE_URL=sqlite:var/kgb.db' > .env
mkdir -p var
sqlx database create
sqlx migrate run
cargo build
```

Run `sqlx migrate run` again after pulling new migrations. To build without a database, use `SQLX_OFFLINE=true cargo build`.

## Sync prerequisites

**Kraken:** create an API key with the **Query Funds** and **Query Ledger Entries** permissions, then:

```sh
export KRAKEN_API_KEY=...
export KRAKEN_SECRET_KEY=...
```

**Interactive Brokers:** run TWS or IB Gateway and log in. Under *Configure → API → Settings*, enable **ActiveX and Socket Clients** and allow connections from `127.0.0.1`. The defaults are below; override them only if your setup differs:

```sh
export IBKR_HOST=127.0.0.1
export IBKR_PORT=7496        # TWS live; 7497 TWS paper, 4001/4002 Gateway live/paper
export IBKR_CLIENT_ID=19
```

## Sync

```sh
cargo run -- -c sync                  # all brokers
cargo run -- -c sync --broker ibkr    # or kraken
cargo run -- -c sync --full           # refetch all history
```

## TUI

```sh
cargo run -- -c tui
```

| Key | Action |
| --- | --- |
| any key | Skip the startup animation |
| `o` | Overview / Detail |
| `←` `→` | Prev / Next tab |
| `↑` `↓` / `j` `k` | Move row |
| `r` | Reload from database |
| `s` | Sync brokers |
| `q` | Quit |

The database path defaults to `var/kgb.db`. To use a different file, pass `--db <path>` or set `KGB_DB`.

## Server

A read-only JSON API over the same data the TUI shows. It serves what's in the database and never contacts a broker.

```sh
cargo run -- -c serve                          # listens on 0.0.0.0:8080
KGB_API_TOKEN=secret cargo run -- -c serve     # require a bearer token
```

| Variable / flag | Default | Purpose |
| --- | --- | --- |
| `KGB_BIND` / `--bind` | `0.0.0.0:8080` | Listen address |
| `KGB_API_TOKEN` / `--api-token` | unset (no auth) | Require `Authorization: Bearer <token>` on everything except `/health` |
| `--cors-origin <url>` | any origin | Restrict browser origins; repeat for several |
| `RUST_LOG` | `kgbctl=info,tower_http=info` | Log filter |

All endpoints are `GET` under `/api/v1`. Money is in USD; percentages are fractions (`0.0523` = +5.23%); timestamps are RFC 3339 UTC.

| Endpoint | Returns |
| --- | --- |
| `/health` | `{status, version, db}` (no auth) |
| `/snapshot` | Everything the TUI shows: totals, per-broker sources with last sync run, and every tab with its holdings |
| `/tabs`, `/tabs/{name}` | Tab summaries; one tab's holdings (name is case-insensitive) |
| `/positions?broker=&account=` | Open positions per account |
| `/positions/{id}`, `/positions/{id}/history?from=&to=` | One position; its snapshots, oldest first |
| `/accounts` | Brokerage accounts |
| `/groups`, `/groups/{name}` | Groups with totals (`null` if nothing held); one group's members |
| `/orders/open` | Working orders |
| `/sync/runs?broker=&limit=` | Sync attempts, newest first (`limit` defaults to 50, max 500) |

Errors are `{"error": {"code", "message"}}` with status 400, 401, 404 or 500.

```sh
curl -H "Authorization: Bearer $KGB_API_TOKEN" localhost:8080/api/v1/snapshot | jq
```

### Raspberry Pi

Build for 64-bit ARM Linux, either natively on the Pi with `nix build .#packages.aarch64-linux.default`, or cross-compiled with `SQLX_OFFLINE=true cargo zigbuild --release --target aarch64-unknown-linux-gnu`. Then follow the steps at the top of `deploy/kgbctl.service` to install the binary and the systemd unit.

The server doesn't sync yet, so copy the database over. `.backup` makes a consistent copy even while the database is in use:

```sh
sqlite3 var/kgb.db ".backup /tmp/kgb.db"
scp /tmp/kgb.db pi:/tmp/ && ssh pi 'sudo install -o kgbctl -g kgbctl /tmp/kgb.db /var/lib/kgbctl/kgb.db'
```
