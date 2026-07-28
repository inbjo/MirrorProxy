# Administration

The independent `/admin` console uses SQLite-backed cookie sessions and
supports multiple administrators, `admin` and `super_admin` roles, password
recovery, session revocation, and optional WebAuthn passkeys. Super
administrators manage service policy; regular administrators retain read-only
operational visibility.

Access and quota settings cover registration policy, user routing domains,
global/user/billing-group monthly limits, and optional bidirectional billing.
Traffic is counted from bytes actually streamed to clients; request events use
the configured retention period while daily aggregates remain available for
reports.

Source health, audit logs, Prometheus `/metrics`, structured logs, and optional
OTLP traces provide operational visibility. Secrets remain redacted from admin
API responses.

[中文](Administration-zh-CN)
