use std::collections::BTreeMap;

use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use quick_xml::{events::Event, Reader};
use reqwest::{Client, Method, RequestBuilder};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::AcmeDnsConfig;

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug)]
pub struct DnsRecordHandle {
    pub name: String,
    pub value: String,
    pub provider_id: Option<String>,
}

#[async_trait]
pub trait DnsProvider: Send + Sync {
    async fn present(&self, name: &str, value: &str) -> Result<DnsRecordHandle>;
    async fn cleanup(&self, record: &DnsRecordHandle) -> Result<()>;
}

pub fn build_provider(config: &AcmeDnsConfig, client: Client) -> Result<Box<dyn DnsProvider>> {
    let provider: Box<dyn DnsProvider> = match config.provider.as_str() {
        "cloudflare" => Box::new(CloudflareProvider {
            config: config.clone(),
            client,
        }),
        "aliyun" => Box::new(AliyunProvider {
            config: config.clone(),
            client,
        }),
        "tencent" => Box::new(TencentProvider {
            config: config.clone(),
            client,
        }),
        "route53" => Box::new(Route53Provider {
            config: config.clone(),
            client,
        }),
        "webhook" => Box::new(WebhookProvider {
            config: config.clone(),
            client,
        }),
        value => anyhow::bail!("unsupported ACME DNS provider: {value}"),
    };
    Ok(provider)
}

struct CloudflareProvider {
    config: AcmeDnsConfig,
    client: Client,
}

#[derive(Deserialize)]
struct CloudflareRecord {
    id: String,
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct CloudflareResponse<T> {
    success: bool,
    result: Option<T>,
    #[serde(default)]
    errors: Vec<CloudflareError>,
}

#[derive(Deserialize)]
struct CloudflareError {
    #[serde(default)]
    code: u64,
    #[serde(default)]
    message: String,
}

#[async_trait]
impl DnsProvider for CloudflareProvider {
    async fn present(&self, name: &str, value: &str) -> Result<DnsRecordHandle> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
            self.config.cloudflare_zone_id
        );
        let response = cloudflare_auth(&self.config, self.client.post(url))
            .json(&serde_json::json!({
                "type": "TXT",
                "name": name,
                "content": value,
                "ttl": 120,
            }))
            .send()
            .await
            .context("Cloudflare DNS record creation failed")?;
        let status = response.status();
        let body: CloudflareResponse<CloudflareRecord> = response
            .json()
            .await
            .context("Cloudflare returned an invalid response")?;
        let record_id = if status.is_success() && body.success {
            format!(
                "created:{}",
                body.result
                    .ok_or_else(|| anyhow::anyhow!("Cloudflare omitted the DNS record ID"))?
                    .id
            )
        } else if body.errors.iter().any(|error| {
            error.code == 81058
                || error
                    .message
                    .to_ascii_lowercase()
                    .contains("already exists")
        }) {
            format!(
                "existing:{}",
                self.find_txt_record(name, value)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!(
                        "Cloudflare reported an existing TXT record but did not return it"
                    ))?
            )
        } else {
            let message = body
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::bail!("Cloudflare rejected the DNS record: {message}");
        };
        Ok(DnsRecordHandle {
            name: name.to_string(),
            value: value.to_string(),
            provider_id: Some(record_id),
        })
    }

    async fn cleanup(&self, record: &DnsRecordHandle) -> Result<()> {
        let id = record
            .provider_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Cloudflare DNS record ID is missing"))?;
        let Some(id) = id.strip_prefix("created:") else {
            return Ok(());
        };
        let response = cloudflare_auth(
            &self.config,
            self.client.delete(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{id}",
                self.config.cloudflare_zone_id
            )),
        )
        .send()
        .await?;
        if !response.status().is_success() {
            anyhow::bail!("Cloudflare DNS cleanup returned {}", response.status());
        }
        Ok(())
    }
}

