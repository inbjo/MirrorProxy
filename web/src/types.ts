import { type SourceCapabilities, type SelectionReport } from './source-details'

export type Locale = 'en' | 'zh'

export type Theme = 'light' | 'dark'

export type AdminNotice = { tone: 'error' | 'success'; title: string; message: string }

export type SiteSettings = { title: string; description: string; keywords: string[]; icon_url: string; footer_text: string }

export type PublicConfig = {
  doh?: { enabled: boolean }
  public_base_url: string
  site?: SiteSettings
  enabled_proxies: string[]
  quota: {
    enabled: boolean
    bidirectional_accounting: boolean
    monthly_gb: number
    timezone: string
    on_exceeded: string
    month?: string
    status?: 'available' | 'exhausted' | 'reserved' | 'unavailable'
  }
  user_access?: { enabled: boolean; mode: string }
  registration?: {
    mode: 'invite_only' | 'domain_allowlist' | 'open' | 'disabled'
    allowed_email_domains: string[]
    email_login_enabled: boolean
  }
}

export type UpstreamClientEndpoint =
  | { kind: 'url' | 'repository' | 'image_prefix'; value: string }
  | { kind: 'docker_mirror' }
  | { kind: 'internal' }

export type AdminConfig = Omit<PublicConfig, 'quota' | 'user_access' | 'site'> & {
  built_in_os_repositories?: string[]
  upstream_client_endpoints?: Record<string, UpstreamClientEndpoint>
  doh: { enabled: boolean; upstreams: string[]; ecs_mode?: 'auto' | 'passthrough'; ecs_auto_upstreams?: string[]; requests_per_minute?: number }
  site: SiteSettings
  quota: PublicConfig['quota'] & { request_event_retention_days: number; default_user_monthly_gb: number | null }
  trusted_proxies: string[]
  forward_client_authorization: boolean
  database_path: string
  listen_addr: string
  management: { enabled: boolean; listen_addr: string }
  metrics: { local_only: boolean }
  upstreams: Record<string, string | Record<string, string>>
  timeout: { request_secs: number }
  upstream_selection: { strategy: 'ordered' | 'adaptive'; failure_threshold: number; cooldown_secs: number }
  alerts: { enabled: boolean; webhook_url: string; has_webhook_url: boolean; email_enabled: boolean; email_recipients: string[]; quota_percent: number; source_failures: number; cooldown_secs: number }
  outbound_proxy: {
    enabled: boolean
    url: string
    no_proxy: string[]
    username: string | null
    password: string | null
    has_password: boolean
  }
  upstream_tls: {
    ca_certificates: string[]
    insecure_skip_verify: boolean
  }
  rate_limit: { enabled: boolean; requests_per_minute: number }
  cache: { enabled: boolean; directory: string; max_entry_mb: number; max_total_mb: number; default_ttl_secs: number; max_ttl_secs: number }
  geoip: { enabled: boolean; ipv4_path: string; ipv6_path: string }
  user_access: {
    base_domain: string
    mode: 'public' | 'subdomain_required'
    infrastructure_ready: boolean
    routing_id_min_length: number
    routing_rotation_cooldown_hours: number
  }
  registration: {
    mode: 'invite_only' | 'domain_allowlist' | 'open' | 'disabled'
    allowed_email_domains: string[]
    email_token_ttl_minutes: number
  }
  webauthn: {
    enabled: boolean
    rp_id: string
    rp_origin: string
    rp_name: string
    require_passkey: boolean
    break_glass_username: string
  }
}

export type AdminStats = {
  month: string
  request_count: number
  response_bytes: number
  error_count: number
  quota: {
    enabled: boolean
    monthly_limit_bytes: number | null
    remaining_bytes: number | null
    exceeded: boolean
    timezone: string
    on_exceeded: string
  }
  daily: Array<{ day: string; target_code: string; request_count: number; response_bytes: number; error_count: number }>
  targets: Array<{ target_code: string; request_count: number; response_bytes: number; error_count: number }>
}

export type GeoDatabaseStatus = { ip_version: number; available: boolean; path: string; created_at: number | null; modified_at: number | null; size_bytes: number | null; sha256: string | null; error: string | null }

export type GeoIpStatus = { enabled: boolean; ipv4: GeoDatabaseStatus; ipv6: GeoDatabaseStatus }

export type GeoLocation = { country: string | null; province: string | null; city: string | null; isp: string | null; country_code: string | null }

export type IpAccessRule = { id: number; action: 'allow' | 'deny'; input_kind: 'ip' | 'cidr'; network: string; note: string; enabled: boolean; created_at: number; updated_at: number }

export type GeoTrafficOverview = { request_count: number; response_bytes: number; billed_bytes: number; error_count: number; daily: Array<{ day: string; request_count: number; response_bytes: number; billed_bytes: number; error_count: number }>; regions: Array<{ country_code: string; country: string; province: string; city: string; request_count: number; response_bytes: number; billed_bytes: number; error_count: number }> }

export type AuditLogEntry = {
  created_at: number
  username: string
  action: string
  detail: string
}

export type AdminIdentity = { username: string; role: string }

export type AdminPasskey = { id: number; name: string; created_at: number; last_used_at: number | null }

