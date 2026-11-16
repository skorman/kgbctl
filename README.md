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
