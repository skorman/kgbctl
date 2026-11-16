use super::{Db, KrakenLedgerEntry};

impl Db {
    /// Store ledger entries for `account_id`, atomically. Entries already stored
    /// are skipped, so overlapping fetches are harmless. Returns how many were new.
    pub async fn insert_kraken_ledger(
        &self,
        account_id: i64,
        entries: &[KrakenLedgerEntry],
    ) -> sqlx::Result<u64> {
        let mut tx = self.pool.begin().await?;
        let mut inserted = 0;

        for e in entries {
            inserted += sqlx::query!(
                r#"
                INSERT INTO kraken_ledger (
                    ledger_id, account_id, refid, time, type, subtype, asset,
                    amount, fee, balance, raw)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT (ledger_id) DO NOTHING
                "#,
                e.ledger_id,
                account_id,
                e.refid,
                e.time,
                e.entry_type,
                e.subtype,
                e.asset,
                e.amount,
                e.fee,
                e.balance,
                e.raw,
            )
            .execute(&mut *tx)
            .await?
            .rows_affected();
        }

        tx.commit().await?;
        Ok(inserted)
    }

    /// Time of the newest stored ledger entry for `account_id`: where the next
    /// incremental fetch picks up.
    pub async fn kraken_ledger_cursor(&self, account_id: i64) -> sqlx::Result<Option<f64>> {
        sqlx::query_scalar!(
            r#"SELECT MAX(time) AS "time: f64" FROM kraken_ledger WHERE account_id = ?"#,
            account_id,
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Every stored ledger entry for `account_id`, oldest first.
    pub async fn kraken_ledger(&self, account_id: i64) -> sqlx::Result<Vec<KrakenLedgerEntry>> {
        sqlx::query_as!(
            KrakenLedgerEntry,
            r#"
            SELECT ledger_id, refid, time, type AS entry_type, subtype, asset,
                   amount, fee, balance, raw
            FROM kraken_ledger
            WHERE account_id = ?
            ORDER BY time, ledger_id
            "#,
            account_id,
        )
        .fetch_all(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Broker;

    fn entry(id: &str, time: f64) -> KrakenLedgerEntry {
        KrakenLedgerEntry {
            ledger_id: id.into(),
            refid: format!("R{id}"),
            time,
            entry_type: "deposit".into(),
            subtype: None,
            asset: "XXBT".into(),
            amount: 1.0,
            fee: 0.0,
            balance: Some(1.0),
            raw: None,
        }
    }

    #[tokio::test]
    async fn ledger_insert_is_idempotent_and_tracks_cursor() {
        let db = Db::connect_in_memory().await.unwrap();
        let account = db
            .upsert_account(Broker::Kraken, "main", None, "USD")
            .await
            .unwrap();
        assert_eq!(db.kraken_ledger_cursor(account).await.unwrap(), None);

        let batch = [entry("L2", 200.0), entry("L1", 100.0)];
        assert_eq!(db.insert_kraken_ledger(account, &batch).await.unwrap(), 2);
        assert_eq!(db.insert_kraken_ledger(account, &batch).await.unwrap(), 0);

        // An overlapping fetch only adds what is new.
        let next = [entry("L2", 200.0), entry("L3", 300.0)];
        assert_eq!(db.insert_kraken_ledger(account, &next).await.unwrap(), 1);

        assert_eq!(db.kraken_ledger_cursor(account).await.unwrap(), Some(300.0));
        let ids: Vec<_> = db
            .kraken_ledger(account)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.ledger_id)
            .collect();
        assert_eq!(ids, ["L1", "L2", "L3"]);
    }
}
