use std::sync::Arc;

use axum::{body::Body, http::StatusCode, response::Response};
use futures_util::StreamExt;
use tokio::{runtime::Handle, task::JoinHandle};

use crate::{
    database::{Database, HierarchicalReservationOutcome, ProxyTrafficRecord},
    geoip::GeoLocation,
    observability::Observability,
};

#[derive(Debug, thiserror::Error)]
pub enum ReservationError {
    #[error("monthly traffic quota exceeded: {0}")]
    Exceeded(&'static str),
    #[error(transparent)]
    Database(#[from] anyhow::Error),
}

/// Owns accounting from reservation through response-body destruction. Moving
/// this guard into the body also covers cancellation while awaiting an upstream.
pub struct ProxyTraffic {
    database: Arc<Database>,
    observability: Arc<Observability>,
    runtime: Handle,
    record: Option<TrafficRecord>,
    armed: bool,
}

struct TrafficRecord {
    day: String,
    month: String,
    target_code: &'static str,
    method: String,
    path: String,
    status_code: u16,
    response_bytes: u64,
    reserved_bytes: u64,
    accounting_multiplier: u64,
    user_id: Option<i64>,
    group_id: Option<i64>,

    location: GeoLocation,
}

impl ProxyTraffic {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        database: Arc<Database>,
        observability: Arc<Observability>,
        day: String,
        month: String,
        target_code: &'static str,
        method: String,
        path: String,
        accounting_multiplier: u64,
        user_id: Option<i64>,
        _request_event_retention_days: u32,
        location: GeoLocation,
    ) -> Self {
        Self {
            database,
            observability,
            runtime: Handle::current(),
            armed: false,
            record: Some(TrafficRecord {
                day,
                month,
                target_code,
                method,
                path,
                // No response headers were produced if cancellation happens
                // while the handler is still running.
                status_code: 499,
                response_bytes: 0,
                reserved_bytes: 0,
                accounting_multiplier,
                user_id,
                group_id: None,
                location,
            }),
        }
    }

    pub async fn reserve(
        mut self,
        global_limit: Option<u64>,
        default_user_limit: Option<u64>,
        bytes: u64,
    ) -> Result<Self, ReservationError> {
        // Cancellation must not lose a successful COMMIT before its guard is
        // armed. A dropped JoinHandle detaches this task; its returned guard is
        // then dropped and settles the reservation even if nobody receives it.
        let pending = self.database.tasks.enter();
        let task = self.runtime.clone().spawn(async move {
            let _pending = pending;
            let record = self.record.as_mut().expect("traffic has not been settled");
            if let Some(user_id) = record.user_id {
                match self
                    .database
                    .try_reserve_hierarchical_bytes(
                        &record.month,
                        user_id,
                        global_limit,
                        default_user_limit,
                        bytes,
                    )
                    .await?
                {
                    HierarchicalReservationOutcome::Reserved { group_id } => {
                        record.group_id = group_id;
                        record.reserved_bytes = bytes;
                    }
                    HierarchicalReservationOutcome::Exceeded { scope } => {
                        return Err(ReservationError::Exceeded(scope));
                    }
                }
            } else if let Some(limit) = global_limit {
                if !self
                    .database
                    .try_reserve_monthly_bytes(&record.month, limit, bytes)
                    .await?
                {
                    return Err(ReservationError::Exceeded("global"));
                }
                record.reserved_bytes = bytes;
            }
            self.armed = true;
            Ok(self)
        });
        task.await
            .map_err(|error| ReservationError::Database(error.into()))?
    }

    fn settle(&mut self, stream_error: bool) -> Option<JoinHandle<()>> {
        if !self.armed {
            return None;
        }
        self.armed = false;
        let record = self.record.take()?;
        let database = self.database.clone();
        let observability = self.observability.clone();
        // Both EOF/error settlement and Drop use one detached task. Cancelling
        // the response while this task awaits SQLite cannot cancel the write
        // or start a second settlement.
        let pending = database.tasks.enter();
        Some(self.runtime.spawn(async move {
            let _pending = pending;
            if let Err(error) = database
                .record_proxy_response(ProxyTrafficRecord {
                    day: &record.day,
                    month: &record.month,
                    target_code: record.target_code,
                    method: &record.method,
                    path: &record.path,
                    status_code: record.status_code,
                    response_bytes: record
                        .response_bytes
                        .saturating_mul(record.accounting_multiplier),
                    delivered_response_bytes: record.response_bytes,
                    stream_error,
                    reserved_bytes: record.reserved_bytes,
                    user_id: record.user_id,
                    group_id: record.group_id,
                    location: &record.location,
                })
                .await
            {
                tracing::error!(%error, "failed to record proxy traffic");
            }
            observability.observe_proxy_body(
                record.target_code,
                record.status_code,
                record.response_bytes,
                stream_error,
            );
        }))
    }

    async fn finish(&mut self, stream_error: bool) {
        if let Some(task) = self.settle(stream_error) {
            if let Err(error) = task.await {
                tracing::error!(%error, "proxy traffic settlement task failed");
            }
        }
    }
}

impl Drop for ProxyTraffic {
    fn drop(&mut self) {
        self.settle(true);
    }
}

pub fn track_proxy_response(response: Response, mut traffic: ProxyTraffic) -> Response {
    let status = response.status();
    let record = traffic.record.as_mut().expect("traffic is active");
    record.status_code = status.as_u16();
    // Hyper does not poll bodies forbidden by HTTP semantics. These successful
    // responses must release reservations without being counted as disconnects.
    if record.method == "HEAD"
        || status.is_informational()
        || matches!(status, StatusCode::NO_CONTENT | StatusCode::NOT_MODIFIED)
    {
        traffic.settle(false);
        return response;
    }
    let (parts, body) = response.into_parts();
    let tracked = futures_util::stream::unfold(
        (body.into_data_stream(), Some(traffic)),
        |(mut stream, traffic)| async move {
            let mut traffic = traffic?;
            match stream.next().await {
                Some(Ok(chunk)) => {
                    let record = traffic.record.as_mut().expect("traffic is active");
                    record.response_bytes =
                        record.response_bytes.saturating_add(chunk.len() as u64);
                    Some((Ok::<_, axum::Error>(chunk), (stream, Some(traffic))))
                }
                Some(Err(error)) => {
                    traffic.finish(true).await;
                    Some((Err(error), (stream, None)))
                }
                None => {
                    traffic.finish(false).await;
                    None
                }
            }
        },
    );
    Response::from_parts(parts, Body::from_stream(tracked))
}
