use super::*;

#[derive(Serialize)]
pub struct UserSummary {
    #[serde(flatten)]
    pub user: UserAccount,
    pub billing: UserBillingProfile,
    pub response_bytes: u64,
}

#[derive(Serialize)]
pub struct UserSummaryPage {
    pub items: Vec<UserSummary>,
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
}

impl Database {
    pub async fn user_summaries(
        &self,
        search: &str,
        page: u32,
        per_page: u32,
        month: &str,
    ) -> anyhow::Result<UserSummaryPage> {
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
        let mut tx = self.pool.begin().await?;
        let base = " FROM users u JOIN user_routing_ids r ON r.user_id = u.id AND r.active = 1 WHERE u.deleted_at IS NULL";
        // SQLite lower() handles ASCII only. Preserve Unicode search without
        // changing the schema or requiring a SQLite extension.
        let (filter, search) = if search.is_ascii() {
            (format!("{base} AND instr(lower(u.display_name || ' ' || COALESCE(u.username, '') || ' ' || u.email || ' ' || r.routing_id), lower(?)) > 0"), search.to_owned())
        } else {
            let needle = search.to_lowercase();
            let query = format!("SELECT u.id, u.display_name || ' ' || COALESCE(u.username, '') || ' ' || u.email || ' ' || r.routing_id AS search_text {base}");
            // SQL fragments are literals; the Unicode search runs in Rust.
            let mut rows = sqlx::query(sqlx::AssertSqlSafe(query)).fetch(&mut *tx);
            let mut ids = Vec::new();
            use futures_util::TryStreamExt;
            while let Some(row) = rows.try_next().await? {
                if row
                    .try_get::<String, _>("search_text")?
                    .to_lowercase()
                    .contains(&needle)
                {
                    ids.push(row.try_get::<i64, _>("id")?);
                }
            }
            (
                format!("{base} AND u.id IN (SELECT value FROM json_each(?))"),
                serde_json::to_string(&ids)?,
            )
        };
        // Both filter branches contain only literal SQL. Search, pagination and
        // month values remain bound parameters, never interpolated text.
        let total: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*){filter}")))
                .bind(&search)
                .fetch_one(&mut *tx)
                .await?;
        // Page before joining traffic and billing data. No per-user database queries.
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!("WITH selected AS (SELECT u.*, r.routing_id, r.created_at AS routing_rotated_at {filter} ORDER BY u.id LIMIT ? OFFSET ?) SELECT u.*, u.id AS user_id, g.id AS group_id, g.name AS group_name, gq.monthly_limit_bytes AS group_limit, uq.mode AS quota_mode, uq.monthly_limit_bytes AS user_limit, COALESCE(t.response_bytes, 0) AS response_bytes FROM selected u LEFT JOIN group_members gm ON gm.user_id = u.id AND gm.is_billing = 1 LEFT JOIN groups g ON g.id = gm.group_id LEFT JOIN group_quota_settings gq ON gq.group_id = g.id LEFT JOIN user_quota_overrides uq ON uq.user_id = u.id LEFT JOIN user_traffic_monthly t ON t.user_id = u.id AND t.month = ? ORDER BY u.id")))
            .bind(&search).bind(i64::from(per_page)).bind(i64::from(page - 1) * i64::from(per_page)).bind(month).fetch_all(&mut *tx).await?;
        let items = rows
            .into_iter()
            .map(|row| {
                let billing = billing_profile_from_row(&row)?;
                let response_bytes = as_u64(row.try_get("response_bytes")?);
                Ok::<_, sqlx::Error>(UserSummary {
                    user: user_account_from_row(row)?,
                    billing,
                    response_bytes,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit().await?;
        Ok(UserSummaryPage {
            items,
            page,
            per_page,
            total: as_u64(total),
        })
    }
}

pub(super) fn billing_profile_from_row(row: &SqliteRow) -> Result<UserBillingProfile, sqlx::Error> {
    Ok(UserBillingProfile {
        user_id: row.try_get("user_id")?,
        group_id: row.try_get("group_id")?,
        group_name: row.try_get("group_name")?,
        group_monthly_limit_bytes: row.try_get::<Option<i64>, _>("group_limit")?.map(as_u64),
        quota_mode: row
            .try_get::<Option<String>, _>("quota_mode")?
            .unwrap_or_else(|| "default".to_string()),
        user_monthly_limit_bytes: row.try_get::<Option<i64>, _>("user_limit")?.map(as_u64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn summaries_match_billing_and_usage_and_paginate_search() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let first = database
            .create_user("admin", "first@example.com", "First", 12)
            .await
            .unwrap()
            .unwrap();
        let second = database
            .create_user("admin", "second@example.com", "Second", 12)
            .await
            .unwrap()
            .unwrap();
        let group = database
            .create_billing_group("admin", "Engineering", Some(1000))
            .await
            .unwrap()
            .unwrap();
        assert!(database
            .set_user_billing_profile("admin", first.id, Some(group.id), "custom", Some(2000))
            .await
            .unwrap());
        sqlx::query("INSERT INTO user_traffic_monthly (month, user_id, request_count, response_bytes, error_count) VALUES ('2026-10', ?, 2, 123, 0)").bind(first.id).execute(&database.pool).await.unwrap();
        let page = database.user_summaries("", 1, 1, "2026-10").await.unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].user.id, first.id);
        assert_eq!(page.items[0].response_bytes, 123);
        let existing = database
            .user_billing_profile(first.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::to_value(&page.items[0].billing).unwrap(),
            serde_json::to_value(existing).unwrap()
        );
        let next = database.user_summaries("", 2, 1, "2026-10").await.unwrap();
        assert_eq!(next.items[0].user.id, second.id);
        assert_eq!(next.items[0].response_bytes, 0);
        assert_eq!(
            database
                .user_summaries("FIRST@", 1, 10, "2026-10")
                .await
                .unwrap()
                .total,
            1
        );
        assert_eq!(
            database
                .user_summaries("%", 1, 10, "2026-10")
                .await
                .unwrap()
                .total,
            0
        );
        assert!(database.soft_delete_user("admin", first.id).await.unwrap());
        assert_eq!(
            database
                .user_summaries("", 1, 10, "2026-10")
                .await
                .unwrap()
                .total,
            1
        );
        database
            .create_user("admin", "unicode@example.com", "ÜBER 中文", 12)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            database
                .user_summaries("über", 1, 10, "2026-10")
                .await
                .unwrap()
                .total,
            1
        );
        assert_eq!(
            database
                .user_summaries("中文", 1, 10, "2026-10")
                .await
                .unwrap()
                .total,
            1
        );
        for search in ["' OR 1=1 --", "中文' OR 1=1 --"] {
            let page = database
                .user_summaries(search, 1, 10, "2026-10")
                .await
                .unwrap();
            assert_eq!(page.total, 0);
            assert!(page.items.is_empty());
        }
    }
}