impl CloudflareProvider {
    async fn find_txt_record(&self, name: &str, value: &str) -> Result<Option<String>> {
        let response = cloudflare_auth(
            &self.config,
            self.client.get(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
                self.config.cloudflare_zone_id
            )),
        )
        .query(&[("type", "TXT"), ("name", name)])
        .send()
        .await
        .context("Cloudflare DNS record lookup failed")?;
        let status = response.status();
        let body: CloudflareResponse<Vec<CloudflareRecord>> = response
            .json()
            .await
            .context("Cloudflare returned an invalid record lookup response")?;
        if !status.is_success() || !body.success {
            anyhow::bail!("Cloudflare DNS record lookup returned {status}");
        }
        Ok(body
            .result
            .unwrap_or_default()
            .into_iter()
            .find(|record| record.content == value)
            .map(|record| record.id))
    }
}

fn cloudflare_auth(config: &AcmeDnsConfig, request: RequestBuilder) -> RequestBuilder {
    if !config.cloudflare_api_token.is_empty() {
        request.bearer_auth(&config.cloudflare_api_token)
    } else {
        request
            .header("X-Auth-Email", &config.cloudflare_email)
            .header("X-Auth-Key", &config.cloudflare_api_key)
    }
}

struct AliyunProvider {
    config: AcmeDnsConfig,
    client: Client,
}

#[async_trait]
impl DnsProvider for AliyunProvider {
    async fn present(&self, name: &str, value: &str) -> Result<DnsRecordHandle> {
        let rr = relative_record_name(name, &self.config.aliyun_domain)?;
        let body = self
            .request(
                "AddDomainRecord",
                &[
                    ("DomainName", self.config.aliyun_domain.as_str()),
                    ("RR", rr.as_str()),
                    ("Type", "TXT"),
                    ("Value", value),
                    ("TTL", "600"),
                ],
            )
            .await?;
        let id = body
            .get("RecordId")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Alibaba Cloud DNS omitted RecordId"))?;
        Ok(DnsRecordHandle {
            name: name.to_string(),
            value: value.to_string(),
            provider_id: Some(id.to_string()),
        })
    }

    async fn cleanup(&self, record: &DnsRecordHandle) -> Result<()> {
        let id = record
            .provider_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Alibaba Cloud DNS record ID is missing"))?;
        self.request("DeleteDomainRecord", &[("RecordId", id)])
            .await?;
        Ok(())
    }
}

impl AliyunProvider {
    async fn request(&self, action: &str, values: &[(&str, &str)]) -> Result<serde_json::Value> {
        let mut parameters = BTreeMap::from([
            (
                "AccessKeyId".to_string(),
                self.config.aliyun_access_key_id.clone(),
            ),
            ("Action".to_string(), action.to_string()),
            ("Format".to_string(), "JSON".to_string()),
            ("SignatureMethod".to_string(), "HMAC-SHA1".to_string()),
            ("SignatureNonce".to_string(), Uuid::new_v4().to_string()),
            ("SignatureVersion".to_string(), "1.0".to_string()),
            (
                "Timestamp".to_string(),
                Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            ),
            ("Version".to_string(), "2015-01-09".to_string()),
        ]);
        for (key, value) in values {
            parameters.insert((*key).to_string(), (*value).to_string());
        }
        parameters.insert(
            "Signature".to_string(),
            aliyun_signature(&parameters, &self.config.aliyun_access_key_secret)?,
        );
        let response = self
            .client
            .get(format!(
                "https://alidns.aliyuncs.com/?{}",
                canonical_query(&parameters)
            ))
            .send()
            .await
            .context("Alibaba Cloud DNS request failed")?;
        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Alibaba Cloud DNS returned invalid JSON")?;
        if !status.is_success() || body.get("Code").is_some() {
            let message = body
                .get("Message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown API error");
            anyhow::bail!("Alibaba Cloud DNS rejected {action}: {message}");
        }
        Ok(body)
    }
}

struct TencentProvider {
    config: AcmeDnsConfig,
    client: Client,
}

#[async_trait]
impl DnsProvider for TencentProvider {
    async fn present(&self, name: &str, value: &str) -> Result<DnsRecordHandle> {
        let subdomain = relative_record_name(name, &self.config.tencent_domain)?;
        let body = self
            .request(
                "CreateRecord",
                serde_json::json!({
                    "Domain": self.config.tencent_domain,
                    "SubDomain": subdomain,
                    "RecordType": "TXT",
                    "RecordLine": "默认",
                    "Value": value,
                    "TTL": 600,
                }),
            )
            .await?;
        let id = body
            .pointer("/Response/RecordId")
            .and_then(|value| {
                value
                    .as_u64()
                    .map(|id| id.to_string())
                    .or_else(|| value.as_str().map(ToString::to_string))
            })
            .ok_or_else(|| anyhow::anyhow!("Tencent DNSPod omitted RecordId"))?;
        Ok(DnsRecordHandle {
            name: name.to_string(),
            value: value.to_string(),
            provider_id: Some(id),
        })
    }

