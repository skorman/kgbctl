use chrono::{DateTime, Utc};

use super::{ts, Db, NewFill, NewOrder, Order, OrderSide, OrderStatus, OrderType};

impl Db {
    /// Insert or update an order keyed on `(account_id, broker_order_id)`,
    /// returning its id.
    pub async fn upsert_order(&self, account_id: i64, o: &NewOrder) -> sqlx::Result<i64> {
        let placed_at = ts(o.placed_at);
        let updated_at = ts(o.updated_at);
        let closed_at = o.closed_at.map(ts);

        sqlx::query_scalar!(
            r#"
            INSERT INTO orders (
                account_id, instrument_id, broker_order_id, side, order_type, time_in_force,
                quantity, limit_price, stop_price, filled_quantity, avg_fill_price, fees_usd,
                status, placed_at, updated_at, closed_at, raw)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (account_id, broker_order_id) DO UPDATE SET
                time_in_force   = excluded.time_in_force,
                quantity        = excluded.quantity,
                limit_price     = excluded.limit_price,
                stop_price      = excluded.stop_price,
                filled_quantity = excluded.filled_quantity,
                avg_fill_price  = excluded.avg_fill_price,
                fees_usd        = excluded.fees_usd,
                status          = excluded.status,
                updated_at      = excluded.updated_at,
                closed_at       = excluded.closed_at,
                raw             = excluded.raw
            RETURNING id
            "#,
            account_id,
            o.instrument_id,
            o.broker_order_id,
            o.side,
            o.order_type,
            o.time_in_force,
            o.quantity,
            o.limit_price,
            o.stop_price,
            o.filled_quantity,
            o.avg_fill_price,
            o.fees_usd,
            o.status,
            placed_at,
            updated_at,
            closed_at,
            o.raw,
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Record an execution. Re-inserting a fill already seen is a no-op.
    /// Returns whether the fill was new.
    pub async fn insert_fill(&self, order_id: i64, f: &NewFill) -> sqlx::Result<bool> {
        let executed_at = ts(f.executed_at);
        let result = sqlx::query!(
            r#"
            INSERT INTO fills (order_id, broker_fill_id, quantity, price, fee_usd, executed_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT (order_id, broker_fill_id) DO NOTHING
            "#,
            order_id,
            f.broker_fill_id,
            f.quantity,
            f.price,
            f.fee_usd,
            executed_at,
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Set an order's `fees_usd` to the sum of its recorded fills' fees, for
    /// brokers whose fills of one order can arrive across several syncs.
    pub async fn refresh_order_fees(&self, order_id: i64) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            UPDATE orders
            SET fees_usd = (SELECT COALESCE(SUM(fee_usd), 0) FROM fills WHERE order_id = ?1)
            WHERE id = ?1
            "#,
            order_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Execution time of the newest fill recorded for `account_id`: where the
    /// next incremental executions fetch picks up.
    pub async fn latest_fill_time(&self, account_id: i64) -> sqlx::Result<Option<DateTime<Utc>>> {
        sqlx::query_scalar!(
            r#"
            SELECT MAX(f.executed_at) AS "executed_at: DateTime<Utc>"
            FROM fills f
            JOIN orders o ON o.id = f.order_id
            WHERE o.account_id = ?
            "#,
            account_id,
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Orders that are still working (pending, open, or partially filled).
    pub async fn list_open_orders(&self) -> sqlx::Result<Vec<Order>> {
        sqlx::query_as!(
            Order,
            r#"
            SELECT id, account_id, instrument_id, broker_order_id,
                   side AS "side: OrderSide", order_type AS "order_type: OrderType",
                   time_in_force, quantity, limit_price, stop_price, filled_quantity,
                   avg_fill_price, fees_usd, status AS "status: OrderStatus",
                   placed_at AS "placed_at: DateTime<Utc>",
                   updated_at AS "updated_at: DateTime<Utc>",
                   closed_at AS "closed_at: DateTime<Utc>", raw
            FROM orders
            WHERE status IN ('pending', 'open', 'partially_filled')
            ORDER BY placed_at
            "#
        )
        .fetch_all(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{AssetClass, Broker, NewInstrument};

    #[tokio::test]
    async fn order_upsert_and_fills_round_trip() {
        let db = Db::connect_in_memory().await.unwrap();
        let account = db.upsert_account(Broker::Kraken, "main", None, "USD").await.unwrap();
        let btc = db
            .upsert_instrument(&NewInstrument {
                symbol: "BTC".into(),
                asset_class: AssetClass::Crypto,
                currency: "USD".into(),
                exchange: None,
                name: None,
            })
            .await
            .unwrap();

        let placed = "2026-11-15T14:30:00.250Z".parse::<DateTime<Utc>>().unwrap();
        let mut order = NewOrder {
            instrument_id: btc,
            broker_order_id: "OABC-123".into(),
            side: OrderSide::Buy,
            order_type: OrderType::StopLimit,
            time_in_force: Some("GTC".into()),
            quantity: 0.5,
            limit_price: Some(90_000.0),
            stop_price: Some(89_500.0),
            filled_quantity: 0.0,
            avg_fill_price: None,
            fees_usd: 0.0,
            status: OrderStatus::Open,
            placed_at: placed,
            updated_at: placed,
            closed_at: None,
            raw: None,
        };
        let id = db.upsert_order(account, &order).await.unwrap();
        assert_eq!(db.latest_fill_time(account).await.unwrap(), None);

        let open = db.list_open_orders().await.unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].order_type, OrderType::StopLimit);
        assert_eq!(open[0].placed_at, placed);

        let fill = NewFill {
            broker_fill_id: "T1".into(),
            quantity: 0.5,
            price: 89_900.0,
            fee_usd: 12.5,
            executed_at: placed,
        };
        assert!(db.insert_fill(id, &fill).await.unwrap());
        assert!(!db.insert_fill(id, &fill).await.unwrap());
        assert_eq!(db.latest_fill_time(account).await.unwrap(), Some(placed));
        db.refresh_order_fees(id).await.unwrap();
        assert_eq!(db.list_open_orders().await.unwrap()[0].fees_usd, 12.5);

        order.status = OrderStatus::Filled;
        order.filled_quantity = 0.5;
        order.closed_at = Some(placed);
        assert_eq!(db.upsert_order(account, &order).await.unwrap(), id);
        assert!(db.list_open_orders().await.unwrap().is_empty());

        let fills: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fills")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(fills, 1);
    }
}
