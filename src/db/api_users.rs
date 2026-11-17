use chrono::{DateTime, Utc};

use super::{ApiUser, Db, ts};

impl Db {
    /// Add a user with the given token hash, returning its id. Fails if the name
    /// is taken (case-insensitive).
    pub async fn create_api_user(
        &self,
        name: &str,
        token_hash: &str,
        can_sync: bool,
    ) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            "INSERT INTO api_users (name, token_hash, can_sync) VALUES (?, ?, ?) RETURNING id",
            name,
            token_hash,
            can_sync,
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Find the user whose token hashes to `token_hash` and record the request
    /// as their latest activity.
    pub async fn authenticate_api_user(&self, token_hash: &str) -> sqlx::Result<Option<ApiUser>> {
        let now = ts(Utc::now());
        sqlx::query_as!(
            ApiUser,
            r#"
            UPDATE api_users SET last_active_at = ?
            WHERE token_hash = ?
            RETURNING id, name, created_at AS "created_at: DateTime<Utc>",
                      last_active_at AS "last_active_at: DateTime<Utc>",
                      can_sync AS "can_sync: bool"
            "#,
            now,
            token_hash,
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Replace a user's token. Returns false if there's no such user.
    pub async fn set_api_user_token(&self, name: &str, token_hash: &str) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "UPDATE api_users SET token_hash = ? WHERE name = ?",
            token_hash,
            name,
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Swap user `id`'s token from `old_hash` to `new_hash`. Returns false if the
    /// token no longer matches, e.g. a concurrent rotation already replaced it.
    pub async fn rotate_api_user_token(
        &self,
        id: i64,
        old_hash: &str,
        new_hash: &str,
    ) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "UPDATE api_users SET token_hash = ? WHERE id = ? AND token_hash = ?",
            new_hash,
            id,
            old_hash,
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Grant or revoke the right to push syncs. Returns false if there's no such user.
    pub async fn set_api_user_sync(&self, name: &str, can_sync: bool) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "UPDATE api_users SET can_sync = ? WHERE name = ?",
            can_sync,
            name,
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Returns false if there's no such user.
    pub async fn delete_api_user(&self, name: &str) -> sqlx::Result<bool> {
        let result = sqlx::query!("DELETE FROM api_users WHERE name = ?", name)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_api_users(&self) -> sqlx::Result<Vec<ApiUser>> {
        sqlx::query_as!(
            ApiUser,
            r#"
            SELECT id, name, created_at AS "created_at: DateTime<Utc>",
                   last_active_at AS "last_active_at: DateTime<Utc>",
                   can_sync AS "can_sync: bool"
            FROM api_users
            ORDER BY name
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
    async fn authenticate_matches_hash_and_records_activity() {
        let db = Db::connect_in_memory().await.unwrap();
        db.create_api_user("Sam", "hash-a", false).await.unwrap();
        assert!(
            db.list_api_users().await.unwrap()[0]
                .last_active_at
                .is_none()
        );

        let user = db.authenticate_api_user("hash-a").await.unwrap().unwrap();
        assert_eq!(user.name, "Sam");
        assert!(user.last_active_at.is_some());
        assert!(!user.can_sync);
        assert!(db.authenticate_api_user("hash-b").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn names_are_unique_ignoring_case() {
        let db = Db::connect_in_memory().await.unwrap();
        db.create_api_user("Sam", "hash-a", false).await.unwrap();
        let err = db
            .create_api_user("sam", "hash-b", false)
            .await
            .unwrap_err();
        assert!(
            err.as_database_error()
                .is_some_and(|e| e.is_unique_violation())
        );
    }

    #[tokio::test]
    async fn rotate_and_delete() {
        let db = Db::connect_in_memory().await.unwrap();
        db.create_api_user("Sam", "old", false).await.unwrap();

        assert!(db.set_api_user_token("sam", "new").await.unwrap());
        assert!(db.authenticate_api_user("old").await.unwrap().is_none());
        assert!(db.authenticate_api_user("new").await.unwrap().is_some());
        assert!(!db.set_api_user_token("nobody", "x").await.unwrap());

        let id = db.list_api_users().await.unwrap()[0].id;
        assert!(!db.rotate_api_user_token(id, "old", "newer").await.unwrap());
        assert!(db.rotate_api_user_token(id, "new", "newer").await.unwrap());
        assert!(db.authenticate_api_user("new").await.unwrap().is_none());
        assert!(db.set_api_user_token("sam", "new").await.unwrap());

        assert!(db.delete_api_user("Sam").await.unwrap());
        assert!(!db.delete_api_user("Sam").await.unwrap());
        assert!(db.list_api_users().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn sync_permission_is_granted_and_revoked_by_name() {
        let db = Db::connect_in_memory().await.unwrap();
        db.create_api_user("Laptop", "h", true).await.unwrap();
        assert!(
            db.authenticate_api_user("h")
                .await
                .unwrap()
                .unwrap()
                .can_sync
        );

        assert!(db.set_api_user_sync("laptop", false).await.unwrap());
        assert!(
            !db.authenticate_api_user("h")
                .await
                .unwrap()
                .unwrap()
                .can_sync
        );
        assert!(!db.set_api_user_sync("nobody", true).await.unwrap());
    }
}