    async fn cleanup(&self, record: &DnsRecordHandle) -> Result<()> {
        let id = record
            .provider_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Tencent DNSPod record ID is missing"))?
            .parse::<u64>()?;
        self.request(
            "DeleteRecord",
            serde_json::json!({
                "Domain": self.config.tencent_domain,
                "RecordId": id,
            }),
        )
        .await?;
        Ok(())
    }
}

impl TencentProvider {
    async fn request(&self, action: &str, payload: serde_json::Value) -> Result<serde_json::Value> {
        let payload = serde_json::to_string(&payload)?;
        let timestamp = Utc::now().timestamp();
        let authorization = tencent_authorization(
            &self.config.tencent_secret_id,
            &self.config.tencent_secret_key,
            action,
            &payload,
            timestamp,
        )?;
        let response = self
            .client
            .post("https://dnspod.tencentcloudapi.com/")
            .header("Authorization", authorization)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Host", "dnspod.tencentcloudapi.com")
            .header("X-TC-Action", action)
            .header("X-TC-Timestamp", timestamp.to_string())
            .header("X-TC-Version", "2021-03-23")
            .body(payload)
            .send()
            .await
            .context("Tencent DNSPod request failed")?;
        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Tencent DNSPod returned invalid JSON")?;
        if !status.is_success() || body.pointer("/Response/Error").is_some() {
            let message = body
                .pointer("/Response/Error/Message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown API error");
            anyhow::bail!("Tencent DNSPod rejected {action}: {message}");
        }
        Ok(body)
    }
}

