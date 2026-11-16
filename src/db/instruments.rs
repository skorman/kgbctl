use sqlx::SqliteConnection;

use super::{Broker, Db, NewInstrument};

impl Db {
    /// Insert or update an instrument keyed on `(symbol, asset_class, currency)`,
    /// returning its id.
    pub async fn upsert_instrument(&self, instrument: &NewInstrument) -> sqlx::Result<i64> {
        let mut conn = self.pool.acquire().await?;
        upsert_instrument(&mut conn, instrument).await
    }

    /// Map a broker-specific identifier (IBKR conId, Kraken 'XXBT') to an instrument.
    pub async fn link_broker_id(
        &self,
        broker: Broker,
        external_id: &str,
        instrument_id: i64,
    ) -> sqlx::Result<()> {
        let mut conn = self.pool.acquire().await?;
        link_broker_id(&mut conn, broker, external_id, instrument_id).await
    }

    pub async fn resolve_broker_id(
        &self,
        broker: Broker,
        external_id: &str,
    ) -> sqlx::Result<Option<i64>> {
        sqlx::query_scalar!(
            "SELECT instrument_id FROM instrument_broker_ids WHERE broker = ? AND external_id = ?",
            broker,
            external_id,
        )
        .fetch_optional(&self.pool)
        .await
    }
}

pub(super) async fn upsert_instrument(
    conn: &mut SqliteConnection,
    i: &NewInstrument,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"
        INSERT INTO instruments (symbol, asset_class, currency, exchange, name)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT (symbol, asset_class, currency) DO UPDATE SET
            exchange = COALESCE(excluded.exchange, instruments.exchange),
            name     = COALESCE(excluded.name, instruments.name)
        RETURNING id
        "#,
        i.symbol,
        i.asset_class,
        i.currency,
        i.exchange,
        i.name,
    )
    .fetch_one(conn)
    .await
}

pub(super) async fn link_broker_id(
    conn: &mut SqliteConnection,
    broker: Broker,
    external_id: &str,
    instrument_id: i64,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO instrument_broker_ids (broker, external_id, instrument_id)
        VALUES (?, ?, ?)
        ON CONFLICT (broker, external_id) DO UPDATE SET instrument_id = excluded.instrument_id
        "#,
        broker,
        external_id,
        instrument_id,
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AssetClass;

    #[tokio::test]
    async fn upsert_reuses_seeded_instrument_and_links_broker_id() {
        let db = Db::connect_in_memory().await.unwrap();

        // NVDA is seeded by 0002_seed_groups.sql.
        let nvda = NewInstrument {
            symbol: "NVDA".into(),
            asset_class: AssetClass::Stock,
            currency: "USD".into(),
            exchange: Some("NASDAQ".into()),
            name: None,
        };
        let a = db.upsert_instrument(&nvda).await.unwrap();
        let b = db.upsert_instrument(&nvda).await.unwrap();
        assert_eq!(a, b);

        db.link_broker_id(Broker::Ibkr, "4815747", a).await.unwrap();
        assert_eq!(db.resolve_broker_id(Broker::Ibkr, "4815747").await.unwrap(), Some(a));
        assert_eq!(db.resolve_broker_id(Broker::Kraken, "4815747").await.unwrap(), None);
    }
}
