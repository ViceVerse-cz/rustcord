# 0001 — Native client, temporary login webview, local cache

Accepted from the owner’s final instructions on 2026-09-09. The earlier independent-service specification is superseded. Serein directly accesses Discord’s existing service using normal-user sessions; no backend, bot replacement or separate voice service is built.

The owner subsequently replaced the no-webview login and no-storage constraints: the real Discord login belongs in a temporary platform webview; login tokens should survive restarts; local files/SQLite/caches/drafts/settings are allowed. Serein uses egui/wgpu for messaging, Wry only for authentication, OS credential storage for the token, and a bounded account-isolated SQLite content cache.

The initial temporary native-password-form implementation was removed after this clarification. OAuth was investigated for feasibility but is not the login route. No credential extraction from other applications or challenge bypass is authorized. Live interoperability remains an explicit owner-controlled gate.
