use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use instant_acme::{
    Account, AccountCredentials, AuthorizationStatus, ChallengeType, Identifier, NewAccount,
    NewOrder, OrderStatus, RetryPolicy,
};
use reqwest::Client;
use serde::Serialize;
use tokio::sync::{mpsc, watch, Mutex, RwLock};
use uuid::Uuid;
use x509_parser::pem::parse_x509_pem;

use crate::{
    acme_dns::{build_provider, DnsProvider, DnsRecordHandle},
    config::AcmeConfig,
};

#[derive(Clone, Debug, Serialize)]
pub struct AcmeStatus {
    pub enabled: bool,
    pub challenge: String,
    pub dns_provider: Option<String>,
    pub domains: Vec<String>,
    pub certificate_path: String,
    pub private_key_path: String,
    pub certificate_not_after: Option<i64>,
    pub last_success_at: Option<i64>,
    pub last_error: Option<String>,
    pub running: bool,
    pub direct_https: bool,
    pub http_listen_addr: String,
    pub https_listen_addr: String,
    pub https_active: bool,
}

pub struct AcmeManager {
    config: AcmeConfig,
    challenges: RwLock<HashMap<String, String>>,
    status: RwLock<AcmeStatus>,
    issue_lock: Mutex<()>,
    trigger: mpsc::Sender<()>,
    certificate_generation: watch::Sender<u64>,
}

enum ProvisionedChallenge {
    Http { token: String },
    Dns(DnsRecordHandle),
}

impl AcmeManager {
    pub fn new(config: AcmeConfig) -> (Arc<Self>, mpsc::Receiver<()>) {
        let (trigger, receiver) = mpsc::channel(1);
        let (certificate_generation, _) = watch::channel(0);
        let certificate_path = Path::new(&config.storage_directory).join("fullchain.pem");
        let private_key_path = Path::new(&config.storage_directory).join("privkey.pem");
        let not_after = certificate_not_after(&certificate_path).ok().flatten();
        let status = AcmeStatus {
            enabled: config.enabled,
            challenge: config.challenge.clone(),
            dns_provider: (config.challenge == "dns-01").then(|| config.dns.provider.clone()),
            domains: config.domains.clone(),
            certificate_path: certificate_path.display().to_string(),
            private_key_path: private_key_path.display().to_string(),
            certificate_not_after: not_after,
            last_success_at: None,
            last_error: None,
            running: false,
            direct_https: config.direct_https,
            http_listen_addr: config.http_listen_addr.clone(),
            https_listen_addr: config.https_listen_addr.clone(),
            https_active: false,
        };
        (
            Arc::new(Self {
                config,
                challenges: RwLock::new(HashMap::new()),
                status: RwLock::new(status),
                issue_lock: Mutex::new(()),
                trigger,
                certificate_generation,
            }),
            receiver,
        )
    }

