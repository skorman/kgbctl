-- Seed the KGB ETF groups and their members from poc/*.csv.
-- IBKR reports ETFs (URA, PICK, ...) as STK, so every member is seeded as 'stock'.

INSERT INTO position_groups (name) VALUES ('Zeus'), ('Ares'), ('Prometheus'), ('Gaia');

CREATE TEMP TABLE seed_members (grp TEXT NOT NULL, symbol TEXT NOT NULL);
INSERT INTO seed_members (grp, symbol) VALUES
    ('Zeus', 'XOM'), ('Zeus', 'CVX'), ('Zeus', 'COP'), ('Zeus', 'TRP'), ('Zeus', 'NEM'),
    ('Zeus', 'DOW'), ('Zeus', 'FCX'), ('Zeus', 'EPD'), ('Zeus', 'KMI'), ('Zeus', 'ALB'),
    ('Zeus', 'SQM'), ('Zeus', 'PICK'), ('Zeus', 'NUKZ'), ('Zeus', 'UEC'), ('Zeus', 'NUE'),
    ('Zeus', 'MP'), ('Zeus', 'TALO'), ('Zeus', 'EGY'), ('Zeus', 'FTI'),

    ('Ares', 'LMT'), ('Ares', 'NOC'), ('Ares', 'RTX'), ('Ares', 'GD'), ('Ares', 'BA'),
    ('Ares', 'LHX'), ('Ares', 'TXT'), ('Ares', 'HII'), ('Ares', 'URA'), ('Ares', 'REMX'),
    ('Ares', 'LIT'), ('Ares', 'PLTR'), ('Ares', 'MRCY'),

    ('Prometheus', 'NVDA'), ('Prometheus', 'AMD'), ('Prometheus', 'AVGO'), ('Prometheus', 'TSM'),
    ('Prometheus', 'ASML'), ('Prometheus', 'AMAT'), ('Prometheus', 'MSFT'), ('Prometheus', 'GOOGL'),
    ('Prometheus', 'CRM'), ('Prometheus', 'AAPL'), ('Prometheus', 'QCOM'), ('Prometheus', 'PANW'),
    ('Prometheus', 'CRWD'), ('Prometheus', 'DLR'), ('Prometheus', 'EQIX'), ('Prometheus', 'ANET'),
    ('Prometheus', 'VRT'), ('Prometheus', 'AI'), ('Prometheus', 'PATH'), ('Prometheus', 'IONQ'),
    ('Prometheus', 'NVTS'),

    ('Gaia', 'MP'), ('Gaia', 'FCX'), ('Gaia', 'RIO'), ('Gaia', 'ALB'), ('Gaia', 'IDR'),
    ('Gaia', 'UUUU'), ('Gaia', 'USAR'), ('Gaia', 'CRML'), ('Gaia', 'NB'), ('Gaia', 'TMC');

INSERT OR IGNORE INTO instruments (symbol, asset_class)
SELECT DISTINCT symbol, 'stock' FROM seed_members;

INSERT INTO position_group_members (group_id, instrument_id)
SELECT g.id, i.id
FROM seed_members s
JOIN position_groups g ON g.name = s.grp
JOIN instruments i     ON i.symbol = s.symbol AND i.asset_class = 'stock' AND i.currency = 'USD';

DROP TABLE seed_members;
