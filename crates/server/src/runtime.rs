//! One publication boundary for validated configuration and derived clients.
use crate::config::Config;
use reqwest::Client;
use std::sync::{Arc, RwLock};
use webauthn_rs::prelude::Webauthn;
#[derive(Clone)]
pub(crate) struct RuntimeState {
    pub config: Arc<Config>,
    pub client: Client,
    pub webauthn: Option<Arc<Webauthn>>,
    pub revision: String,
}
impl RuntimeState {
    pub fn from_parts(
        config: Arc<RwLock<Arc<Config>>>,
        client: Arc<RwLock<Client>>,
        webauthn: Arc<RwLock<Option<Arc<Webauthn>>>>,
    ) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            config: config.read().unwrap().clone(),
            client: client.read().unwrap().clone(),
            webauthn: webauthn.read().unwrap().clone(),
            revision: uuid::Uuid::new_v4().to_string(),
        }))
    }
}

// Pin one generation for handler execution so multiple config/client reads in
// the same request cannot straddle a live update. Detached work captures inputs.
tokio::task_local! { static REQUEST_RUNTIME: RuntimeState; }
pub(crate) fn request_config() -> Option<Arc<Config>> {
    REQUEST_RUNTIME
        .try_with(|runtime| runtime.config.clone())
        .ok()
}
pub(crate) fn request_client() -> Option<Client> {
    REQUEST_RUNTIME
        .try_with(|runtime| runtime.client.clone())
        .ok()
}
pub(crate) fn request_webauthn() -> Option<Option<Arc<Webauthn>>> {
    REQUEST_RUNTIME
        .try_with(|runtime| runtime.webauthn.clone())
        .ok()
}
pub(crate) async fn pin(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let snapshot = state.runtime.read().unwrap().clone();
    REQUEST_RUNTIME.scope(snapshot, next.run(request)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn request_reads_keep_their_generation_during_a_live_update() {
        let app = crate::build_application(Config::default()).await.unwrap();
        let snapshot = app.state.runtime.read().unwrap().clone();
        REQUEST_RUNTIME
            .scope(snapshot, async {
                let before = app.state.config().site.title.clone();
                {
                    let mut live = app.state.runtime.write().unwrap();
                    Arc::make_mut(&mut live.config).site.title = "Updated live".into();
                }
                assert_eq!(app.state.config().site.title, before);
            })
            .await;
        assert_eq!(app.state.config().site.title, "Updated live");
    }
}
