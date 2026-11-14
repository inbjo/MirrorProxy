use std::{sync::Arc, time::Duration};

use axum::{body::Body, response::Response};
use bytes::Bytes;
use futures_util::{stream, StreamExt};

use super::*;
use crate::{
    observability::Observability,
    traffic::{track_proxy_response, ProxyTraffic, ReservationError},
};

const MONTH: &str = "2026-07";
const LIMIT: u64 = 1_000;
const RESERVED: u64 = 100;

fn traffic(database: &Database, user_id: Option<i64>) -> ProxyTraffic {
    traffic_with_method(database, user_id, "GET")
}

fn traffic_with_method(database: &Database, user_id: Option<i64>, method: &str) -> ProxyTraffic {
    ProxyTraffic::new(
        Arc::new(database.clone()),
        Arc::new(Observability::new().unwrap()),
        "2026-07-31".into(),
        MONTH.into(),
        "npm",
        method.into(),
        "/npm/package".into(),
        2,
        user_id,
        30,
        GeoLocation::default(),
    )
}

async fn reserved(database: &Database) -> i64 {
    sqlx::query_scalar("SELECT reserved_bytes FROM traffic_monthly WHERE month = ?")
        .bind(MONTH)
        .fetch_one(&database.pool)
        .await
        .unwrap()
}

async fn assert_settled(database: &Database, billed: u64, errors: u64) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let usage = database.traffic_overview(MONTH).await.unwrap();
            if usage.request_count != 0 {
                assert_eq!(usage.request_count, 1, "settle exactly once");
                assert_eq!(usage.response_bytes, billed);
                assert_eq!(usage.error_count, errors);
                assert_eq!(reserved(database).await, 0);
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled request must settle its reservation");
}

#[tokio::test]
async fn dropping_unpolled_response_releases_reservation_without_billing_unsent_bytes() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let guard = traffic(&database, None)
        .reserve(Some(LIMIT), None, RESERVED)
        .await
        .unwrap();
    assert_eq!(reserved(&database).await, RESERVED as i64);
    drop(track_proxy_response(
        Response::new(Body::from("unsent")),
        guard,
    ));
    assert_settled(&database, 0, 1).await;
}

#[tokio::test]
async fn head_and_bodyless_statuses_release_quota_without_recording_a_disconnect() {
    for (method, status) in [("HEAD", 200), ("GET", 204), ("GET", 304)] {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let guard = traffic_with_method(&database, None, method)
            .reserve(Some(LIMIT), None, RESERVED)
            .await
            .unwrap();
        let response = Response::builder()
            .status(status)
            .body(Body::empty())
            .unwrap();
        // The HTTP server may never poll the response body for these statuses.
        drop(track_proxy_response(response, guard));
        assert_settled(&database, 0, 0).await;
    }
}

#[tokio::test]
async fn dropping_pending_body_bills_partial_delivery_and_releases_all_quota_scopes() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let user = database
        .create_user("admin", "cancel@example.com", "Cancel", 12)
        .await
        .unwrap()
        .unwrap();
    let group = database
        .create_billing_group("admin", "Cancel", Some(LIMIT))
        .await
        .unwrap()
        .unwrap();
    database
        .set_user_billing_profile("admin", user.id, Some(group.id), "default", None)
        .await
        .unwrap();
    // A request remains attributed to its original month when dropped later.
    assert!(database
        .try_reserve_monthly_bytes("2026-08", LIMIT, RESERVED)
        .await
        .unwrap());
    let guard = traffic(&database, Some(user.id))
        .reserve(Some(LIMIT), Some(LIMIT), RESERVED)
        .await
        .unwrap();
    let body = stream::once(async { Ok::<_, std::io::Error>(Bytes::from_static(b"hello")) })
        .chain(stream::pending());
    let mut body = track_proxy_response(Response::new(Body::from_stream(body)), guard)
        .into_body()
        .into_data_stream();
    assert_eq!(body.next().await.unwrap().unwrap(), "hello");
    assert!(futures_util::poll!(body.next()).is_pending());
    drop(body);
    assert_settled(&database, 10, 1).await;
    for table in ["user_traffic_monthly", "group_traffic_monthly"] {
        // Table names are fixed fixture identifiers from the list above.
        let row: (i64, i64, i64) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT response_bytes, reserved_bytes, request_count FROM {table} WHERE month = ?"
        )))
        .bind(MONTH)
        .fetch_one(&database.pool)
        .await
        .unwrap();
        assert_eq!(row, (10, 0, 1));
    }
    let other_month: i64 =
        sqlx::query_scalar("SELECT reserved_bytes FROM traffic_monthly WHERE month = '2026-08'")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(other_month, RESERVED as i64);
    let delivered: i64 = sqlx::query_scalar("SELECT SUM(response_bytes) FROM geo_traffic_daily")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert_eq!(delivered, 5);
}

