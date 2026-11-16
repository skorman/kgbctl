use chrono::{DateTime, Utc};

use super::{Account, Broker, Db};

impl Db {
    /// Insert or update an account, returning its id.
    pub async fn upsert_account(
        &self,
        broker: Broker,
        external_id: &str,
        name: Option<&str>,
        base_currency: &str,
    ) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            r#"
            INSERT INTO accounts (broker, external_id, name, base_currency)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (broker, external_id) DO UPDATE SET
                name          = COALESCE(excluded.name, accounts.name),
                base_currency = excluded.base_currency
            RETURNING id
            "#,
            broker,
            external_id,
            name,
            base_currency,
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_accounts(&self) -> sqlx::Result<Vec<Account>> {
        sqlx::query_as!(
            Account,
            r#"
            SELECT id, broker AS "broker: Broker", external_id, name, base_currency,
                   created_at AS "created_at: DateTime<Utc>"
            FROM accounts
            ORDER BY broker, external_id
            "#
        )
        .fetch_all(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upsert_account_is_idempotent() {
        let db = Db::connect_in_memory().await.unwrap();

        let a = db.upsert_account(Broker::Ibkr, "U1234567", Some("Main"), "USD").await.unwrap();
        let b = db.upsert_account(Broker::Ibkr, "U1234567", None, "USD").await.unwrap();
        assert_eq!(a, b);

        let accounts = db.list_accounts().await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].broker, Broker::Ibkr);
        // A later upsert without a name keeps the existing one.
        assert_eq!(accounts[0].name.as_deref(), Some("Main"));
    }
}