fn tencent_authorization(
    secret_id: &str,
    secret_key: &str,
    _action: &str,
    payload: &str,
    timestamp: i64,
) -> Result<String> {
    let date = DateTime::from_timestamp(timestamp, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid Tencent request timestamp"))?
        .format("%Y-%m-%d")
        .to_string();
    let canonical_headers =
        "content-type:application/json; charset=utf-8\nhost:dnspod.tencentcloudapi.com\n";
    let signed_headers = "content-type;host";
    let canonical_request = format!(
        "POST\n/\n\n{canonical_headers}\n{signed_headers}\n{}",
        sha256_hex(payload.as_bytes())
    );
    let scope = format!("{date}/dnspod/tc3_request");
    let string_to_sign = format!(
        "TC3-HMAC-SHA256\n{timestamp}\n{scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let secret_date = hmac_sha256(format!("TC3{secret_key}").as_bytes(), date.as_bytes())?;
    let secret_service = hmac_sha256(&secret_date, b"dnspod")?;
    let secret_signing = hmac_sha256(&secret_service, b"tc3_request")?;
    let signature = hex::encode(hmac_sha256(&secret_signing, string_to_sign.as_bytes())?);
    Ok(format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
    ))
}

struct Route53Provider {
    config: AcmeDnsConfig,
    client: Client,
}

#[derive(Default)]
struct Route53RecordSet {
    ttl: u32,
    values: Vec<String>,
}

#[async_trait]
impl DnsProvider for Route53Provider {
    async fn present(&self, name: &str, value: &str) -> Result<DnsRecordHandle> {
        let mut record_set = self
            .get_txt_record_set(name)
            .await?
            .unwrap_or(Route53RecordSet {
                ttl: 60,
                values: Vec::new(),
            });
        let created = !record_set.values.iter().any(|current| current == value);
        if created {
            record_set.values.push(value.to_string());
            self.change_record_set("UPSERT", name, &record_set).await?;
        }
        Ok(DnsRecordHandle {
            name: name.to_string(),
            value: value.to_string(),
            provider_id: Some(if created { "created" } else { "existing" }.to_string()),
        })
    }

    async fn cleanup(&self, record: &DnsRecordHandle) -> Result<()> {
        if record.provider_id.as_deref() != Some("created") {
            return Ok(());
        }
        let Some(mut current) = self.get_txt_record_set(&record.name).await? else {
            return Ok(());
        };
        if !current.values.iter().any(|value| value == &record.value) {
            return Ok(());
        }
        if current.values.len() == 1 {
            self.change_record_set("DELETE", &record.name, &current)
                .await?;
        } else {
            current.values.retain(|value| value != &record.value);
            self.change_record_set("UPSERT", &record.name, &current)
                .await?;
        }
        Ok(())
    }
}

impl Route53Provider {
    fn hosted_zone_id(&self) -> &str {
        self.config
            .route53_hosted_zone_id
            .trim()
            .trim_start_matches("/hostedzone/")
    }

    async fn get_txt_record_set(&self, name: &str) -> Result<Option<Route53RecordSet>> {
        let query = format!(
            "maxitems=1&name={}&type=TXT",
            percent_encode(&ensure_trailing_dot(name))
        );
        let response = self
            .signed_request(
                Method::GET,
                &format!("/2013-04-01/hostedzone/{}/rrset", self.hosted_zone_id()),
                &query,
                "",
                "application/xml",
            )
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            anyhow::bail!(
                "AWS Route53 list records returned {status}: {}",
                bounded(&body)
            );
        }
        parse_route53_record_set(&body, name)
    }

    async fn change_record_set(
        &self,
        action: &str,
        name: &str,
        record_set: &Route53RecordSet,
    ) -> Result<()> {
        let values = record_set
            .values
            .iter()
            .map(|value| {
                format!(
                    "<ResourceRecord><Value>&quot;{}&quot;</Value></ResourceRecord>",
                    xml_escape(value)
                )
            })
            .collect::<String>();
        let body = format!(
            "<ChangeResourceRecordSetsRequest xmlns=\"https://route53.amazonaws.com/doc/2013-04-01/\"><ChangeBatch><Changes><Change><Action>{action}</Action><ResourceRecordSet><Name>{}</Name><Type>TXT</Type><TTL>{}</TTL><ResourceRecords>{values}</ResourceRecords></ResourceRecordSet></Change></Changes></ChangeBatch></ChangeResourceRecordSetsRequest>",
            xml_escape(&ensure_trailing_dot(name)),
            record_set.ttl
        );
        let response = self
            .signed_request(
                Method::POST,
                &format!("/2013-04-01/hostedzone/{}/rrset", self.hosted_zone_id()),
                "",
                &body,
                "application/xml",
            )
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("AWS Route53 {action} returned {status}: {}", bounded(&body));
        }
        Ok(())
    }

    async fn signed_request(
        &self,
        method: Method,
        path: &str,
        query: &str,
        body: &str,
        content_type: &str,
    ) -> Result<reqwest::Response> {
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let short_date = now.format("%Y%m%d").to_string();
        let mut headers = BTreeMap::from([
            ("content-type", content_type.to_string()),
            ("host", "route53.amazonaws.com".to_string()),
            ("x-amz-date", amz_date.clone()),
        ]);
        if !self.config.route53_session_token.is_empty() {
            headers.insert(
                "x-amz-security-token",
                self.config.route53_session_token.clone(),
            );
        }
        let signed_headers = headers.keys().copied().collect::<Vec<_>>().join(";");
        let canonical_headers = headers
            .iter()
            .map(|(key, value)| format!("{key}:{}\n", value.trim()))
            .collect::<String>();
        let canonical_request = format!(
            "{}\n{path}\n{query}\n{canonical_headers}\n{signed_headers}\n{}",
            method.as_str(),
            sha256_hex(body.as_bytes())
        );
        let scope = format!("{short_date}/us-east-1/route53/aws4_request");
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );
        let date_key = hmac_sha256(
            format!("AWS4{}", self.config.route53_secret_access_key).as_bytes(),
            short_date.as_bytes(),
        )?;
        let region_key = hmac_sha256(&date_key, b"us-east-1")?;
        let service_key = hmac_sha256(&region_key, b"route53")?;
        let signing_key = hmac_sha256(&service_key, b"aws4_request")?;
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes())?);
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.config.route53_access_key_id
        );
        let url = if query.is_empty() {
            format!("https://route53.amazonaws.com{path}")
        } else {
            format!("https://route53.amazonaws.com{path}?{query}")
        };
        let mut request = self
            .client
            .request(method, url)
            .header("Authorization", authorization)
            .header("Content-Type", content_type)
            .header("X-Amz-Date", amz_date);
        if !self.config.route53_session_token.is_empty() {
            request = request.header("X-Amz-Security-Token", &self.config.route53_session_token);
        }
        request
            .body(body.to_string())
            .send()
            .await
            .map_err(Into::into)
    }
}

