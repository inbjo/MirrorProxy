# Security Policy / 安全策略

## Supported versions / 支持版本

Security fixes are provided for the latest released version of MirrorProxy.
Older releases should be upgraded before a report is evaluated or a fix is
applied.

MirrorProxy 仅为最新正式版本提供安全修复。报告问题或应用修复前，请先升级已停止维护的旧版本。

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| Older releases | No |

## Reporting a vulnerability / 报告安全漏洞

Do not disclose suspected vulnerabilities in a public issue, discussion, or
pull request. Use GitHub's
[private vulnerability reporting](https://github.com/inbjo/MirrorProxy/security/advisories/new)
to send the maintainers a confidential report. If that form is unavailable,
email `2345@mail.com` with the subject `MirrorProxy security report`.

请勿在公开 Issue、Discussion 或 Pull Request 中披露疑似漏洞。请优先通过 GitHub
[私密漏洞报告](https://github.com/inbjo/MirrorProxy/security/advisories/new)联系维护者；若该入口不可用，
请发送邮件至 `2345@mail.com`，主题注明 `MirrorProxy security report`。

Include the affected version and configuration, impact, reproduction steps or
proof of concept, and any suggested mitigation. Remove credentials, access
tokens, private keys, personal data, and production logs that are not necessary
to reproduce the issue.

报告应包含受影响版本与配置、影响范围、复现步骤或 PoC，以及可选的缓解建议。请删除与复现无关的
凭据、访问令牌、私钥、个人信息和生产日志。

We aim to acknowledge a report within 7 days. We will coordinate validation,
remediation, release timing, and public disclosure with the reporter. Please do
not publish details until a fix is available or a disclosure date has been
agreed.

我们会尽量在 7 天内确认收到报告，并与报告者协调验证、修复、发布和公开披露时间。在修复版本可用
或双方约定披露日期之前，请勿公开漏洞细节。

## Scope / 范围

Reports about authentication or authorization bypasses, credential or secret
exposure, unsafe proxy routing, cache isolation, request smuggling, remote code
execution, and release or update integrity are especially useful. General bugs,
feature requests, and support questions belong in
[GitHub Issues](https://github.com/inbjo/MirrorProxy/issues).

我们尤其关注身份认证或授权绕过、凭据或秘密泄露、不安全的代理路由、缓存隔离、请求走私、远程代码
执行，以及发布或更新完整性问题。普通缺陷、功能建议和使用问题请提交到
[GitHub Issues](https://github.com/inbjo/MirrorProxy/issues)。
