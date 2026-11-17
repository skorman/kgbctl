//! Kraken positions and cost basis.
//!
//! The API key only needs "Query Funds" plus "Query open orders & trades".
//! Cost basis comes from replaying the ledger. The ledger is stored raw in
//! `kraken_ledger`, so each sync only fetches entries newer than the latest
//! one stored, then replays the whole history from the database.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256, Sha512};

use super::{SyncOptions, SyncReport};
use crate::db::{AssetClass, Broker, Db, KrakenLedgerEntry, NewInstrument, PositionUpdate};

const BASE: &str = "https://api.kraken.com";
/// One API key is one account; this is its `accounts.external_id`.
const ACCOUNT: &str = "main";
/// Re-fetch this much before the newest stored entry, so the legs of a trade
/// (which share a refid) are never split across two fetches.
const LEDGER_OVERLAP_SECS: f64 = 3600.0;
/// Pause between ledger pages; ledger calls are costly against the rate limit.
const PAGE_DELAY: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct Config {
    pub api_key: String,
    /// Base64 private key, as Kraken shows it.
    pub secret: String,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config").finish_non_exhaustive()
    }
}

pub async fn sync(
    db: &Db,
    run_id: i64,
    cfg: &Config,
    opts: SyncOptions,
) -> anyhow::Result<SyncReport> {
    let client = Client::new(cfg)?;
    let account = db
        .upsert_account(Broker::Kraken, ACCOUNT, None, "USD")
        .await?;

    let since = match opts.full {
        true => None,
        false => db
            .kraken_ledger_cursor(account)
            .await?
            .map(|t| t - LEDGER_OVERLAP_SECS),
    };
    let fetched = client
        .ledger_since(since)
        .await
        .context("fetching ledger")?;
    let new_records = db.insert_kraken_ledger(account, &fetched).await?;

    let balances = client.balances().await.context("fetching balances")?;
    let lots = build_lots(&db.kraken_ledger(account).await?);

    let mut updates = Vec::new();
    for (sym, qty) in balances {
        if sym == "USD" || qty == 0.0 {
            continue;
        }
        let price = client.usd_price(&sym).await;
        let lot = lots.get(&sym).cloned().unwrap_or_default();
        updates.push(position_update(&sym, qty, &lot, price));
    }
    db.apply_position_sync(run_id, account, Broker::Kraken, &updates)
        .await?;

    // Margin positions aren't modelled yet; just make sure they aren't missed silently.
    let margin = client.private("OpenPositions", &[]).await?;
    if let Some(n) = margin.as_object().map(|m| m.len()).filter(|&n| n > 0) {
        eprintln!("kraken: {n} open margin position(s) not synced");
    }

    Ok(SyncReport {
        positions: updates.len(),
        new_records,
        record_kind: "ledger entries",
    })
}

fn position_update(sym: &str, qty: f64, lot: &Lot, price: Option<f64>) -> PositionUpdate {
    let avg_cost = lot.avg_cost();
    let cost = avg_cost * qty;
    let value = price.map(|p| p * qty);
    PositionUpdate {
        instrument: NewInstrument {
            symbol: display_symbol(sym).into(),
            asset_class: AssetClass::Crypto,
            currency: "USD".into(),
            exchange: None,
            name: None,
        },
        broker_instrument_id: sym.into(),
        quantity: qty,
        avg_cost: Some(avg_cost),
        cost_basis_usd: Some(cost),
        market_price: price,
        market_value_usd: value,
        unrealized_pl_usd: value.map(|v| v - cost),
        unrealized_pl_pct: value.filter(|_| cost != 0.0).map(|v| (v - cost) / cost),
        realized_pl_usd: Some(lot.realized),
    }
}

/// XXBT -> XBT, ZUSD -> USD, ETH.S / SOL03.S / SUI.B / XETH.F -> ETH / SOL / SUI / ETH.
pub fn base_asset(asset: &str) -> &str {
    let mut asset = asset.split('.').next().unwrap_or(asset);
    if asset.len() == 4 && asset.starts_with(['X', 'Z']) {
        asset = &asset[1..];
    }
    asset.trim_end_matches(|c: char| c.is_ascii_digit())
}

