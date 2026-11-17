use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;

use super::{
    AssetClass, Broker, Db, PositionSnapshot, PositionUpdate, PositionView, instruments, ts,
};

impl Db {
    /// Write one broker's full position report for `account_id`, atomically:
    /// upsert each instrument and position, append a snapshot per position, and
    /// close any open position for the account that the report no longer includes.
    pub async fn apply_position_sync(
        &self,
        sync_run_id: i64,
        account_id: i64,
        broker: Broker,
        updates: &[PositionUpdate],
    ) -> sqlx::Result<()> {
        let now = ts(Utc::now());
        let mut tx = self.pool.begin().await?;

        for u in updates {
            let instrument_id = instruments::upsert_instrument(&mut tx, &u.instrument).await?;
            instruments::link_broker_id(&mut tx, broker, &u.broker_instrument_id, instrument_id)
                .await?;
            let position_id =
                upsert_position(&mut tx, sync_run_id, account_id, instrument_id, u, &now).await?;
            insert_snapshot(&mut tx, sync_run_id, position_id, u, &now).await?;
        }

        close_missing_positions(&mut tx, sync_run_id, account_id, &now).await?;
        tx.commit().await
    }

    /// Open positions across all accounts (`v_positions`).
    pub async fn list_positions(&self) -> sqlx::Result<Vec<PositionView>> {
        sqlx::query_as!(
            PositionView,
            r#"
            SELECT id AS "id!", broker AS "broker!: Broker", account AS "account!",
                   ticker AS "ticker!", asset_class AS "asset_class!: AssetClass",
                   quantity AS "quantity!", market_price, market_value_usd, avg_cost,
                   cost_basis_usd, unrealized_pl_usd, unrealized_pl_pct,
                   updated_at AS "updated_at!: DateTime<Utc>"
            FROM v_positions
            ORDER BY broker, account, ticker
            "#
        )
        .fetch_all(&self.pool)
        .await
    }

