# Serein

An open-source native Discord desktop client in Rust and egui/eframe. **Experimental, unofficial, and not endorsed by Discord.** The normal-user live message exchange gate has not passed. This is not yet a completed Discord replacement.

```sh
cargo run --locked -- --demo       # synthetic, no network or user storage
cargo run --locked                 # saved login, or official Discord login webview; text-only
cargo run --locked --features voice # optional DM and server voice audio
cargo xtask check
node tests/login-handoff.cjs       # development-only JS bridge test
cargo replay
cargo xtask package                # text-only package; Linux .deb, macOS ad-hoc bundle
cargo xtask package-voice          # separate voice build under dist/voice
```

Rust 1.98.1 is pinned. See [platform build requirements](docs/platform-support.md) before building on Linux or Windows.

The open conversation shows incoming typing with short expiry and names already loaded by the
client. This is synthetic-tested only; service delivery is unverified, and guild signals may
depend on the existing People subscription. Serein does not send typing notifications.

The messaging interface is native egui/wgpu. A temporary platform webview displays Discord’s actual login page; after login, an origin-checked handoff accepts the session credential used by that webview’s own Discord requests and closes the webview. Passwords, QR exchange, and challenges are handled by Discord’s page. Acceptance of this embedded login and each authentication method remains **live-unverified**. Unsupported challenges are never bypassed.

The owner revised SPEC.md to **allow local storage**. Login uses the OS credential store. Recent history and drafts use an account-isolated, bounded local SQLite cache. Clear cached history removes cached messages and service images; logout removes the saved credential and that account’s cache/drafts. SQLite content is not encrypted by the application. Light/Dark appearance and the chosen theme preset (Default, Onyx, Ash or a gradient) persist across launches; System and Default remove their overrides. See [storage policy](docs/storage-policy.md).

Implemented: native navigation with collapsible server categories and cached server icons, native embed cards and [inline image attachments](docs/chat-images.md) with cached previews, a full-window image viewer and explicit original-file downloads, virtualized variable-height text rows, composition, bounded Markdown formatting with clickable inline links and plain URLs, safe link confirmation, inline text spoilers with separate media reveal, CJK/Arabic font fallbacks, synthetic send/edit/delete/reply, direct experimental REST/Gateway adapters, rate-limit cooldowns, heartbeat/reconnect/resume handling, partial patches, bounded message reconciliation, cached history and saved drafts, an on-demand People pane, on-demand [service profile cards](docs/profiles.md) with bios, banners and available account/server details, [clickable user mentions with composer suggestions](docs/mentions.md), native reaction counts with an eight-emoji picker and add/remove controls, unread channel/DM badges with explicit remote mark-read actions, conversation search with paged snippets and history navigation, and static profile pictures cached on disk with a small RAM texture working set. Guild member lists currently show the first 100 list positions, including group separators; DM participants come from the session snapshot. Live functionality is not proven by fixtures or local socket tests. Attachment uploads/animations, Discord Markdown parity, advanced/global search, video and screen sharing remain incomplete. Multilingual glyph coverage is tested; actual IME, bidirectional editing and screen-reader behavior remain unverified.

The optional `voice` feature implements existing one-to-one DM calls and server voice channels: DM Start/Answer/Decline/Hangup, server Join/Leave with participant rosters and elapsed connection time, bounded group playback, native device selection, mute/deafen, focused V push-to-talk, Opus audio, Discord voice WebSocket/UDP transport and DAVE version 1. Default builds remain text-only. Voice has synthetic protocol/codec tests, **no live Discord or physical microphone/speaker validation**. Use headphones: acoustic echo cancellation is not implemented. See [voice scope and live procedure](docs/voice.md).

Discord forbids automating normal accounts outside its OAuth2/bot API and warns of account termination. Interactive use and open source do not establish approval. Read the [compatibility matrix](docs/discord-compatibility.md) and [owner-controlled live procedure](docs/authentication.md). No bot substitution, backend, relay, credential extraction from other applications, CAPTCHA/MFA bypass, fingerprint spoofing, or telemetry.

See [categories](docs/categories.md), [embeds](docs/embeds.md), [chat images and downloads](docs/chat-images.md), [image caching](docs/icons.md), [progress and actual checks](docs/progress.md), [performance](docs/performance.md), [architecture](docs/architecture.md), and [dependency notices](THIRD_PARTY_NOTICES.md). Original code: MIT OR Apache-2.0.