export type SourceCatalog = {
  providers: MirrorProvider[]
  targets: SourceTarget[]
  sources: TargetSource[]
  templates: SourceTemplate[]
  system_profiles?: Array<{ code: string; name: string; format: string; requires_customer_mirror: boolean }>
  container_registries?: ContainerRegistryTarget[]
}

export type ContainerRegistryTarget = {
  code: string
  name: string
  host: string
  aliases: string[]
  example_image: string
  legacy: boolean
}

export type MirrorProvider = {
  code: string
  name: string
  kind: string
  homepage: string
  speed_test_url: string | null
}

export type SourceTarget = {
  capabilities?: SourceCapabilities
  code: string
  name: string
  category: 'lang' | 'os' | 'repo'
  aliases: string[]
  supported_modes: string[]
  default_scope: string
  migration_target?: string
}

export type TargetSource = {
  target_code: string
  provider_code: string
  repo_url: string
  speed_url: string | null
  capability: string
}

export type SourceTemplate = {
  target_code: string
  os_family: string
  scope: string
  template: string
  requires_sudo: boolean
}

export type SourceEndpointHealthItem = {
  position: number
  endpoint: string
  status: 'healthy' | 'unhealthy'
  http_status: number | null
  latency_ms: number | null
  checked_at: number
  error: string | null
}

export type SourceHealthItem = {
  target_code: string
  adapter: string
  status: 'healthy' | 'degraded' | 'unhealthy' | 'disabled'
  http_status: number | null
  latency_ms: number | null
  checked_at: number
  error: string | null
  endpoints: SourceEndpointHealthItem[]
}

export type SourceHealthReport = {
  selection?: SelectionReport
  running: boolean
  total: number
  healthy: number
  degraded: number
  unhealthy: number
  disabled: number
  unknown: number
  last_checked_at: number | null
  items: SourceHealthItem[]
}

export type UserProfile = { user: { username?: string | null; id: number; email: string; display_name: string; routing_id: string; routing_rotated_at: number }; proxy_base_url: string | null }

export type QuotaUsage = { limit_bytes: number | null; used_bytes: number; remaining_bytes: number | null }

export type UserUsage = {
  month: string
  today_response_bytes: number
  request_count: number
  response_bytes: number
  error_count: number
  quota: QuotaUsage
  group: { id: number; name: string; quota: QuotaUsage } | null
  daily: Array<{ day: string; target_code: string; response_bytes: number; request_count: number; error_count: number }>
  targets: Array<{ target_code: string; response_bytes: number; request_count: number; error_count: number }>
}

export type PublicAuthProvider = { slug: string; display_name: string; kind: string }

export type LinkedIdentity = { id: number; provider_slug: string; provider_name: string; provider_subject: string; email: string | null; email_verified: boolean; created_at: number }

export type SmtpView = { enabled: boolean; host: string; port: number; security: string; username: string | null; has_password: boolean; from_name: string; from_address: string }

export type InvitationView = { id: number; email: string; display_name: string; status: string; expires_at: number }

export type OperationNotice = { tone: 'success' | 'error'; message: string }

export type AcmeStatusView = { enabled: boolean; challenge: string; dns_provider: string | null; domains: string[]; certificate_path: string; private_key_path: string; certificate_not_after: number | null; last_success_at: number | null; last_error: string | null; running: boolean; direct_https: boolean; http_listen_addr: string; https_listen_addr: string; https_active: boolean }

export type AcmeDnsConfigDraft = {
  provider: string; cloudflare_zone_id: string; cloudflare_api_token: string; cloudflare_api_key: string; cloudflare_email: string
  aliyun_domain: string; aliyun_access_key_id: string; aliyun_access_key_secret: string
  tencent_domain: string; tencent_secret_id: string; tencent_secret_key: string
  route53_hosted_zone_id: string; route53_access_key_id: string; route53_secret_access_key: string; route53_session_token: string
  webhook_url: string; webhook_bearer_token: string; propagation_delay_secs: number
  has_cloudflare_api_token: boolean; has_cloudflare_api_key: boolean; has_cloudflare_email: boolean
  has_aliyun_access_key_id: boolean; has_aliyun_access_key_secret: boolean
  has_tencent_secret_id: boolean; has_tencent_secret_key: boolean
  has_route53_access_key_id: boolean; has_route53_secret_access_key: boolean; has_route53_session_token: boolean
  has_webhook_bearer_token: boolean
}

export type AcmeConfigDraft = {
  enabled: boolean; email: string; domains: string[]; challenge: 'http-01' | 'dns-01'; directory_url: string
  storage_directory: string; renew_before_days: number; check_interval_hours: number
  direct_https: boolean; http_listen_addr: string; https_listen_addr: string; redirect_http_to_https: boolean; dns: AcmeDnsConfigDraft
}

export type AuthProviderView = {
  id: number; slug: string; display_name: string; kind: 'oauth2' | 'oidc'; preset: string; enabled: boolean; client_id: string; has_client_secret: boolean
  issuer_url: string | null; authorization_url: string | null; token_url: string | null; userinfo_url: string | null; emails_url: string | null
  scopes: string[]; subject_field: string; email_field: string; email_verified_field: string | null; display_name_field: string
}

export type AuthProviderTemplate = Omit<AuthProviderView, 'id' | 'slug' | 'enabled' | 'client_id' | 'has_client_secret' | 'subject_field' | 'email_field' | 'email_verified_field' | 'display_name_field'>

export type AuthProviderDraft = AuthProviderView & { client_secret: string }