    /// One open position (`v_positions`), or `None` if it's closed or unknown.
    pub async fn position(&self, id: i64) -> sqlx::Result<Option<PositionView>> {
        sqlx::query_as!(
            PositionView,
            r#"
            SELECT id AS "id!", broker AS "broker!: Broker", account AS "account!",
                   ticker AS "ticker!", asset_class AS "asset_class!: AssetClass",
                   quantity AS "quantity!", market_price, market_value_usd, avg_cost,
                   cost_basis_usd, unrealized_pl_usd, unrealized_pl_pct,
                   updated_at AS "updated_at!: DateTime<Utc>"
            FROM v_positions
            WHERE id = ?
            "#,
            id,
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Snapshots of one position, oldest first, optionally bounded by `from`/`to`
    /// (inclusive).
    pub async fn position_history(
        &self,
        position_id: i64,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> sqlx::Result<Vec<PositionSnapshot>> {
        let from = from.map(ts);
        let to = to.map(ts);
        sqlx::query_as!(
            PositionSnapshot,
            r#"
            SELECT taken_at AS "taken_at: DateTime<Utc>", quantity, avg_cost, market_price,
                   market_value_usd, unrealized_pl_usd, unrealized_pl_pct
            FROM position_snapshots
            WHERE position_id = ?1
              AND (?2 IS NULL OR taken_at >= ?2)
              AND (?3 IS NULL OR taken_at <= ?3)
            ORDER BY taken_at, id
            "#,
            position_id,
            from,
            to,
        )
        .fetch_all(&self.pool)
        .await
    }
}

async fn upsert_position(
    conn: &mut SqliteConnection,
    sync_run_id: i64,
    account_id: i64,
    instrument_id: i64,
    u: &PositionUpdate,
    now: &str,
) -> sqlx::Result<i64> {
    // opened_at is reset when a closed (quantity 0) position reopens;
    // closed_at is set when quantity goes to 0 and cleared otherwise.
    sqlx::query_scalar!(
        r#"
        INSERT INTO positions (
            account_id, instrument_id, quantity, avg_cost, cost_basis_usd, market_price,
            market_value_usd, unrealized_pl_usd, unrealized_pl_pct, realized_pl_usd,
            opened_at, closed_at, updated_at, last_sync_run_id)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                CASE WHEN ?3 != 0 THEN ?11 END, CASE WHEN ?3 = 0 THEN ?11 END, ?11, ?12)
        ON CONFLICT (account_id, instrument_id) DO UPDATE SET
            quantity          = excluded.quantity,
            avg_cost          = excluded.avg_cost,
            cost_basis_usd    = excluded.cost_basis_usd,
            market_price      = excluded.market_price,
            market_value_usd  = excluded.market_value_usd,
            unrealized_pl_usd = excluded.unrealized_pl_usd,
            unrealized_pl_pct = excluded.unrealized_pl_pct,
            realized_pl_usd   = excluded.realized_pl_usd,
            opened_at = CASE WHEN positions.quantity = 0 AND excluded.quantity != 0
                             THEN excluded.updated_at ELSE positions.opened_at END,
            closed_at = CASE WHEN excluded.quantity = 0
                             THEN COALESCE(positions.closed_at, excluded.updated_at) END,
            updated_at        = excluded.updated_at,
            last_sync_run_id  = excluded.last_sync_run_id
        RETURNING id
        "#,
        account_id,
        instrument_id,
        u.quantity,
        u.avg_cost,
        u.cost_basis_usd,
        u.market_price,
        u.market_value_usd,
        u.unrealized_pl_usd,
        u.unrealized_pl_pct,
        u.realized_pl_usd,
        now,
        sync_run_id,
    )
    .fetch_one(conn)
    .await
}

async fn insert_snapshot(
    conn: &mut SqliteConnection,
    sync_run_id: i64,
    position_id: i64,
    u: &PositionUpdate,
    now: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO position_snapshots (
            sync_run_id, position_id, taken_at, quantity, avg_cost, market_price,
            market_value_usd, unrealized_pl_usd, unrealized_pl_pct)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        sync_run_id,
        position_id,
        now,
        u.quantity,
        u.avg_cost,
        u.market_price,
        u.market_value_usd,
        u.unrealized_pl_usd,
        u.unrealized_pl_pct,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Zero out open positions in `account_id` that this sync run did not report.
async fn close_missing_positions(
    conn: &mut SqliteConnection,
    sync_run_id: i64,
    account_id: i64,
    now: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        UPDATE positions
        SET quantity = 0, market_value_usd = 0, unrealized_pl_usd = 0,
            closed_at = ?1, updated_at = ?1, last_sync_run_id = ?2
        WHERE account_id = ?3 AND quantity != 0 AND last_sync_run_id IS NOT ?2
        "#,
        now,
        sync_run_id,
        account_id,
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::NewInstrument;

    fn stock(
        symbol: &str,
        con_id: &str,
        quantity: f64,
        price: f64,
        avg_cost: f64,
    ) -> PositionUpdate {
        PositionUpdate {
            instrument: NewInstrument {
                symbol: symbol.into(),
                asset_class: AssetClass::Stock,
                currency: "USD".into(),
                exchange: None,
                name: None,
            },
            broker_instrument_id: con_id.into(),
            quantity,
            avg_cost: Some(avg_cost),
            cost_basis_usd: Some(quantity * avg_cost),
            market_price: Some(price),
            market_value_usd: Some(quantity * price),
            unrealized_pl_usd: Some(quantity * (price - avg_cost)),
            unrealized_pl_pct: Some((price - avg_cost) / avg_cost),
            realized_pl_usd: None,
        }
    }

    async fn sync(db: &Db, account_id: i64, updates: &[PositionUpdate]) -> i64 {
        let run = db.start_sync_run(Broker::Ibkr).await.unwrap();
        db.apply_position_sync(run, account_id, Broker::Ibkr, updates)
            .await
            .unwrap();
        db.finish_sync_run_ok(run).await.unwrap();
        run
    }

    async fn count(db: &Db, sql: &'static str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(&db.pool).await.unwrap()
    }

    #[tokio::test]
    async fn repeated_syncs_upsert_positions_and_append_snapshots() {
        let db = Db::connect_in_memory().await.unwrap();
        let account = db
            .upsert_account(Broker::Ibkr, "U1", None, "USD")
            .await
            .unwrap();

        sync(&db, account, &[stock("NVDA", "1", 10.0, 100.0, 80.0)]).await;
        sync(&db, account, &[stock("NVDA", "1", 12.0, 110.0, 82.0)]).await;

        let positions = db.list_positions().await.unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].ticker, "NVDA");
        assert_eq!(positions[0].quantity, 12.0);
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM position_snapshots").await,
            2
        );
    }