#[tokio::test]
async fn dropping_after_last_chunk_without_polling_eof_still_settles() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let guard = traffic(&database, None)
        .reserve(Some(LIMIT), None, RESERVED)
        .await
        .unwrap();
    let mut body = track_proxy_response(Response::new(Body::from("hello")), guard)
        .into_body()
        .into_data_stream();
    assert_eq!(body.next().await.unwrap().unwrap(), "hello");
    drop(body);
    assert_settled(&database, 10, 1).await;
}

#[tokio::test]
async fn stream_error_is_terminal_and_settles_once() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let guard = traffic(&database, None)
        .reserve(Some(LIMIT), None, RESERVED)
        .await
        .unwrap();
    let body = stream::iter([
        Ok(Bytes::from_static(b"hello")),
        Err(std::io::Error::other("upstream disconnected")),
        Err(std::io::Error::other("must not be polled again")),
    ]);
    let mut body = track_proxy_response(Response::new(Body::from_stream(body)), guard)
        .into_body()
        .into_data_stream();
    assert!(body.next().await.unwrap().is_ok());
    assert!(body.next().await.unwrap().is_err());
    assert!(body.next().await.is_none());
    drop(body);
    assert_settled(&database, 10, 1).await;
}

#[tokio::test]
async fn cancellation_while_reserving_cannot_orphan_a_later_commit() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    // The in-memory pool has one connection: reservation is guaranteed to wait.
    let connection = database.pool.acquire().await.unwrap();
    let mut reservation = Box::pin(traffic(&database, None).reserve(Some(LIMIT), None, RESERVED));
    assert!(futures_util::poll!(reservation.as_mut()).is_pending());
    drop(reservation);
    drop(connection);
    assert_settled(&database, 0, 1).await;
}

#[tokio::test]
async fn cancellation_during_eof_settlement_does_not_cancel_or_duplicate_the_write() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let guard = traffic(&database, None)
        .reserve(Some(LIMIT), None, RESERVED)
        .await
        .unwrap();
    let mut body = track_proxy_response(Response::new(Body::from("hello")), guard)
        .into_body()
        .into_data_stream();
    assert!(body.next().await.unwrap().is_ok());
    let connection = database.pool.acquire().await.unwrap();
    assert!(futures_util::poll!(body.next()).is_pending());
    drop(body);
    drop(connection);
    assert_settled(&database, 10, 0).await;
}

#[tokio::test]
async fn quota_rejection_does_not_release_another_requests_reservation() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let first = traffic(&database, None)
        .reserve(Some(RESERVED), None, RESERVED)
        .await
        .unwrap();
    assert!(matches!(
        traffic(&database, None)
            .reserve(Some(RESERVED), None, RESERVED)
            .await,
        Err(ReservationError::Exceeded("global"))
    ));
    assert_eq!(reserved(&database).await, RESERVED as i64);
    assert_eq!(
        database
            .traffic_overview(MONTH)
            .await
            .unwrap()
            .request_count,
        0
    );
    drop(first);
    assert_settled(&database, 0, 1).await;
    assert!(database
        .try_reserve_monthly_bytes(MONTH, RESERVED, RESERVED)
        .await
        .unwrap());
}

#[tokio::test]
async fn public_quota_status_matches_admission_and_ignores_historical_exceeded_flags() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let mut config = Config::default();
    config.quota.enabled = true;
    config.quota.monthly_gb = 1;
    config.quota.bidirectional_accounting = true;
    let limit = 1024 * 1024 * 1024_i64;
    let reservation = (crate::QUOTA_RESERVATION_BYTES * 2) as i64;
    for (billed, reserved, expected) in [
        (0, 0, "available"),
        (limit - reservation, 0, "available"),
        (limit - reservation + 1, 0, "reserved"),
        (61, limit - 61, "reserved"),
        (limit, 0, "exhausted"),
        (limit + 1, 0, "exhausted"),
    ] {
        sqlx::query("INSERT INTO traffic_monthly(month, response_bytes, reserved_bytes, quota_exceeded) VALUES (?, ?, ?, 1) ON CONFLICT(month) DO UPDATE SET response_bytes=excluded.response_bytes, reserved_bytes=excluded.reserved_bytes")
            .bind(MONTH).bind(billed).bind(reserved).execute(&database.pool).await.unwrap();
        assert_eq!(
            crate::public_quota_status(&database, &config, MONTH).await,
            expected
        );
        assert_eq!(
            database
                .try_reserve_monthly_bytes(MONTH, limit as u64, reservation as u64)
                .await
                .unwrap(),
            expected == "available"
        );
    }
    assert_eq!(
        crate::public_quota_status(&database, &config, "2026-08").await,
        "available"
    );
    config.quota.monthly_gb = 0;
    assert_eq!(
        crate::public_quota_status(&database, &config, "2026-08").await,
        "exhausted"
    );
    config.quota.enabled = false;
    assert_eq!(
        crate::public_quota_status(&database, &config, MONTH).await,
        "available"
    );
    database.pool.close().await;
    config.quota.enabled = true;
    assert_eq!(
        crate::public_quota_status(&database, &config, MONTH).await,
        "unavailable"
    );
}