    pub fn spawn(self: Arc<Self>, client: Client, mut receiver: mpsc::Receiver<()>) {
        if !self.config.enabled {
            return;
        }
        tokio::spawn(async move {
            self.refresh(false, &client).await;
            let interval = Duration::from_secs(
                u64::from(self.config.check_interval_hours)
                    .saturating_mul(60 * 60)
                    .max(60),
            );
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => self.refresh(false, &client).await,
                    value = receiver.recv() => {
                        if value.is_none() { break; }
                        self.refresh(true, &client).await;
                    }
                }
            }
        });
    }

    pub async fn status(&self) -> AcmeStatus {
        self.status.read().await.clone()
    }

    pub async fn challenge_response(&self, token: &str) -> Option<String> {
        self.challenges.read().await.get(token).cloned()
    }

    pub fn subscribe_certificates(&self) -> watch::Receiver<u64> {
        self.certificate_generation.subscribe()
    }

    pub(crate) fn notify_certificate_update(&self) {
        self.certificate_generation
            .send_modify(|generation| *generation = generation.saturating_add(1));
    }

    pub async fn set_https_active(&self, active: bool) {
        self.status.write().await.https_active = active;
    }

    pub async fn trigger_renewal(&self) -> Result<()> {
        if !self.config.enabled {
            anyhow::bail!("ACME is disabled");
        }
        if self.status.read().await.running {
            anyhow::bail!("ACME renewal is already running");
        }
        self.trigger
            .try_send(())
            .map_err(|error| anyhow::anyhow!("ACME renewal is already queued: {error}"))?;
        self.status.write().await.running = true;
        Ok(())
    }

    async fn refresh(&self, force: bool, client: &Client) {
        let _guard = self.issue_lock.lock().await;
        let certificate_path = Path::new(&self.config.storage_directory).join("fullchain.pem");
        let not_after = certificate_not_after(&certificate_path).ok().flatten();
        let renew_at = not_after.map(|timestamp| {
            timestamp - i64::from(self.config.renew_before_days).saturating_mul(24 * 60 * 60)
        });
        if !force && renew_at.is_some_and(|timestamp| timestamp > unix_timestamp()) {
            self.status.write().await.certificate_not_after = not_after;
            return;
        }

        {
            let mut status = self.status.write().await;
            status.running = true;
            status.last_error = None;
        }
        match self.issue_certificate(client).await {
            Ok(not_after) => {
                let mut status = self.status.write().await;
                status.running = false;
                status.last_success_at = Some(unix_timestamp());
                status.certificate_not_after = Some(not_after);
                tracing::info!(domains = ?self.config.domains, not_after, "ACME certificate issued");
            }
            Err(error) => {
                tracing::error!(%error, "ACME certificate issuance failed");
                let mut status = self.status.write().await;
                status.running = false;
                status.last_error = Some(error.to_string());
            }
        }
    }

    async fn issue_certificate(&self, client: &Client) -> Result<i64> {
        fs::create_dir_all(&self.config.storage_directory).with_context(|| {
            format!(
                "failed to create ACME storage directory {}",
                self.config.storage_directory
            )
        })?;
        let account = self.load_or_create_account().await?;
        let identifiers = self
            .config
            .domains
            .iter()
            .cloned()
            .map(Identifier::Dns)
            .collect::<Vec<_>>();
        let mut order = account.new_order(&NewOrder::new(&identifiers)).await?;
        let mut provisioned = Vec::new();
        let dns_provider = if self.config.challenge == "dns-01" {
            Some(build_provider(&self.config.dns, client.clone())?)
        } else {
            None
        };

        let authorization_result: Result<()> = async {
            let challenge_type = if self.config.challenge == "dns-01" {
                ChallengeType::Dns01
            } else {
                ChallengeType::Http01
            };
            let mut authorizations = order.authorizations();
            while let Some(result) = authorizations.next().await {
                let mut authorization = result?;
                match authorization.status {
                    AuthorizationStatus::Valid => continue,
                    AuthorizationStatus::Pending => {}
                    status => anyhow::bail!("unexpected ACME authorization status: {status:?}"),
                }
                let challenge =
                    authorization
                        .challenge(challenge_type.clone())
                        .ok_or_else(|| {
                            anyhow::anyhow!("ACME server did not offer {}", self.config.challenge)
                        })?;
                let key_authorization = challenge.key_authorization();
                if challenge_type == ChallengeType::Http01 {
                    let token = challenge.token.clone();
                    self.challenges
                        .write()
                        .await
                        .insert(token.clone(), key_authorization.as_str().to_string());
                    provisioned.push(ProvisionedChallenge::Http { token });
                } else {
                    let domain = challenge.identifier().to_string();
                    let domain = domain.trim_start_matches("*.");
                    let name = format!("_acme-challenge.{domain}");
                    let value = key_authorization.dns_value();
                    let provider = dns_provider
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("ACME DNS provider is unavailable"))?;
                    provisioned.push(ProvisionedChallenge::Dns(
                        provider.present(&name, &value).await?,
                    ));
                }
            }
            if challenge_type == ChallengeType::Dns01 && !provisioned.is_empty() {
                tokio::time::sleep(Duration::from_secs(self.config.dns.propagation_delay_secs))
                    .await;
            }
            let mut authorizations = order.authorizations();
            while let Some(result) = authorizations.next().await {
                let mut authorization = result?;
                match authorization.status {
                    AuthorizationStatus::Valid => continue,
                    AuthorizationStatus::Pending => {}
                    status => anyhow::bail!("unexpected ACME authorization status: {status:?}"),
                }
                authorization
                    .challenge(challenge_type.clone())
                    .ok_or_else(|| {
                        anyhow::anyhow!("ACME server did not offer {}", self.config.challenge)
                    })?
                    .set_ready()
                    .await?;
            }
            Ok(())
        }
        .await;

        let issued = match authorization_result {
            Ok(()) => {
                async {
                    let status = order.poll_ready(&RetryPolicy::default()).await?;
                    if status != OrderStatus::Ready {
                        let mut failures = Vec::new();
                        let mut authorizations = order.authorizations();
                        while let Some(result) = authorizations.next().await {
                            let authorization = result?;
                            for challenge in &authorization.challenges {
                                if let Some(error) = &challenge.error {
                                    failures.push(format!(
                                        "{} {:?}: {error}",
                                        authorization.identifier(),
                                        challenge.r#type
                                    ));
                                }
                            }
                        }
                        let details = if failures.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", failures.join("; "))
                        };
                        anyhow::bail!("ACME order did not become ready: {status:?}{details}");
                    }
                    let private_key = order.finalize().await?;
                    let certificate = order.poll_certificate(&RetryPolicy::default()).await?;
                    Ok::<_, anyhow::Error>((certificate, private_key))
                }
                .await
            }
            Err(error) => Err(error),
        };

        for challenge in provisioned.into_iter().rev() {
            if let Err(error) = self
                .cleanup_challenge(dns_provider.as_deref(), challenge)
                .await
            {
                tracing::warn!(%error, "failed to clean up ACME challenge");
            }
        }

        let (certificate, private_key) = issued?;
        let storage = Path::new(&self.config.storage_directory);
        atomic_write(
            &storage.join("fullchain.pem"),
            certificate.as_bytes(),
            0o644,
        )?;
        atomic_write(&storage.join("privkey.pem"), private_key.as_bytes(), 0o600)?;
        self.notify_certificate_update();
        certificate_not_after(&storage.join("fullchain.pem"))?
            .ok_or_else(|| anyhow::anyhow!("issued certificate has no readable expiry"))
    }

    async fn load_or_create_account(&self) -> Result<Account> {
        let path = Path::new(&self.config.storage_directory).join("account.json");
        let builder = Account::builder()?;
        if path.exists() {
            let raw = fs::read(&path)
                .with_context(|| format!("failed to read ACME account {}", path.display()))?;
            let credentials: AccountCredentials =
                serde_json::from_slice(&raw).context("failed to parse ACME account credentials")?;
            return builder
                .from_credentials(credentials)
                .await
                .context("failed to restore ACME account");
        }
        let contact = format!("mailto:{}", self.config.email.trim());
        let contacts = [contact.as_str()];
        let (account, credentials) = builder
            .create(
                &NewAccount {
                    contact: &contacts,
                    terms_of_service_agreed: true,
                    only_return_existing: false,
                },
                self.config.directory_url.clone(),
                None,
            )
            .await
            .context("failed to create ACME account")?;
        atomic_write(&path, &serde_json::to_vec_pretty(&credentials)?, 0o600)?;
        Ok(account)
    }

    async fn cleanup_challenge(
        &self,
        dns_provider: Option<&dyn DnsProvider>,
        challenge: ProvisionedChallenge,
    ) -> Result<()> {
        match challenge {
            ProvisionedChallenge::Http { token } => {
                self.challenges.write().await.remove(&token);
                Ok(())
            }
            ProvisionedChallenge::Dns(record) => {
                dns_provider
                    .ok_or_else(|| anyhow::anyhow!("ACME DNS provider is unavailable"))?
                    .cleanup(&record)
                    .await
            }
        }
    }
}

