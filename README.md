# Serein

<p align="center">
  <a href="https://github.com/ViceVerse-cz/rustcord">
    <img src="docs/preview.png" alt="Serein Native Discord Client" width="900" style="max-width: 100%; height: auto; border-radius: 8px; box-shadow: 0 4px 20px rgba(0,0,0,0.3);" />
  </a>
</p>

<p align="center">
  <strong>A lightweight, native Discord desktop client written in Rust, powered by egui and wgpu.</strong>
</p>

<p align="center">
  <a href="https://github.com/ViceVerse-cz/rustcord/releases"><img src="https://img.shields.io/github/v/release/ViceVerse-cz/rustcord?label=release&color=blue" alt="GitHub Release" /></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/rust-1.98.1_pinned-blue.svg?logo=rust" alt="Rust 1.98.1 Pinned" /></a>
  <a href="crates/ui"><img src="https://img.shields.io/badge/ui-egui%20%2F%20wgpu-orange.svg" alt="UI egui/wgpu" /></a>
  <a href="docs/platform-support.md"><img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-informational.svg" alt="Platform Support" /></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green.svg" alt="License: MIT or Apache-2.0" /></a>
</p>

---

> [!WARNING]
> **Experimental, unofficial, and not endorsed by Discord.**
> Serein communicates directly with Discord's public gateway and REST endpoints for your existing account. It is **not** a completed Discord replacement, and the normal-user live message exchange gate has not passed. Automating normal accounts outside the official OAuth2/bot API violates Discord's Terms of Service and carries risk of account termination. Technical interoperability does not imply platform approval. Review the [compatibility matrix](docs/discord-compatibility.md) and [authentication guide](docs/authentication.md) before use.

---

## Downloads & Installation

### Homebrew (macOS) *(Placeholder)*

```sh
# Coming soon: install via Homebrew Cask
brew tap ViceVerse-cz/serein
brew install --cask serein
```

### Pre-built Releases

