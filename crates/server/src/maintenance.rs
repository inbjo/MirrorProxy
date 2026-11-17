//! Background cleanup and a shutdown barrier for detached accounting work.
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
#[derive(Default)]
pub(crate) struct Tasks {
    pending: AtomicUsize,
    changed: tokio::sync::Notify,
}
pub(crate) struct Guard(Arc<Tasks>);
impl Tasks {
    #[cfg(test)]
    pub(crate) fn pending(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }
    pub fn enter(self: &Arc<Self>) -> Guard {
        self.pending.fetch_add(1, Ordering::SeqCst);
        Guard(self.clone())
    }
    pub async fn drain(&self) -> bool {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let notified = self.changed.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                if self.pending.load(Ordering::SeqCst) == 0 {
                    return;
                }
                notified.await;
            }
        })
        .await
        .is_ok()
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.pending.fetch_sub(1, Ordering::SeqCst);
        self.0.changed.notify_waiters();
    }
}
#[derive(Default)]
pub(crate) struct Health {
    pub started: AtomicBool,
    pub alive: AtomicBool,
    pub last_tick: AtomicUsize,
    pub cleanup_ok: AtomicBool,
    pub draining: AtomicBool,
}
struct Alive(Arc<Health>);
impl Drop for Alive {
    fn drop(&mut self) {
        self.0.alive.store(false, Ordering::SeqCst);
    }
}
pub(crate) fn spawn(state: crate::AppState) {
    state.maintenance.started.store(true, Ordering::SeqCst);
    state.maintenance.alive.store(true, Ordering::SeqCst);
    tokio::spawn(async move {
        let _alive = Alive(state.maintenance.clone());
        let mut ticks = tokio::time::interval(Duration::from_secs(15));
        let mut cycle = 0u8;
        loop {
            ticks.tick().await;
            if state.maintenance.draining.load(Ordering::SeqCst) {
                break;
            }
            state.rate_limiter.prune(std::time::Instant::now());
            if cycle == 0 {
                let result = state
                    .database
                    .prune_request_events(state.config().quota.request_event_retention_days)
                    .await;
                state
                    .maintenance
                    .cleanup_ok
                    .store(result.is_ok(), Ordering::SeqCst);
                if let Err(error) = result {
                    tracing::warn!(%error, "request event cleanup failed");
                }
            }
            cycle = (cycle + 1) % 4;
            state.maintenance.last_tick.store(now(), Ordering::SeqCst);
        }
    });
}
fn now() -> usize {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as usize
}
pub(crate) async fn ready(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let database = tokio::time::timeout(Duration::from_secs(2), state.database.ping())
        .await
        .is_ok_and(|r| r.is_ok());
    let started = state.maintenance.started.load(Ordering::SeqCst);
    let worker = !started
        || (state.maintenance.alive.load(Ordering::SeqCst)
            && now().saturating_sub(state.maintenance.last_tick.load(Ordering::SeqCst)) < 90);
    let draining = state.maintenance.draining.load(Ordering::SeqCst);
    let ready = database && worker && !draining;
    (if ready { axum::http::StatusCode::OK } else { axum::http::StatusCode::SERVICE_UNAVAILABLE }, [(axum::http::header::CACHE_CONTROL, "no-store")], axum::Json(serde_json::json!({"status":if ready {"ready"}else{"not_ready"},"database":database,"maintenance":worker,"cleanup":state.maintenance.cleanup_ok.load(Ordering::SeqCst),"draining":draining}))).into_response()
}
pub(crate) async fn drain(state: &crate::AppState) {
    state.maintenance.draining.store(true, Ordering::SeqCst);
    if !state.database.tasks.drain().await {
        tracing::error!("accounting tasks exceeded shutdown deadline");
    }
}

pub(crate) async fn stop_accepting(state: crate::AppState) {
    crate::shutdown_signal().await;
    state.maintenance.draining.store(true, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn drain_waits_for_work_and_cancellation_releases_guards() {
        let tasks = Arc::new(Tasks::default());
        let guard = tasks.enter();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), tasks.drain())
                .await
                .is_err()
        );
        drop(guard);
        assert!(tasks.drain().await);
        let guard = tasks.enter();
        let task = tokio::spawn(async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        });
        task.abort();
        let _ = task.await;
        assert!(tasks.drain().await);
    }
}
