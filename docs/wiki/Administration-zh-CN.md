# 后台管理

独立的 `/admin` 控制台使用 SQLite Cookie 会话，支持多管理员、`admin` 与
`super_admin` 角色、密码恢复、会话撤销和可选 WebAuthn Passkey。超级管理员修改服务
策略，普通管理员保留只读运维视图。

访问与配额设置包括注册策略、用户路由域名、全局/用户/计费组月度配额，以及可选的
双向计费。流量以实际写给客户端的字节为基础；请求明细遵循配置的保留期，日聚合继续
用于报表。

镜像检测、审计日志、Prometheus `/metrics`、结构化日志和可选 OTLP trace 提供运维
可观测性。后台 API 返回配置时会隐藏敏感值。

[English](Administration)