#[derive(Serialize)]
struct DnsWebhookRequest<'a> {
    action: &'a str,
    record_type: &'static str,
    record_name: &'a str,
    value: &'a str,
    ttl: u32,
}

struct WebhookProvider {
    config: AcmeDnsConfig,
    client: Client,
}

#[async_trait]
impl DnsProvider for WebhookProvider {
    async fn present(&self, name: &str, value: &str) -> Result<DnsRecordHandle> {
        self.call("present", name, value).await?;
        Ok(DnsRecordHandle {
            name: name.to_string(),
            value: value.to_string(),
            provider_id: None,
        })
    }

    async fn cleanup(&self, record: &DnsRecordHandle) -> Result<()> {
        self.call("cleanup", &record.name, &record.value).await
    }
}

impl WebhookProvider {
    async fn call(&self, action: &str, name: &str, value: &str) -> Result<()> {
        let mut request = self
            .client
            .post(&self.config.webhook_url)
            .json(&DnsWebhookRequest {
                action,
                record_type: "TXT",
                record_name: name,
                value,
                ttl: 120,
            });
        if !self.config.webhook_bearer_token.is_empty() {
            request = request.bearer_auth(&self.config.webhook_bearer_token);
        }
        let response = request.send().await.context("ACME DNS webhook failed")?;
        if !response.status().is_success() {
            anyhow::bail!("ACME DNS webhook returned {}", response.status());
        }
        Ok(())
    }
}

fn relative_record_name(name: &str, zone: &str) -> Result<String> {
    let name = name.trim_end_matches('.');
    let zone = zone.trim_end_matches('.');
    if name == zone {
        return Ok("@".to_string());
    }
    name.strip_suffix(&format!(".{zone}"))
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow::anyhow!("DNS record {name} is outside configured zone {zone}"))
}