#[tokio::test]
async fn unlimited_user_settlement_preserves_another_global_reservation() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let user = database
        .create_user("admin", "toggle@example.com", "Toggle", 12)
        .await
        .unwrap()
        .unwrap();
    // An in-flight request retains its enabled-quota configuration while a
    // later authenticated request uses the newly disabled global limit.
    let first = traffic(&database, None)
        .reserve(Some(LIMIT), None, RESERVED)
        .await
        .unwrap();
    let later = traffic(&database, Some(user.id))
        .reserve(None, None, RESERVED)
        .await
        .unwrap();
    drop(later);
    tokio::time::timeout(Duration::from_secs(5), async {
        while database
            .traffic_overview(MONTH)
            .await
            .unwrap()
            .request_count
            == 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        reserved(&database).await,
        RESERVED as i64,
        "settling an unlimited request must not release another request's bytes"
    );
    drop(first);
}

#[tokio::test]
async fn changing_billing_group_during_download_settles_original_group() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let user = database
        .create_user("admin", "move@example.com", "Move", 12)
        .await
        .unwrap()
        .unwrap();
    let old = database
        .create_billing_group("admin", "Old", Some(LIMIT))
        .await
        .unwrap()
        .unwrap();
    let new = database
        .create_billing_group("admin", "New", Some(LIMIT))
        .await
        .unwrap()
        .unwrap();
    database
        .set_user_billing_profile("admin", user.id, Some(old.id), "default", None)
        .await
        .unwrap();
    let guard = traffic(&database, Some(user.id))
        .reserve(Some(LIMIT), None, RESERVED)
        .await
        .unwrap();
    database
        .set_user_billing_profile("admin", user.id, Some(new.id), "default", None)
        .await
        .unwrap();
    let body = track_proxy_response(Response::new(Body::from("hello")), guard).into_body();
    axum::body::to_bytes(body, usize::MAX).await.unwrap();
    assert_settled(&database, 10, 0).await;
    let rows: Vec<(i64, i64, i64)> = sqlx::query_as("SELECT group_id, response_bytes, reserved_bytes FROM group_traffic_monthly WHERE month = ?")
        .bind(MONTH).fetch_all(&database.pool).await.unwrap();
    assert_eq!(rows, vec![(old.id, 10, 0)]);
}

#[tokio::test]
async fn group_rejection_rolls_back_global_and_user_reservations_without_global_cap() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let user = database
        .create_user("admin", "reject@example.com", "Reject", 12)
        .await
        .unwrap()
        .unwrap();
    let group = database
        .create_billing_group("admin", "Full", Some(0))
        .await
        .unwrap()
        .unwrap();
    database
        .set_user_billing_profile("admin", user.id, Some(group.id), "default", None)
        .await
        .unwrap();
    let first = traffic(&database, None)
        .reserve(Some(LIMIT), None, RESERVED)
        .await
        .unwrap();
    assert!(matches!(
        traffic(&database, Some(user.id))
            .reserve(None, None, RESERVED)
            .await,
        Err(ReservationError::Exceeded("group"))
    ));
    assert_eq!(reserved(&database).await, RESERVED as i64);
    let pending: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(reserved_bytes), 0) FROM user_traffic_monthly")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(pending, 0);
    drop(first);
    assert_settled(&database, 0, 1).await;
}

#[tokio::test]
async fn concurrent_reservations_cannot_overbook_global_capacity() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..20 {
        let database = database.clone();
        tasks.spawn(async move {
            database
                .try_reserve_monthly_bytes(MONTH, LIMIT, RESERVED)
                .await
                .unwrap()
        });
    }
    let mut accepted = 0;
    while let Some(result) = tasks.join_next().await {
        accepted += usize::from(result.unwrap());
    }
    assert_eq!(accepted, 10);
    assert_eq!(reserved(&database).await, LIMIT as i64);
}