    #[tokio::test]
    async fn position_history_is_oldest_first_and_bounded() {
        let db = Db::connect_in_memory().await.unwrap();
        let account = db
            .upsert_account(Broker::Ibkr, "U1", None, "USD")
            .await
            .unwrap();

        for price in [100.0, 110.0, 120.0] {
            sync(&db, account, &[stock("NVDA", "1", 10.0, price, 80.0)]).await;
        }

        let id = db.list_positions().await.unwrap()[0].id;
        assert_eq!(db.position(id).await.unwrap().unwrap().ticker, "NVDA");
        assert!(db.position(id + 1).await.unwrap().is_none());

        let all = db.position_history(id, None, None).await.unwrap();
        let prices: Vec<_> = all.iter().map(|s| s.market_price.unwrap()).collect();
        assert_eq!(prices, [100.0, 110.0, 120.0]);

        let later = db
            .position_history(id, Some(all[1].taken_at), None)
            .await
            .unwrap();
        assert_eq!(later.len(), 2);
        let earlier = db
            .position_history(id, None, Some(all[0].taken_at))
            .await
            .unwrap();
        assert_eq!(earlier.len(), 1);
    }

    #[tokio::test]
    async fn positions_missing_from_a_sync_are_closed() {
        let db = Db::connect_in_memory().await.unwrap();
        let account = db
            .upsert_account(Broker::Ibkr, "U1", None, "USD")
            .await
            .unwrap();

        sync(
            &db,
            account,
            &[
                stock("NVDA", "1", 10.0, 100.0, 80.0),
                stock("AMD", "2", 5.0, 150.0, 120.0),
            ],
        )
        .await;
        sync(&db, account, &[stock("NVDA", "1", 10.0, 101.0, 80.0)]).await;

        let tickers: Vec<_> = db
            .list_positions()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.ticker)
            .collect();
        assert_eq!(tickers, ["NVDA"]);

        let closed: Option<String> = sqlx::query_scalar(
            "SELECT p.closed_at FROM positions p JOIN instruments i ON i.id = p.instrument_id
             WHERE i.symbol = 'AMD'",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert!(closed.is_some());
    }

    #[tokio::test]
    async fn group_totals_sum_open_positions() {
        let db = Db::connect_in_memory().await.unwrap();
        let account = db
            .upsert_account(Broker::Ibkr, "U1", None, "USD")
            .await
            .unwrap();

        // NVDA and AMD are both seeded members of Prometheus.
        sync(
            &db,
            account,
            &[
                stock("NVDA", "1", 10.0, 100.0, 80.0),
                stock("AMD", "2", 5.0, 150.0, 120.0),
            ],
        )
        .await;

        let totals = db.group_totals().await.unwrap();
        assert_eq!(totals.len(), 1);
        let t = &totals[0];
        assert_eq!(t.name, "Prometheus");
        assert_eq!(t.value_usd, Some(1000.0 + 750.0));
        assert_eq!(t.cost_usd, Some(800.0 + 600.0));
        assert_eq!(t.pl_usd, Some(350.0));
        assert_eq!(t.pl_pct, Some(350.0 / 1400.0));
    }

    #[tokio::test]
    async fn group_position_ids_lists_each_group_a_position_is_in() {
        let db = Db::connect_in_memory().await.unwrap();
        let account = db
            .upsert_account(Broker::Ibkr, "U1", None, "USD")
            .await
            .unwrap();

        // MP is seeded in Zeus and Gaia; SPY is in no group.
        sync(
            &db,
            account,
            &[
                stock("MP", "1", 10.0, 50.0, 40.0),
                stock("SPY", "2", 1.0, 500.0, 400.0),
            ],
        )
        .await;

        let mp = db
            .list_positions()
            .await
            .unwrap()
            .into_iter()
            .find(|p| p.ticker == "MP")
            .unwrap();
        let ids = db.group_position_ids().await.unwrap();
        assert_eq!(
            ids,
            [("Zeus".to_string(), mp.id), ("Gaia".to_string(), mp.id)]
        );
    }

    #[tokio::test]
    async fn check_constraint_violation_is_an_error() {
        let db = Db::connect_in_memory().await.unwrap();
        let err = sqlx::query("INSERT INTO accounts (broker, external_id) VALUES ('schwab', 'x')")
            .execute(&db.pool)
            .await
            .unwrap_err();
        assert!(
            err.as_database_error()
                .is_some_and(|e| e.is_check_violation())
        );
    }
}