Pre-compiled release packages are published on the GitHub [Releases](https://github.com/ViceVerse-cz/rustcord/releases) page:

| Platform | Format | Architectures | Details |
|---|---|---|---|
| **macOS** | `.zip` archive | Apple Silicon (`aarch64`), Intel (`x86_64`) | Signed and notarized `.app` bundle |
| **Linux** | `.deb` package / binary | `x86_64` | Ubuntu/Debian native package |
| **Windows** | `.zip` archive | `x86_64` | Standalone executable package |

---

## Highlights

- **Pure Native Performance:** Built with pure Rust and `egui`/`wgpu`. Immediate-mode rendering with minimal idle CPU, low memory footprint, and instantaneous launch times—zero Electron, Node.js, or web messaging runtime.
- **Direct Gateway & REST Transports:** Connects directly to Discord's official endpoints with active rate-limiting cooldowns, heartbeat handling, reconnect/resume loops, and partial payload patching.
- **Secure OS Credential Storage:** Session tokens are stored exclusively in your operating system's secure vault (macOS Keychain, Windows Credential Manager, or Linux Secret Service). Never saved in plaintext.
- **Ephemeral Authentication Webview:** Sign-in uses Discord's official hosted login page inside a temporary native webview (WKWebView, WebView2, or WebKitGTK) supporting email/password, QR login, and MFA. An origin-checked handoff secures the session credential and immediately terminates the webview.
- **Bounded Local Persistence:** Recent chat history, drafts, image previews, settings, and diagnostics are stored in an account-isolated, bounded local SQLite database. All local data is strictly cleared upon explicit logout.
- **Native Voice & Echo Cancellation:** Built-in voice engine supporting Opus audio, Discord Voice WebSocket/UDP, DAVE v1 end-to-end encryption, 1-to-1 DM calls, server voice channels, push-to-talk, Sonora AEC3 acoustic echo cancellation, and RNNoise deep-learning noise suppression.
- **Native Screen Sharing:** Send window or display streams on macOS 14+ (ScreenCaptureKit) and Windows (Windows Graphics Capture) with source selection and quality presets up to 1080p60.
- **Rich Media & Video Playback:** Inline video playback for MOV and MP4 attachments; inline voice message player with interactive waveforms; multi-attachment uploads with thumbnail previews and progress tracking; related embed image galleries; and full-resolution image viewer modals.
- **Local Game IPC & Rich Presence:** Built-in Discord IPC server detecting local games, showing live game activities in member rosters, DM lists, and user profiles, with opt-in system tray integration.
- **Native Profile Customization:** In-app profile editor for global display names, bios / about me, pronouns, and custom accent colors via secure normal-user routes.
- **Discord-Fidelity UI & Context Actions:** Phosphor icon atlas, styled system event rows (welcome, boosts, pins, channel edits) with clickable member names, server dropdown with friend invites, group chat management, right-click message context menus, quick edit (`Up`), and quick delete (`Backspace`).

---

## Feature Showcase

| Chat Timeline & Styled System Events | Voice Call Stage & Pill Controls |
| :---: | :---: |
| <img src="docs/pr-evidence/system-message-design/after.png" alt="Chat Timeline & System Events" width="450" /> | <img src="docs/pr-evidence/voice-call-ui/after-call.png" alt="Voice Call UI" width="450" /> |
| **Multi-Attachment Batch Uploading** | **Native Profile Customization** |
| <img src="docs/pr-evidence/multi-attachment-sending/after.png" alt="Multi-Attachment Sending" width="450" /> | <img src="docs/pr-evidence/profile-edit/after.png" alt="Profile Editing" width="450" /> |
| **Native Screen Sharing Picker** | **Rich Presence & Game Activities** |
| <img src="docs/pr-evidence/screen-sharing/after.png" alt="Screen Sharing" width="450" /> | <img src="docs/pr-evidence/presence-and-tray/after.png" alt="Rich Presence and Tray" width="450" /> |

---

## Quick Start

### Prerequisites
Rust **1.98.1** is pinned. Ensure you have the standard C/C++ toolchain and CMake installed for your platform:
- **macOS:** Xcode command-line tools (`xcode-select --install`)
- **Linux:** GCC/Clang, ALSA development headers, `pkg-config`, GTK 4, WebKitGTK 6.0, fontconfig, and Vulkan drivers (see [Platform Support](docs/platform-support.md))
- **Windows:** Visual Studio C++ build tools and WebView2 Runtime

### Running Locally

```sh
# 1. Opt in to the offline synthetic demo (no network, no storage)
cargo run --locked --features demo -- --demo

# 2. Launch standard client with voice (uses saved login or official webview)
cargo run --locked
```

### Workspace Commands

```sh
# Run full workspace validation (formatting, Clippy, tests, policy checks)
cargo xtask check

# Run release reducer benchmark
cargo replay

# Run authentication bridge JS test harness
node tests/login-handoff.cjs

# Package release including voice (macOS .app bundle, Linux .deb)
cargo xtask package
```

---

## Feature Matrix

| Capability | Status | Notes |
|---|---|---|
| **Navigation & Guilds** | Implemented | Collapsible server categories, cached icons, guild channels, DM lists, People pane, and server dropdown menu |
| **Message Timeline** | Implemented | Virtualized variable-height rows, inline link confirmations, spoiler text/media reveal, deleted message hiding, and local timezone timestamps |
| **Markdown & System Messages** | Implemented | Bold, italics, code blocks, blockquotes, clickable links, and styled system events with tinted Phosphor icons and clickable names |
| **Reactions & Emojis** | Implemented | Twemoji rendering, native reaction counts, eight-emoji quick picker, full emoji picker integration, and add/remove reaction controls |
| **User Mentions & Autocomplete** | Implemented | Clickable user mentions with interactive composer autocompletion and visual highlight styling |
| **Media Previews & Video Player** | Implemented | Inline MOV and MP4 video playback, inline image cards, embed cards, related embed image galleries, full-resolution image viewer with aspect ratios, and explicit file downloads |
| **File & Attachment Uploads** | Implemented | Multi-attachment batch sending with composer thumbnail previews, upload progress bar, cancel action, and drag-and-drop / file picker |
| **Voice Engine & Calls** | Built in | 1-to-1 DM calls & server channels, Opus codec, DAVE v1, Sonora AEC3 echo cancellation, RNNoise suppression, push-to-talk (`V`), device selector |
| **Voice Messages** | Implemented | Inline voice message playback with interactive waveforms and bounded streaming audio buffering |
| **Screen Sharing** | Implemented (sender) | macOS 14+ ScreenCaptureKit and Windows Graphics Capture; source & quality selector (720p/1080p, 15/30/60 fps) |
| **Rich Presence & Game IPC** | Implemented | Discord IPC socket server detecting active games; displays activities in member lists, DMs, and profiles; opt-in system tray |
| **Profile Cards & Editing** | Implemented | On-demand profile popouts with banners, bios, badges, connections; native in-app editor for display name, bio, pronouns, and accent color |
| **Server & Group Actions** | Implemented | Server dropdown with friend invites and leave server; group DM actions (edit name/icon preview, mute, leave) |
| **Context Menus & Shortcuts** | Implemented | Message right-click context menu (reply, edit, delete, pin, copy link, react); quick edit (`Up`) and quick delete (`Backspace`) |
| **Typing Indicators** | Partial | Displays incoming typing with short expiry; Serein **never** emits outgoing typing signals |
| **Persistence & Drafts** | Implemented | Bounded SQLite cache for history, drafts, settings, and diagnostics; OS credential store for auth tokens; sanitary logout |
| **Internationalization** | Partial | Bundled Inter font, CJK and Arabic font fallbacks included; full IME and bidirectional editing unverified |
| **Camera Video / Stream Viewing** | Unimplemented | Planned for future milestones |

> [!NOTE]
> **Voice Scope Notice:** Voice includes Sonora AEC3 echo cancellation and RNNoise noise suppression. However, live two-way voice audio with an official Discord client remains an owner-operated validation gate, as default automated tests run against synthetic protocols and codecs.

---

## Architecture Overview

Serein is engineered as a clean multi-crate Cargo workspace, isolating UI rendering from networking, persistence, and service protocols:

```
rustcord/
├── apps/
│   └── desktop/          # Application entrypoint, CLI flags, window lifecycle
├── crates/
│   ├── client-core/      # Client state coordinator, generation tracking, events
│   ├── session-cache/    # In-memory bounded cache and state reconciliation
│   ├── ui/               # egui widgets, message virtualizer, themes, design tokens
│   ├── model/            # Strongly-typed Discord domain entities
│   ├── discord-protocol/ # Wire protocol serialization and partial payload patches
│   ├── discord-api/      # HTTP/2 REST client with rate limiting and backoff
│   ├── discord-gateway/  # WebSocket gateway client with heartbeat and resume
│   ├── discord-voice/    # Opus codecs, RTP/UDP transport, DAVE v1, Sonora AEC, RNNoise
│   ├── local-store/      # Bounded SQLite database for history, drafts, settings
│   ├── platform/         # OS credential store (Keychain/CredManager/SecretService)
│   └── test-support/     # Deterministic synthetic fixtures and mocks
└── tools/
    ├── replay-bench/     # Benchmarking harness for state reducers
    └── xtask/            # Workspace automation tasks (packaging, checks, linting)
```

---

## Security & Storage Policy

- **Token Protection:** Tokens are saved solely in the native OS credential store (macOS Keychain, Windows Credential Manager, Linux Secret Service). Plaintext token fallback is strictly prohibited. Active tokens remain redacted in memory.
- **Local Cache Bounds:** SQLite databases store recent channel history, drafts, settings, diagnostics, and image preview metadata within bounded byte and count limits. The local SQLite store is **not** encrypted by the application.
- **Sanitary Logout:** Executing an explicit logout destroys active network sessions, purges active secrets from memory, deletes the token from the OS credential store, and erases that account's local cache and drafts.
- **Zero Telemetry:** Serein contains no analytics, telemetry, background crash collectors, or tracking beacons.
- **Platform Integrity:** No fingerprint spoofing, CAPTCHA/MFA bypasses, bot substitutions, token scrapers, or third-party relays.

For full details, review the [Storage Policy](docs/storage-policy.md) and [Threat Model](docs/threat-model.md).

---

## Documentation

- [Architecture & Monorepo Design](docs/architecture.md)
- [Discord Compatibility & Protocol Details](docs/discord-compatibility.md)
- [Authentication & Login Handoff](docs/authentication.md)
- [Storage Policy & Cache Retention](docs/storage-policy.md)
- [Platform Support & Build Requirements](docs/platform-support.md)
- [Voice Architecture & Procedure](docs/voice.md)
- [User Profiles & Customization](docs/profiles.md)
- [Notifications & Sound Settings](docs/notifications.md)
- [Performance & Benchmarks](docs/performance.md)
- [Design Tokens & UI Styling](docs/design.md)
- [Third-Party Licenses & Notices](THIRD_PARTY_NOTICES.md)

---

## License

Original Serein code is dual-licensed under either:
- **MIT License** ([LICENSE-MIT](LICENSE-MIT))
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

at your option. Third-party library notices, bundled font licenses (Inter, Noto Sans CJK/Arabic), and Twemoji graphics licenses are cataloged in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Demo fixtures and simulated actions are excluded from normal app and CI packages.
Build with `--features demo` and launch with `--demo` to enable them; `--demo-*`
scenario flags additionally require `--demo`. `cargo xtask package` always builds
without demo support, while offline tests can still use synthetic fixtures.
