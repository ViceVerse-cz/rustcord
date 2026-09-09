# Serein
Read SPEC.md completely before changing scope. This is an unofficial native client for existing Discord accounts, never a backend or bot replacement. Preserve unrelated work.

Rust + egui/eframe; text-only by default. Local bounded SQLite caches, drafts, settings and diagnostics are allowed. Tokens must use the OS credential store; no plaintext token fallback. Official Discord login may use an ephemeral authentication-only webview; all messaging UI stays egui. No unredacted/unbounded logs, telemetry, credential extraction from other applications, challenge bypass, or unapproved external-account actions. Bound all caches, payloads and queues. Keep active secrets redacted in session memory; saved tokens belong only in the OS credential store.

Run `cargo xtask check`. Default tests are synthetic and offline. Live tests require the owner to explicitly enable the developer-session build and control the private conversation; never request credentials in chat. Update docs/progress.md with actual evidence and blockers. Never call an offline fixture a working Discord client.