pub fn display_symbol(sym: &str) -> &str {
    match sym {
        "XBT" => "BTC",
        s => s,
    }
}

struct Client {
    http: reqwest::Client,
    api_key: String,
    secret: Vec<u8>,
    last_nonce: AtomicU64,
}

#[derive(Deserialize)]
struct RawLedgerEntry {
    refid: String,
    time: f64,
    #[serde(rename = "type")]
    entry_type: String,
    subtype: Option<String>,
    asset: String,
    amount: String,
    fee: String,
    balance: Option<String>,
}

impl Client {
    fn new(cfg: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()?,
            api_key: cfg.api_key.clone(),
            secret: BASE64
                .decode(&cfg.secret)
                .context("KRAKEN_SECRET_KEY is not valid base64")?,
            last_nonce: AtomicU64::new(0),
        })
    }

    /// Microsecond timestamp, strictly increasing across calls as Kraken requires.
    fn nonce(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        let prev = self.last_nonce.fetch_max(now, Ordering::SeqCst);
        if prev >= now {
            self.last_nonce.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            now
        }
    }

    async fn public(&self, path: &str, params: &[(&str, &str)]) -> anyhow::Result<Value> {
        let url = format!("{BASE}/0/public/{path}?{}", encode(params));
        check(self.http.get(url).send().await?.json().await?)
    }

    async fn private(&self, path: &str, params: &[(&str, &str)]) -> anyhow::Result<Value> {
        let urlpath = format!("/0/private/{path}");
        let nonce = self.nonce().to_string();
        let mut all = vec![("nonce", nonce.as_str())];
        all.extend_from_slice(params);
        let postdata = encode(&all);

        let resp = self
            .http
            .post(format!("{BASE}{urlpath}"))
            .header("API-Key", &self.api_key)
            .header("API-Sign", sign(&self.secret, &urlpath, &nonce, &postdata))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(postdata)
            .send()
            .await?;
        check(resp.json().await?)
    }

    /// Ledger entries newer than `start` (unix seconds), or the whole ledger.
    /// Kraken pages newest-first, 50 entries per call.
    async fn ledger_since(&self, start: Option<f64>) -> anyhow::Result<Vec<KrakenLedgerEntry>> {
        let start = start.map(|s| (s.floor() as i64).to_string());
        let mut out = Vec::new();
        loop {
            let ofs = out.len().to_string();
            let mut params = vec![("ofs", ofs.as_str())];
            if let Some(s) = &start {
                params.push(("start", s));
            }
            let r = self.private("Ledgers", &params).await?;
            let page = r["ledger"]
                .as_object()
                .ok_or_else(|| anyhow!("Ledgers: missing ledger"))?;
            let count = r["count"].as_u64().unwrap_or(0);
            for (id, v) in page {
                out.push(parse_ledger_entry(id, v)?);
            }
            if page.is_empty() || out.len() as u64 >= count {
                return Ok(out);
            }
            tokio::time::sleep(PAGE_DELAY).await;
        }
    }

    /// Balances summed per base asset, so staked wrappers (ETH.S) fold into ETH.
    async fn balances(&self) -> anyhow::Result<BTreeMap<String, f64>> {
        let r = self.private("Balance", &[]).await?;
        let mut out = BTreeMap::new();
        for (asset, v) in r
            .as_object()
            .ok_or_else(|| anyhow!("Balance: not an object"))?
        {
            let qty: f64 = v.as_str().unwrap_or("0").parse()?;
            *out.entry(base_asset(asset).to_string()).or_default() += qty;
        }
        Ok(out)
    }

    /// Last trade price against USD, or `None` if Kraken has no such pair.
    async fn usd_price(&self, sym: &str) -> Option<f64> {
        if sym == "USD" {
            return Some(1.0);
        }
        let r = self
            .public("Ticker", &[("pair", &format!("{sym}USD"))])
            .await
            .ok()?;
        let (_, ticker) = r.as_object()?.iter().next()?;
        ticker["c"][0].as_str()?.parse().ok()
    }
}

