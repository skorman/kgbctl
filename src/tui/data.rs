//! What the TUI shows, read from the database and bucketed like the POC: one
//! tab per position group, then IBKR positions in no group ("Other"), then
//! Kraken positions ("Crypto").

use std::collections::HashMap;

use crate::db::{Broker, Db, PositionGroup, PositionView, SyncRun};

pub const OTHER: &str = "Other";
pub const CRYPTO: &str = "Crypto";

/// One ticker in one tab, summed across accounts.
#[derive(Clone, Debug, PartialEq)]
pub struct Holding {
    pub ticker: String,
    pub value: f64,
    pub shares: f64,
    pub basis: f64,
    pub price: f64,
}

impl Holding {
    /// Average cost per unit.
    pub fn cost(&self) -> f64 {
        if self.shares != 0.0 { self.basis / self.shares } else { 0.0 }
    }

    /// Unrealized P/L as a fraction of cost basis.
    pub fn pl_pct(&self) -> f64 {
        if self.basis != 0.0 { self.value / self.basis - 1.0 } else { 0.0 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    pub value: f64,
    pub basis: f64,
}

impl Totals {
    fn add(&mut self, value: f64, basis: f64) {
        self.value += value;
        self.basis += basis;
    }

    pub fn pl(&self) -> f64 {
        self.value - self.basis
    }

    pub fn pl_pct(&self) -> f64 {
        if self.basis != 0.0 { self.pl() / self.basis } else { 0.0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    pub name: String,
    /// Sorted by value, largest first.
    pub holdings: Vec<Holding>,
    pub totals: Totals,
}

/// One broker's line in the overview breakdown.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub broker: Broker,
    pub totals: Totals,
    pub last_run: Option<SyncRun>,
}

impl Source {
    pub fn label(&self) -> &'static str {
        match self.broker {
            Broker::Ibkr => "Interactive Brokers",
            Broker::Kraken => "Kraken",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub tabs: Vec<Tab>,
    pub sources: Vec<Source>,
    pub totals: Totals,
}

pub const BROKERS: [Broker; 2] = [Broker::Ibkr, Broker::Kraken];

impl Snapshot {
    pub async fn load(db: &Db) -> sqlx::Result<Self> {
        let positions = db.list_positions().await?;
        let memberships = db.group_position_ids().await?;
        let groups = db.list_groups().await?;
        let mut runs = Vec::new();
        for broker in BROKERS {
            runs.push(db.last_sync_run(broker).await?);
        }
        Ok(Self::build(&positions, &memberships, &groups, runs))
    }

    /// `runs` is the last sync run for each of `BROKERS`, in order.
    pub fn build(
        positions: &[PositionView],
        memberships: &[(String, i64)],
        groups: &[PositionGroup],
        runs: Vec<Option<SyncRun>>,
    ) -> Self {
        let mut groups_of: HashMap<i64, Vec<&str>> = HashMap::new();
        for (name, position_id) in memberships {
            groups_of.entry(*position_id).or_default().push(name);
        }

        let names: Vec<&str> =
            groups.iter().map(|g| g.name.as_str()).chain([OTHER, CRYPTO]).collect();
        // tab name -> ticker -> holding
        let mut buckets: HashMap<&str, HashMap<&str, Holding>> = HashMap::new();
        let mut sources: Vec<Source> = BROKERS
            .iter()
            .zip(runs)
            .map(|(&broker, last_run)| Source { broker, totals: Totals::default(), last_run })
            .collect();

        for p in positions {
            let value = p.market_value_usd.unwrap_or(0.0);
            let basis = p.cost_basis_usd.unwrap_or(0.0);
            if let Some(s) = sources.iter_mut().find(|s| s.broker == p.broker) {
                s.totals.add(value, basis);
            }

            let tabs = match p.broker {
                Broker::Kraken => vec![CRYPTO],
                Broker::Ibkr => groups_of.get(&p.id).cloned().unwrap_or_else(|| vec![OTHER]),
            };
            for tab in tabs {
                let h = buckets.entry(tab).or_default().entry(&p.ticker).or_insert_with(|| {
                    Holding {
                        ticker: p.ticker.clone(),
                        value: 0.0,
                        shares: 0.0,
                        basis: 0.0,
                        price: 0.0,
                    }
                });
                h.value += value;
                h.shares += p.quantity;
                h.basis += basis;
                if let Some(price) = p.market_price {
                    h.price = price;
                }
            }
        }

        let tabs = names
            .into_iter()
            .map(|name| {
                let mut holdings: Vec<Holding> =
                    buckets.remove(name).map(|b| b.into_values().collect()).unwrap_or_default();
                holdings.sort_by(|a, b| b.value.total_cmp(&a.value).then(a.ticker.cmp(&b.ticker)));
                let mut totals = Totals::default();
                for h in &holdings {
                    totals.add(h.value, h.basis);
                }
                Tab { name: name.to_string(), holdings, totals }
            })
            .collect();

        let mut totals = Totals::default();
        for s in &sources {
            totals.add(s.totals.value, s.totals.basis);
        }
        Self { tabs, sources, totals }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{AssetClass, NewInstrument, PositionUpdate};

    fn update(symbol: &str, class: AssetClass, qty: f64, price: f64, avg_cost: f64) -> PositionUpdate {
        PositionUpdate {
            instrument: NewInstrument {
                symbol: symbol.into(),
                asset_class: class,
                currency: "USD".into(),
                exchange: None,
                name: None,
            },
            broker_instrument_id: symbol.into(),
            quantity: qty,
            avg_cost: Some(avg_cost),
            cost_basis_usd: Some(qty * avg_cost),
            market_price: Some(price),
            market_value_usd: Some(qty * price),
            unrealized_pl_usd: Some(qty * (price - avg_cost)),
            unrealized_pl_pct: Some(price / avg_cost - 1.0),
            realized_pl_usd: None,
        }
    }

    async fn sync(db: &Db, broker: Broker, account: &str, updates: &[PositionUpdate]) {
        let account = db.upsert_account(broker, account, None, "USD").await.unwrap();
        let run = db.start_sync_run(broker).await.unwrap();
        db.apply_position_sync(run, account, broker, updates).await.unwrap();
        db.finish_sync_run_ok(run).await.unwrap();
    }

    fn tab<'a>(s: &'a Snapshot, name: &str) -> &'a Tab {
        s.tabs.iter().find(|t| t.name == name).unwrap()
    }

    #[tokio::test]
    async fn buckets_and_aggregates_positions() {
        let db = Db::connect_in_memory().await.unwrap();
        use AssetClass::{Crypto, Stock};
        sync(&db, Broker::Ibkr, "U1", &[
            update("NVDA", Stock, 10.0, 100.0, 80.0),
            update("MP", Stock, 4.0, 50.0, 25.0),
            update("SPY", Stock, 1.0, 500.0, 400.0),
        ])
        .await;
        sync(&db, Broker::Ibkr, "U2", &[update("NVDA", Stock, 10.0, 100.0, 60.0)]).await;
        sync(&db, Broker::Kraken, "main", &[update("BTC", Crypto, 0.5, 90000.0, 60000.0)]).await;

        let s = Snapshot::load(&db).await.unwrap();
        let names: Vec<_> = s.tabs.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Zeus", "Ares", "Prometheus", "Gaia", OTHER, CRYPTO]);

        // NVDA summed across both accounts.
        let prometheus = tab(&s, "Prometheus");
        assert_eq!(
            prometheus.holdings,
            [Holding { ticker: "NVDA".into(), value: 2000.0, shares: 20.0, basis: 1400.0, price: 100.0 }]
        );
        assert_eq!(prometheus.holdings[0].cost(), 70.0);
        assert!((prometheus.holdings[0].pl_pct() - (2000.0 / 1400.0 - 1.0)).abs() < 1e-12);

        // MP is in two groups and shows in both.
        assert_eq!(tab(&s, "Zeus").holdings[0].ticker, "MP");
        assert_eq!(tab(&s, "Gaia").holdings[0].ticker, "MP");
        assert!(tab(&s, "Ares").holdings.is_empty());

        assert_eq!(tab(&s, OTHER).holdings[0].ticker, "SPY");
        assert_eq!(tab(&s, CRYPTO).holdings[0].ticker, "BTC");
        assert_eq!(tab(&s, CRYPTO).totals, Totals { value: 45000.0, basis: 30000.0 });

        // Source totals count each position once, even when it's in two groups.
        let ibkr = &s.sources[0];
        assert_eq!(ibkr.broker, Broker::Ibkr);
        assert_eq!(ibkr.totals, Totals { value: 2000.0 + 200.0 + 500.0, basis: 1400.0 + 100.0 + 400.0 });
        assert!(ibkr.last_run.is_some());
        assert_eq!(s.totals.value, 2700.0 + 45000.0);
    }

    #[tokio::test]
    async fn holdings_sort_by_value_descending() {
        let db = Db::connect_in_memory().await.unwrap();
        use AssetClass::Stock;
        sync(&db, Broker::Ibkr, "U1", &[
            update("AMD", Stock, 1.0, 100.0, 100.0),
            update("NVDA", Stock, 1.0, 300.0, 100.0),
            update("AVGO", Stock, 1.0, 200.0, 100.0),
        ])
        .await;

        let s = Snapshot::load(&db).await.unwrap();
        let tickers: Vec<_> =
            tab(&s, "Prometheus").holdings.iter().map(|h| h.ticker.as_str()).collect();
        assert_eq!(tickers, ["NVDA", "AVGO", "AMD"]);
        assert!(s.sources[1].last_run.is_none());
    }
}