fn canonical_query(parameters: &BTreeMap<String, String>) -> String {
    parameters
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn aliyun_signature(parameters: &BTreeMap<String, String>, secret: &str) -> Result<String> {
    let query = canonical_query(parameters);
    let string_to_sign = format!("GET&%2F&{}", percent_encode(&query));
    let mut mac = HmacSha1::new_from_slice(format!("{secret}&").as_bytes())?;
    mac.update(string_to_sign.as_bytes());
    Ok(BASE64_STANDARD.encode(mac.finalize().into_bytes()))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn hmac_sha256(key: &[u8], value: &[u8]) -> Result<Vec<u8>> {
    let mut mac = HmacSha256::new_from_slice(key)?;
    mac.update(value);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn sha256_hex(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}

fn ensure_trailing_dot(value: &str) -> String {
    format!("{}.", value.trim_end_matches('.'))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn parse_route53_record_set(body: &str, expected_name: &str) -> Result<Option<Route53RecordSet>> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    let mut in_record_set = false;
    let mut in_resource_record = false;
    let mut field = Vec::new();
    let mut name = String::new();
    let mut record_type = String::new();
    let mut ttl = 0;
    let mut values = Vec::new();
    loop {
        match reader.read_event()? {
            Event::Start(event) => {
                field = event.name().as_ref().to_vec();
                if field == b"ResourceRecordSet" {
                    in_record_set = true;
                    name.clear();
                    record_type.clear();
                    ttl = 0;
                    values.clear();
                } else if field == b"ResourceRecord" && in_record_set {
                    in_resource_record = true;
                }
            }
            Event::Text(text) if in_record_set => {
                let value = text.decode()?.into_owned();
                match field.as_slice() {
                    b"Name" => name = value,
                    b"Type" => record_type = value,
                    b"TTL" => ttl = value.parse().unwrap_or(0),
                    b"Value" if in_resource_record => {
                        values.push(value.trim_matches('"').to_string())
                    }
                    _ => {}
                }
            }
            Event::End(event) => {
                if event.name().as_ref() == b"ResourceRecord" {
                    in_resource_record = false;
                } else if event.name().as_ref() == b"ResourceRecordSet" {
                    in_record_set = false;
                    if record_type == "TXT"
                        && name.trim_end_matches('.') == expected_name.trim_end_matches('.')
                    {
                        return Ok(Some(Route53RecordSet { ttl, values }));
                    }
                }
                field.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(None)
}

fn bounded(value: &str) -> &str {
    value.get(..512).unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn extracts_relative_record_names_without_guessing_public_suffixes() {
        assert_eq!(
            relative_record_name("_acme-challenge.www.example.co.uk", "example.co.uk").unwrap(),
            "_acme-challenge.www"
        );
        assert!(relative_record_name("_acme-challenge.other.net", "example.com").is_err());
    }

    #[test]
    fn aliyun_percent_encoding_matches_rpc_rules() {
        assert_eq!(percent_encode("a b+c/~"), "a%20b%2Bc%2F~");
    }

    #[test]
    fn aliyun_signature_matches_official_rpc_example() {
        let parameters = BTreeMap::from([
            ("AccessKeyId".to_string(), "testid".to_string()),
            ("Action".to_string(), "DescribeDedicatedHosts".to_string()),
            ("Format".to_string(), "JSON".to_string()),
            ("RegionId".to_string(), "cn-beijing".to_string()),
            ("SignatureMethod".to_string(), "HMAC-SHA1".to_string()),
            (
                "SignatureNonce".to_string(),
                "edb2b34af0af9a6d14deaf7c1a5315eb".to_string(),
            ),
            ("SignatureVersion".to_string(), "1.0".to_string()),
            ("Timestamp".to_string(), "2023-03-13T08:34:30Z".to_string()),
            ("Version".to_string(), "2014-05-26".to_string()),
        ]);
        assert_eq!(
            aliyun_signature(&parameters, "testsecret").unwrap(),
            "9NaGiOspFP5UPcwX8Iwt2YJXXuk="
        );
    }

    #[test]
    fn parses_route53_txt_record_sets_with_multiple_values() {
        let xml = r#"<ListResourceRecordSetsResponse><ResourceRecordSets><ResourceRecordSet><Name>_acme-challenge.example.com.</Name><Type>TXT</Type><TTL>60</TTL><ResourceRecords><ResourceRecord><Value>"one"</Value></ResourceRecord><ResourceRecord><Value>"two"</Value></ResourceRecord></ResourceRecords></ResourceRecordSet></ResourceRecordSets></ListResourceRecordSetsResponse>"#;
        let record = parse_route53_record_set(xml, "_acme-challenge.example.com")
            .unwrap()
            .unwrap();
        assert_eq!(record.ttl, 60);
        assert_eq!(record.values, ["one", "two"]);
    }

    #[test]
    fn tencent_signature_is_stable() {
        let signature = tencent_authorization(
            "AKIDEXAMPLE",
            "secret",
            "CreateRecord",
            r#"{"Domain":"example.com"}"#,
            1_700_000_000,
        )
        .unwrap();
        assert!(signature
            .starts_with("TC3-HMAC-SHA256 Credential=AKIDEXAMPLE/2023-11-14/dnspod/tc3_request"));
        assert!(signature.contains("SignedHeaders=content-type;host"));
    }

    #[tokio::test]
    async fn webhook_provider_presents_and_cleans_up_records() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buffer = vec![0_u8; 8192];
                let read = stream.read(&mut buffer).await.unwrap();
                requests.push(String::from_utf8_lossy(&buffer[..read]).to_string());
                stream
                    .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                    .await
                    .unwrap();
            }
            requests
        });
        let config = AcmeDnsConfig {
            provider: "webhook".to_string(),
            webhook_url: format!("http://{address}"),
            webhook_bearer_token: "webhook-secret".to_string(),
            ..AcmeDnsConfig::default()
        };
        let provider = build_provider(&config, Client::new()).unwrap();
        let record = provider
            .present("_acme-challenge.example.com", "challenge-value")
            .await
            .unwrap();
        provider.cleanup(&record).await.unwrap();
        let requests = server.await.unwrap();
        assert!(requests[0].contains("authorization: Bearer webhook-secret"));
        assert!(requests[0].contains("\"action\":\"present\""));
        assert!(requests[1].contains("\"action\":\"cleanup\""));
    }
}