fn parse_ledger_entry(id: &str, v: &Value) -> anyhow::Result<KrakenLedgerEntry> {
    let raw: RawLedgerEntry =
        serde_json::from_value(v.clone()).with_context(|| format!("ledger entry {id}"))?;
    Ok(KrakenLedgerEntry {
        ledger_id: id.into(),
        refid: raw.refid,
        time: raw.time,
        entry_type: raw.entry_type,
        subtype: raw.subtype.filter(|s| !s.is_empty()),
        asset: raw.asset,
        amount: raw.amount.parse()?,
        fee: raw.fee.parse()?,
        balance: raw.balance.map(|b| b.parse()).transpose()?,
        raw: Some(v.to_string()),
    })
}

fn check(resp: Value) -> anyhow::Result<Value> {
    if let Some(errors) = resp["error"].as_array().filter(|e| !e.is_empty()) {
        bail!("Kraken error: {errors:?}");
    }
    Ok(resp["result"].clone())
}

fn encode(params: &[(&str, &str)]) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(params)
        .finish()
}

/// `API-Sign`: HMAC-SHA512 of `urlpath + SHA256(nonce + postdata)`, keyed by the decoded secret.
fn sign(secret: &[u8], urlpath: &str, nonce: &str, postdata: &str) -> String {
    let digest = Sha256::digest(format!("{nonce}{postdata}"));
    let mut mac = Hmac::<Sha512>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(urlpath.as_bytes());
    mac.update(&digest);
    BASE64.encode(mac.finalize().into_bytes())
}

/// Running position for one base asset, using the average-cost method.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lot {
    /// Net quantity held, from the ledger.
    pub qty: f64,
    /// USD cost basis of that quantity.
    pub cost: f64,
    /// Lifetime quantity bought for USD, and the USD spent on it (incl. fees).
    pub bought_qty: f64,
    pub bought_usd: f64,
    /// Staking rewards / bonuses (cost basis $0).
    pub free_qty: f64,
    /// Deposited from outside (cost basis unknown, taken as $0).
    pub in_qty: f64,
    pub realized: f64,
}

impl Lot {
    fn buy(&mut self, qty: f64, usd: f64) {
        self.qty += qty;
        self.cost += usd;
        self.bought_qty += qty;
        self.bought_usd += usd;
    }

    fn sell(&mut self, qty: f64, usd: f64) {
        let avg = self.avg_cost();
        self.qty -= qty;
        self.cost -= avg * qty;
        self.realized += usd - avg * qty;
    }

    fn reward(&mut self, qty: f64) {
        self.qty += qty;
        self.free_qty += qty;
    }

    fn deposit(&mut self, qty: f64) {
        self.qty += qty;
        self.in_qty += qty;
    }

    pub fn avg_cost(&self) -> f64 {
        if self.qty != 0.0 {
            self.cost / self.qty
        } else {
            0.0
        }
    }
}

