use chrono::{DateTime, Utc};

use super::{AssetClass, Db, GroupTotal, Instrument, PositionGroup};

impl Db {
    pub async fn list_groups(&self) -> sqlx::Result<Vec<PositionGroup>> {
        sqlx::query_as!(
            PositionGroup,
            r#"
            SELECT id, name, description, created_at AS "created_at: DateTime<Utc>"
            FROM position_groups
            ORDER BY id
            "#
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Look up a group by name (case-insensitive).
    pub async fn find_group(&self, name: &str) -> sqlx::Result<Option<PositionGroup>> {
        sqlx::query_as!(
            PositionGroup,
            r#"
            SELECT id, name, description, created_at AS "created_at: DateTime<Utc>"
            FROM position_groups
            WHERE name = ?
            "#,
            name,
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn group_members(&self, group_id: i64) -> sqlx::Result<Vec<Instrument>> {
        sqlx::query_as!(
            Instrument,
            r#"
            SELECT i.id, i.symbol, i.asset_class AS "asset_class: AssetClass",
                   i.currency, i.exchange, i.name
            FROM position_group_members m
            JOIN instruments i ON i.id = m.instrument_id
            WHERE m.group_id = ?
            ORDER BY i.symbol
            "#,
            group_id,
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn add_group_member(
        &self,
        group_id: i64,
        instrument_id: i64,
        target_weight: Option<f64>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO position_group_members (group_id, instrument_id, target_weight)
            VALUES (?, ?, ?)
            ON CONFLICT (group_id, instrument_id) DO UPDATE SET target_weight = excluded.target_weight
            "#,
            group_id,
            instrument_id,
            target_weight,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_group_member(&self, group_id: i64, instrument_id: i64) -> sqlx::Result<()> {
        sqlx::query!(
            "DELETE FROM position_group_members WHERE group_id = ? AND instrument_id = ?",
            group_id,
            instrument_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Per-group value and P/L (`v_group_totals`).
    pub async fn group_totals(&self) -> sqlx::Result<Vec<GroupTotal>> {
        sqlx::query_as!(
            GroupTotal,
            r#"
            SELECT name AS "name!", value_usd AS "value_usd?: f64", cost_usd AS "cost_usd?: f64",
                   pl_usd AS "pl_usd?: f64", pl_pct AS "pl_pct?: f64"
            FROM v_group_totals
            ORDER BY name
            "#
        )
        .fetch_all(&self.pool)
        .await
    }

    /// (group name, position id) for every open position in a group. A position
    /// whose instrument is in several groups appears once per group.
    pub async fn group_position_ids(&self) -> sqlx::Result<Vec<(String, i64)>> {
        let rows = sqlx::query!(
            r#"
            SELECT g.name, p.id AS "position_id!"
            FROM position_groups g
            JOIN position_group_members m ON m.group_id = g.id
            JOIN positions p              ON p.instrument_id = m.instrument_id AND p.quantity != 0
            ORDER BY g.id, p.id
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.name, r.position_id)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_seed_groups() {
        let db = Db::connect_in_memory().await.unwrap();

        let names: Vec<_> = db
            .list_groups()
            .await
            .unwrap()
            .into_iter()
            .map(|g| g.name)
            .collect();
        assert_eq!(names, ["Zeus", "Ares", "Prometheus", "Gaia"]);

        let zeus = db.find_group("zeus").await.unwrap().unwrap();
        let members = db.group_members(zeus.id).await.unwrap();
        assert_eq!(members.len(), 19);
        assert!(members.iter().all(|i| i.asset_class == AssetClass::Stock));
    }

    #[tokio::test]
    async fn add_and_remove_member() {
        let db = Db::connect_in_memory().await.unwrap();
        let gaia = db.find_group("Gaia").await.unwrap().unwrap();
        let before = db.group_members(gaia.id).await.unwrap();

        // NVDA is seeded but not in Gaia.
        let nvda = db.find_group("Prometheus").await.unwrap().unwrap();
        let nvda = db
            .group_members(nvda.id)
            .await
            .unwrap()
            .into_iter()
            .find(|i| i.symbol == "NVDA")
            .unwrap();

        db.add_group_member(gaia.id, nvda.id, Some(0.1))
            .await
            .unwrap();
        assert_eq!(
            db.group_members(gaia.id).await.unwrap().len(),
            before.len() + 1
        );

        db.remove_group_member(gaia.id, nvda.id).await.unwrap();
        assert_eq!(db.group_members(gaia.id).await.unwrap(), before);
    }
}
