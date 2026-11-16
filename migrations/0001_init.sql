-- One row per brokerage account (IBKR U1234567, Kraken "main", ...)
CREATE TABLE accounts (
    id            INTEGER PRIMARY KEY,
    broker        TEXT NOT NULL CHECK (broker IN ('ibkr', 'kraken')),
    external_id   TEXT NOT NULL,                 -- IBKR account code / Kraken label
    name          TEXT,
    base_currency TEXT NOT NULL DEFAULT 'USD',
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (broker, external_id)
) STRICT;

-- Tradable things, broker-agnostic. "ticker" lives here.
CREATE TABLE instruments (
    id          INTEGER PRIMARY KEY,
    symbol      TEXT NOT NULL,                   -- display ticker: NVDA, BTC
    asset_class TEXT NOT NULL CHECK (asset_class IN
                  ('stock','etf','option','future','bond','crypto','cash','other')),
    currency    TEXT NOT NULL DEFAULT 'USD',     -- quote currency
    exchange    TEXT,
    name        TEXT,
    UNIQUE (symbol, asset_class, currency)
) STRICT;

-- Broker-specific identifiers -> instrument (IBKR conId, Kraken 'XXBT', 'ETH.S', ...)
CREATE TABLE instrument_broker_ids (
    broker        TEXT NOT NULL CHECK (broker IN ('ibkr', 'kraken')),
    external_id   TEXT NOT NULL,
    instrument_id INTEGER NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
    PRIMARY KEY (broker, external_id)
) STRICT;

-- Audit of each sync attempt; lets one broker fail without hiding the other.
CREATE TABLE sync_runs (
    id          INTEGER PRIMARY KEY,
    broker      TEXT NOT NULL CHECK (broker IN ('ibkr', 'kraken')),
    started_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    finished_at TEXT,
    status      TEXT NOT NULL DEFAULT 'running'
                  CHECK (status IN ('running','ok','error')),
    error       TEXT
) STRICT;

-- Current state: one row per account+instrument, upserted on each sync.
CREATE TABLE positions (
    id                INTEGER PRIMARY KEY,
    account_id        INTEGER NOT NULL REFERENCES accounts(id),
    instrument_id     INTEGER NOT NULL REFERENCES instruments(id),
    quantity          REAL NOT NULL,             -- shares / coins; negative = short
    avg_cost          REAL,                      -- per unit, instrument currency
    cost_basis_usd    REAL,                      -- quantity * avg_cost in USD
    market_price      REAL,                      -- last price, instrument currency
    market_value_usd  REAL,
    unrealized_pl_usd REAL,                      -- absolute P/L
    unrealized_pl_pct REAL,                      -- fraction: 0.0523 = +5.23%
    realized_pl_usd   REAL,
    opened_at         TEXT,
    closed_at         TEXT,                      -- set when quantity goes to 0
    updated_at        TEXT NOT NULL,
    last_sync_run_id  INTEGER REFERENCES sync_runs(id),
    UNIQUE (account_id, instrument_id)
) STRICT;

-- Append-only history: one row per position per successful sync.
CREATE TABLE position_snapshots (
    id                INTEGER PRIMARY KEY,
    sync_run_id       INTEGER NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE,
    position_id       INTEGER NOT NULL REFERENCES positions(id) ON DELETE CASCADE,
    taken_at          TEXT NOT NULL,
    quantity          REAL NOT NULL,
    avg_cost          REAL,
    market_price      REAL,
    market_value_usd  REAL,
    unrealized_pl_usd REAL,
    unrealized_pl_pct REAL
) STRICT;
CREATE INDEX idx_snapshots_position_time ON position_snapshots(position_id, taken_at);

-- Orders, open or closed.
CREATE TABLE orders (
    id               INTEGER PRIMARY KEY,
    account_id       INTEGER NOT NULL REFERENCES accounts(id),
    instrument_id    INTEGER NOT NULL REFERENCES instruments(id),
    broker_order_id  TEXT NOT NULL,              -- IBKR permId / Kraken txid
    side             TEXT NOT NULL CHECK (side IN ('buy','sell')),
    order_type       TEXT NOT NULL CHECK (order_type IN
                       ('market','limit','stop','stop_limit','trailing_stop','other')),
    time_in_force    TEXT,                       -- DAY, GTC, IOC, ...
    quantity         REAL NOT NULL,
    limit_price      REAL,
    stop_price       REAL,
    filled_quantity  REAL NOT NULL DEFAULT 0,
    avg_fill_price   REAL,
    fees_usd         REAL NOT NULL DEFAULT 0,
    status           TEXT NOT NULL CHECK (status IN
                       ('pending','open','partially_filled','filled',
                        'cancelled','rejected','expired')),
    placed_at        TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    closed_at        TEXT,                       -- filled/cancelled/rejected/expired time
    raw              TEXT,                       -- original broker JSON, for debugging
    UNIQUE (account_id, broker_order_id)
) STRICT;
CREATE INDEX idx_orders_status     ON orders(status);
CREATE INDEX idx_orders_instrument ON orders(instrument_id, placed_at);

-- Individual executions (partial fills); one order can have many.
CREATE TABLE fills (
    id             INTEGER PRIMARY KEY,
    order_id       INTEGER NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    broker_fill_id TEXT NOT NULL,                -- IBKR execId / Kraken trade id
    quantity       REAL NOT NULL,
    price          REAL NOT NULL,
    fee_usd        REAL NOT NULL DEFAULT 0,
    executed_at    TEXT NOT NULL,
    UNIQUE (order_id, broker_fill_id)
) STRICT;

-- KGB "ETFs": Zeus, Ares, Prometheus, Gaia, ...
CREATE TABLE position_groups (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
    description TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- Many-to-many: a ticker can belong to several groups.
CREATE TABLE position_group_members (
    group_id      INTEGER NOT NULL REFERENCES position_groups(id) ON DELETE CASCADE,
    instrument_id INTEGER NOT NULL REFERENCES instruments(id),
    target_weight REAL,                          -- optional, fraction of group
    added_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    PRIMARY KEY (group_id, instrument_id)
) STRICT;

-- Convenience: what the TUI table shows.
CREATE VIEW v_positions AS
SELECT p.id, a.broker, a.external_id AS account, i.symbol AS ticker, i.asset_class,
       p.quantity, p.market_price, p.market_value_usd, p.avg_cost, p.cost_basis_usd,
       p.unrealized_pl_usd, p.unrealized_pl_pct, p.updated_at
FROM positions p
JOIN accounts a    ON a.id = p.account_id
JOIN instruments i ON i.id = p.instrument_id
WHERE p.quantity != 0;

-- Group cards (Total Value / P/L $ / P/L %). A ticker in two groups counts in both.
CREATE VIEW v_group_totals AS
SELECT g.name,
       SUM(p.market_value_usd)                              AS value_usd,
       SUM(p.cost_basis_usd)                                AS cost_usd,
       SUM(p.market_value_usd) - SUM(p.cost_basis_usd)      AS pl_usd,
       (SUM(p.market_value_usd) - SUM(p.cost_basis_usd))
         / NULLIF(SUM(p.cost_basis_usd), 0)                 AS pl_pct
FROM position_groups g
JOIN position_group_members m ON m.group_id = g.id
JOIN positions p              ON p.instrument_id = m.instrument_id AND p.quantity != 0
GROUP BY g.id;