/// Replay the ledger oldest-first into one `Lot` per base asset. Legs sharing
/// a refid form one trade/convert.
pub fn build_lots(ledger: &[KrakenLedgerEntry]) -> HashMap<String, Lot> {
    let mut sorted: Vec<_> = ledger.iter().collect();
    sorted.sort_by(|a, b| a.time.total_cmp(&b.time));

    // Groups come out in order of their first leg's time.
    let mut groups: Vec<Vec<&KrakenLedgerEntry>> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for e in sorted {
        let i = *index.entry(&e.refid).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[i].push(e);
    }

    let net = |e: &KrakenLedgerEntry| e.amount - e.fee;
    let mut lots: HashMap<String, Lot> = HashMap::new();

    for legs in groups {
        let has = |kind: &str| legs.iter().any(|e| e.entry_type == kind);
        let (usd, crypto): (Vec<_>, Vec<_>) = legs
            .iter()
            .copied()
            .partition(|e| base_asset(&e.asset) == "USD");

        if (has("trade") || has("receive") || has("spend")) && !usd.is_empty() {
            let syms: HashSet<_> = crypto.iter().map(|e| base_asset(&e.asset)).collect();
            if syms.len() != 1 {
                continue; // crypto<->crypto or wrapper-only moves: nothing to price
            }
            let sym = base_asset(&crypto[0].asset);
            let qty: f64 = crypto.iter().map(|e| net(e)).sum();
            let usd_flow: f64 = usd.iter().map(|e| net(e)).sum(); // fees folded in: negative on buys
            let lot = lots.entry(sym.into()).or_default();
            if qty > 0.0 {
                lot.buy(qty, -usd_flow);
            } else if qty < 0.0 {
                lot.sell(-qty, usd_flow);
            }
        } else if legs.iter().all(|e| e.entry_type == "transfer") || has("earn") {
            continue; // spot <-> staked wrapper moves net to zero for the base asset
        } else {
            for e in crypto {
                let lot = lots.entry(base_asset(&e.asset).into()).or_default();
                match e.entry_type.as_str() {
                    "staking" | "reward" => lot.reward(net(e)),
                    "deposit" | "receive" => lot.deposit(net(e)),
                    "withdrawal" if net(e) < 0.0 => lot.sell(-net(e), 0.0),
                    _ => {}
                }
            }
        }
    }
    lots
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg(
        refid: &str,
        time: f64,
        kind: &str,
        asset: &str,
        amount: f64,
        fee: f64,
    ) -> KrakenLedgerEntry {
        KrakenLedgerEntry {
            ledger_id: format!("{refid}-{asset}"),
            refid: refid.into(),
            time,
            entry_type: kind.into(),
            subtype: None,
            asset: asset.into(),
            amount,
            fee,
            balance: None,
            raw: None,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn base_asset_strips_prefixes_and_suffixes() {
        for (raw, want) in [
            ("XXBT", "XBT"),
            ("ZUSD", "USD"),
            ("ETH.S", "ETH"),
            ("SOL03.S", "SOL"),
            ("SUI.B", "SUI"),
            ("XETH.F", "ETH"),
            ("DOT", "DOT"),
            ("USDC", "USDC"),
        ] {
            assert_eq!(base_asset(raw), want, "{raw}");
        }
        assert_eq!(display_symbol("XBT"), "BTC");
        assert_eq!(display_symbol("ETH"), "ETH");
    }

    #[test]
    fn signature_matches_kraken_docs_example() {
        let secret = BASE64
            .decode("kQH5HW/8p1uGOVjbgWA7FunAmGO8lsSUXNsu3eow76sz84Q18fWxnyRzBHCd3pd5nE9qa99HAZtuZuj6F1huXg==")
            .unwrap();
        let sig = sign(
            &secret,
            "/0/private/AddOrder",
            "1616492376594",
            "nonce=1616492376594&ordertype=limit&pair=XBTUSD&price=37500&type=buy&volume=1.25",
        );
        assert_eq!(
            sig,
            "4/dpxb3iT4tp/ZCVEwSnEsLxx0bqyhLpdfOpc6fn7OR8+UClSV5n9E6aSS8MPtnRfp32bAb0nmbRn6H8ndwLUQ=="
        );
    }

    #[test]
    fn buys_and_sells_use_average_cost_with_fees() {
        let ledger = [
            // Buy 1 BTC for $100 + $1 fee.
            leg("T1", 1.0, "trade", "ZUSD", -100.0, 1.0),
            leg("T1", 1.0, "trade", "XXBT", 1.0, 0.0),
            // Buy 1 BTC for $200, no fee.
            leg("T2", 2.0, "trade", "ZUSD", -200.0, 0.0),
            leg("T2", 2.0, "trade", "XXBT", 1.0, 0.0),
            // Sell 0.5 BTC for $150.
            leg("T3", 3.0, "trade", "XXBT", -0.5, 0.0),
            leg("T3", 3.0, "trade", "ZUSD", 150.0, 0.0),
        ];
        let lots = build_lots(&ledger);
        let btc = &lots["XBT"];
        assert!(close(btc.qty, 1.5));
        assert!(close(btc.bought_usd, 301.0));
        assert!(close(btc.avg_cost(), 150.5));
        assert!(close(btc.realized, 150.0 - 0.5 * 150.5));
        assert!(!lots.contains_key("USD") || lots["USD"].qty == 0.0);
    }

    #[test]
    fn rewards_and_deposits_come_in_at_zero_cost() {
        let ledger = [
            leg("T1", 1.0, "trade", "ZUSD", -100.0, 0.0),
            leg("T1", 1.0, "trade", "XETH", 1.0, 0.0),
            leg("S1", 2.0, "staking", "ETH.S", 0.1, 0.0),
            leg("D1", 3.0, "deposit", "XETH", 0.9, 0.0),
        ];
        let eth = &build_lots(&ledger)["ETH"];
        assert!(close(eth.qty, 2.0));
        assert!(close(eth.free_qty, 0.1));
        assert!(close(eth.in_qty, 0.9));
        assert!(close(eth.avg_cost(), 50.0));
    }

    #[test]
    fn transfers_earn_and_crypto_swaps_are_skipped() {
        let ledger = [
            leg("T1", 1.0, "trade", "ZUSD", -100.0, 0.0),
            leg("T1", 1.0, "trade", "XETH", 1.0, 0.0),
            // Spot -> staked wrapper.
            leg("X1", 2.0, "transfer", "XETH", -1.0, 0.0),
            leg("X1", 2.0, "transfer", "ETH.S", 1.0, 0.0),
            leg("E1", 3.0, "earn", "ETH.S", -1.0, 0.0),
            leg("E1", 3.0, "earn", "XETH", 1.0, 0.0),
            // ETH -> SOL with a USD leg: two crypto assets, nothing to price.
            leg("C1", 4.0, "trade", "XETH", -0.5, 0.0),
            leg("C1", 4.0, "trade", "ZUSD", 0.0, 0.0),
            leg("C1", 4.0, "trade", "SOL", 10.0, 0.0),
        ];
        let lots = build_lots(&ledger);
        assert!(close(lots["ETH"].qty, 1.0));
        assert!(close(lots["ETH"].cost, 100.0));
        assert!(!lots.contains_key("SOL"));
    }

    #[test]
    fn withdrawals_reduce_quantity_at_average_cost() {
        let ledger = [
            leg("T1", 1.0, "trade", "ZUSD", -100.0, 0.0),
            leg("T1", 1.0, "trade", "XXBT", 2.0, 0.0),
            leg("W1", 2.0, "withdrawal", "XXBT", -1.0, 0.0),
        ];
        let btc = &build_lots(&ledger)["XBT"];
        assert!(close(btc.qty, 1.0));
        assert!(close(btc.avg_cost(), 50.0));
        assert!(close(btc.realized, -50.0));
    }

    #[test]
    fn position_update_values_holding() {
        let lot = Lot {
            qty: 2.0,
            cost: 100.0,
            ..Lot::default()
        };
        let u = position_update("XBT", 2.0, &lot, Some(75.0));
        assert_eq!(u.instrument.symbol, "BTC");
        assert_eq!(u.broker_instrument_id, "XBT");
        assert_eq!(u.cost_basis_usd, Some(100.0));
        assert_eq!(u.market_value_usd, Some(150.0));
        assert_eq!(u.unrealized_pl_usd, Some(50.0));
        assert_eq!(u.unrealized_pl_pct, Some(0.5));

        let unpriced = position_update("XBT", 2.0, &lot, None);
        assert_eq!(unpriced.market_value_usd, None);
        assert_eq!(unpriced.unrealized_pl_pct, None);
    }
}
