-- Raw Kraken ledger entries. Cost basis is replayed from these, so a sync only
-- has to fetch entries newer than the latest one stored.
CREATE TABLE kraken_ledger (
    ledger_id  TEXT PRIMARY KEY,                 -- Kraken ledger key, e.g. L4UESK-KG3EQ-UFO4T5
    account_id INTEGER NOT NULL REFERENCES accounts(id),
    refid      TEXT NOT NULL,                    -- legs of one trade/convert share a refid
    time       REAL NOT NULL,                    -- unix seconds, as Kraken reports it
    type       TEXT NOT NULL,                    -- trade, deposit, staking, transfer, ...
    subtype    TEXT,
    asset      TEXT NOT NULL,                    -- raw code: XXBT, ZUSD, ETH.S, ...
    amount     REAL NOT NULL,
    fee        REAL NOT NULL,
    balance    REAL,
    raw        TEXT                              -- original JSON, for debugging
) STRICT;
CREATE INDEX idx_kraken_ledger_account_time ON kraken_ledger(account_id, time);