fn certificate_not_after(path: &Path) -> Result<Option<i64>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    let (_, pem) = parse_x509_pem(&bytes).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let certificate = pem
        .parse_x509()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(Some(certificate.validity().not_after.timestamp()))
}

fn atomic_write(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = temporary_path(path);
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode);
    }
    let mut file = options
        .open(&temporary)
        .with_context(|| format!("failed to create {}", temporary.display()))?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path)
        .with_context(|| format!("failed to install {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("acme");
    path.with_file_name(format!(".{name}.{}.tmp", Uuid::new_v4()))
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_certificate_has_no_expiry() {
        let path = std::env::temp_dir().join(format!("mirrorproxy-acme-{}.pem", Uuid::new_v4()));
        assert_eq!(certificate_not_after(&path).unwrap(), None);
    }

    #[test]
    fn atomic_write_replaces_contents_and_applies_private_mode() {
        let directory = std::env::temp_dir().join(format!("mirrorproxy-acme-{}", Uuid::new_v4()));
        let path = directory.join("account.json");
        atomic_write(&path, b"one", 0o600).unwrap();
        atomic_write(&path, b"two", 0o600).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = fs::remove_dir_all(directory);
    }

    #[tokio::test]
    async fn stores_exact_http01_challenge_responses() {
        let (manager, _) = AcmeManager::new(AcmeConfig::default());
        manager
            .challenges
            .write()
            .await
            .insert("token".to_string(), "token.thumbprint".to_string());
        assert_eq!(
            manager.challenge_response("token").await.as_deref(),
            Some("token.thumbprint")
        );
        assert_eq!(manager.challenge_response("other").await, None);
    }
}
