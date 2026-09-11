# Native Discord Client — AI Implementation Specification

**Scope:** An open-source, lightweight replacement desktop client for the **existing Discord service**, written in **Rust + egui/eframe**, for **Windows, macOS, and Linux**.

**Persistence policy (owner revised September 10, 2026):** Local SQLite databases, caches, drafts, settings, and diagnostic files are permitted with explicit limits. Save login tokens in the OS credential store for automatic sign-in. Use a temporary authentication-only webview for the official Discord login. No independent backend, account service, relay, or project-operated voice server.

**Specification date:** September 9, 2026. Reverify service behavior and dependency APIs when implementing.

**This specification supersedes the earlier “Relay” independent communication-service specification.** Do not carry over its backend, registration system, PostgreSQL, SQLite, durable outbox, migrations, object storage, LiveKit deployment, or custom messaging protocol.

---

## 1. Assignment and non-negotiable product identity

You are implementing a real native desktop **Discord client**, not a Discord competitor with its own service. Users bring their existing Discord accounts, guild memberships, channels, DMs, and history. The client communicates directly with Discord's existing infrastructure. Other people continue using the official client; they must not install our software or join a new service to communicate with our users.

Build the application in the repository. Do not stop at a plan, a UI mockup, empty crates, or a bot dashboard. Work in runnable, tested vertical slices. Preserve unrelated files and uncommitted changes. Inspect the repository and existing agent instructions before editing. Do not delete existing user work merely because the previous specification was wrong; isolate or carefully replace obsolete generated components and document the changes.

The priorities are correctness, account security, honest compatibility, bounded RAM, low idle CPU, small installation footprint, secure, bounded local persistence, and usable desktop interaction. A fast but unreliable client is not acceptable.

Use egui/eframe for the desktop messaging UI. **Owner-authorized exception:** a temporary platform webview displays Discord’s real login page and its available email/password, QR, MFA, and other authentication options. Close and release the webview once authentication finishes or is canceled; it must not render the messaging application or remain a hidden runtime dependency. No Electron/Tauri wrapper, JavaScript messaging frontend, official Discord desktop process underneath, or dependency on another client's local RPC server. **Owner revision (September 11, 2026):** Serein may host an opt-in, activity-only local Discord IPC endpoint so games can publish Rich Presence directly to this client. System webview runtimes are permitted solely for this login flow; account for their platform dependencies and temporary resource usage.

No mobile or browser build is required. Do not introduce a web frontend, mobile bindings, server deployment, synchronization service, or account database. A local mock HTTP/WebSocket endpoint for isolated tests is allowed, but is test infrastructure, not a runtime backend.

Use an original project name and original/licensed UI assets. Include an obvious statement that the project is unofficial and not endorsed by Discord. Prefer MIT OR Apache-2.0 for original code unless the repository already has a license; preserve existing licensing and dependency notices.

## 2. Local persistence, cache limits, and data ownership

**Owner revision (September 9, 2026): the previous “no storage” requirement is withdrawn.** Local files, embedded SQLite databases, disk image/message caches, saved drafts, session credentials, settings, window layout, and local diagnostic files are permitted. Closing the app should preserve saved login and drafts; cached history may be available on relaunch. This supersedes every earlier instruction to automatically discard all user data or to avoid a local database.

This remains a client for Discord’s existing service. A local database is a client cache, not an independent messaging backend, account system, relay, or production message-storage service. Discord remains authoritative for messages, permissions, membership, and account state.

### 2.1 Secure, bounded local storage

Save session tokens in the operating system credential store: macOS Keychain, Windows Credential Manager, and Linux Secret Service. Never fall back to plaintext token files. Do not save passwords, MFA codes, QR exchange secrets, or security-challenge answers. Prefer an ephemeral authentication webview; the saved token is sufficient for automatic native sign-in. Report credential-store failures clearly.

Store local data in the OS-standard application-data directory. Keep accounts isolated. Prioritize low RAM usage and a small package over minimizing cached disk data (owner clarification September 10, 2026). Use generous disk caches to speed up repeat navigation and avatar loads. Set explicit item, byte, and retention limits for disk caches as well as RAM caches; implement eviction and a visible cache-clear action. Do not grow a database or diagnostic log indefinitely. Cache hydration, SQLite work, image decoding, and file operations run outside the render callback.

Treat cached history as potentially stale until Discord revalidates it. Reconcile edits, deletions, permission changes, and missing events without resurrecting removed content from disk. A cache hit never grants authorization. Stop showing inaccessible channel content after revocation. Cache only content needed for normal user-driven navigation, not bulk account exports or member scraping.

### 2.2 Drafts, settings, logout, and explicit files

Save drafts and useful session preferences locally. Make save failures and unsaved/pending operations visible. Use transactional database writes or atomic file replacement for data that must survive ordinary restarts; do not claim recovery from every crash. Do not automatically resend uncertain message writes merely because their drafts or pending state survived.

On explicit logout, stop network and media work, clear active secrets and state, and remove the saved credential. Define what happens to local account caches and drafts; the initial implementation should clear that account’s local data on logout and expose any deletion failure. Ordinary application exit preserves the local cache, drafts, and saved login.

A user-selected attachment save writes to the chosen destination with bounded buffers and clear cancellation/partial-file behavior. Cached attachment previews may be automatic within the disk-cache policy; opening downloads or external applications is still a deliberate action. Upload only files the user chooses. Diagnostics, if written, must be redacted and bounded; exporting or sharing them is explicit. No telemetry or crash upload by default.

### 2.3 Honest privacy and resource claims

Document what is stored, where it lives, how it is cleared, and which local data is not encrypted by the application. Secure token storage does not imply SQLite message/draft encryption. Do not call the client “no storage,” “forensically traceless,” or claim Discord messages are ephemeral. Discord retains service-side data according to its behavior. OS swap, backups, crash collection, GPU drivers, credential stores, browser engines, and other system components have their own storage behavior.

Audit application and dependency writes to identify unintended storage and secret leakage. A zero-byte write target is no longer the product requirement. Report database/cache size, retained RAM, temporary peaks, authentication webview usage, and cleanup behavior separately.

---

## 3. Discord compatibility, authentication, and account risk

### 3.1 Research the actual integration before inventing a login screen

Discord's developer APIs are not a promise of full third-party, normal-user client functionality. Its documented OAuth scopes include limited account information and restricted or local-RPC capabilities; a bot, an OAuth application, a Social SDK integration, and a normal logged-in user session are different things. Do not claim that a basic OAuth authorization grants complete access to a person's chats. [S1]

Discord states that automating normal user accounts outside its OAuth2/bot API is forbidden and may lead to termination. Its terms also restrict unauthorized modifications and certain reverse engineering. An unofficial replacement client therefore carries policy/account risk; technical interoperability does not establish platform approval. Do not promise that interactive-only use, open-source licensing, or a warning dialog makes the client approved or ban-proof. [S2][S3]

Create `docs/discord-compatibility.md` before extensive integration. For each capability record:

- Which credential/session type it actually requires.
- The official documentation or dated public implementation evidence used.
- Whether it is public/documented, restricted/approval-dependent, or unofficial/unstable.
- Whether our implementation was tested, on what date, and with what limitations.
- The fallback or visible unsupported state.

Cover authentication, guild/channel access, DMs, message history/send/edit/delete, realtime events, read markers, relationships, uploads, search, voice, and logout. Public bot documentation is useful protocol evidence, but is not proof that a normal-user session accepts identical flows, intents, fields, or quotas.

### 3.2 Required authentication boundary

Implement an `AuthProvider`/session boundary independent of egui and the REST client. Model unauthenticated, authenticating, challenged, authenticated, expired, and failed states. Session material belongs in a redacting, non-serializable secret type; restrict copies and clear owned secret buffers where practical.

**Owner clarification (September 9, 2026; supersedes the earlier native-password-form clarification):** the product login is Discord’s real hosted login screen in a temporary embedded platform webview, not OAuth2, a fabricated native password form, or a token-paste screen. The user completes Discord’s own available email/password, QR, MFA, CAPTCHA, and other login options. Do not promise every method works on every embedded platform before testing it.

The application may receive the resulting token from **its own newly created, owner-operated authentication webview session** through a narrowly scoped, origin-checked integration. This is not permission to read another application, browser profile, unrelated tab, or existing Discord session. Document the actual token handoff and its unofficial/unstable aspects before implementation. Do not collect or expose the password, QR exchange, or challenge answers in Rust, logs, or diagnostics. Close and clear the webview after the handoff; keep the active token redacted in RAM and save it only in the OS credential store for automatic sign-in.

Preserve normal certificate validation and origin boundaries. Do not imitate official device fingerprints or bypass challenges. If Discord rejects the embedded browser, a login method is unavailable, or no verified token handoff works, show the precise blocker. Never fall back to credential extraction, OAuth scopes that lack chat access, a token broker, or a hidden official messaging client.

Where full normal-user interoperability requires unofficial behavior, isolate it, document the evidence and limitations, and label it honestly. An optional developer-only, session-only credential input may accept a credential deliberately provided by its account owner for adapter testing, but must not replace the required official Discord login webview; that is not permission for the agent or application to obtain credentials elsewhere, and is not evidence of Discord approval. Never disguise a credential import as an ordinary approved OAuth login.

Do not extract tokens from the official Discord application, browser profiles, local databases, process memory, or other software. Do not ask users to run opaque console scripts. Do not put tokens in command-line arguments, URLs, shell history, telemetry, fixtures, or CI. Do not harvest QR login sessions. Do not implement CAPTCHA bypass, MFA bypass, device-verification bypass, fingerprint spoofing for evasion, proxy rotation, or anti-detection behavior.

A legitimate challenge must be completed through an implemented, verified user-controlled flow. Otherwise, stop that login attempt and explain the limitation. Do not retry around it or tell the UI that authentication succeeded.

The coding environment may lack a user-owned test session or required service permissions. Implement the real adapter and its tests where evidence supports it, but mark live validation blocked when appropriate. Continue useful non-live work rather than substituting a bot or another service. A mock session is never proof of Discord compatibility.

### 3.3 Human-operated client only

The product is for deliberate interaction with the user's own account. Do not add mass DMs, automated replies, member scraping, account enumeration, bulk history exports, spam tooling, token checking, or bulk moderation automation. Normal event-driven fetching and bounded viewport pagination should serve the visible client, not background collection.

Live tests must be explicitly enabled by the owner, small, and limited to accounts/channels they control. Do not create accounts, join unrelated guilds, publish messages, or run load tests against Discord autonomously.

## 4. Monorepo structure and responsibilities

Use one Cargo workspace with shared dependencies, a committed lockfile, and a pinned stable Rust toolchain. Use a layout like this; retain smaller responsibilities as modules until separate crates are justified.

```text
native-discord/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── .cargo/config.toml
├── SPEC.md
├── AGENTS.md
├── README.md
├── LICENSE-MIT
├── LICENSE-APACHE
├── SECURITY.md
├── CONTRIBUTING.md
├── THIRD_PARTY_NOTICES.md
├── apps/
│   └── desktop/                # Startup, eframe integration, dependency wiring
├── crates/
│   ├── model/                  # Typed IDs, UI-neutral entities, value types
│   ├── client-core/            # Session state, reducers, commands, bounded views
│   ├── discord-protocol/       # REST/Gateway DTOs, patch types, wire decoding
│   ├── discord-api/            # HTTP, authentication adapter, rate scheduling
│   ├── discord-gateway/        # WebSocket lifecycle, decoding, resume handling
│   ├── session-cache/          # Explicitly bounded RAM working set
│   ├── local-store/            # Bounded account-isolated SQLite history/drafts cache
│   ├── ui/                     # egui layout, timeline, composer, settings
│   ├── platform/               # OS integrations, approved save/open operations
│   ├── discord-voice/          # Discord-compatible voice signaling and media
│   └── test-support/           # Synthetic events, clocks, transport mocks
├── tools/
│   ├── xtask/                  # Cross-platform build/test/package tasks
│   └── replay-bench/           # Synthetic local event/scroll/soak workloads
├── assets/                     # Original or appropriately licensed assets
├── packaging/
│   ├── windows/
│   ├── macos/
│   └── linux/
├── tests/
├── docs/
│   ├── architecture.md
│   ├── discord-compatibility.md
│   ├── authentication.md
│   ├── storage-policy.md
│   ├── performance.md
│   ├── threat-model.md
│   ├── platform-support.md
│   ├── dependency-versions.md
│   ├── progress.md
│   └── adr/
└── .github/workflows/
```

There is no `apps/server`, server deployment, database container, object store service, or production relay. A local embedded database and its schema/version handling are allowed under the revised storage policy.

`model` must not depend on GUI, networking, audio, or filesystem crates. `discord-protocol` owns service wire shapes; do not make raw JSON objects the application state. `client-core` owns application behavior and ports. Adapters report typed outcomes. The UI receives bounded state/views and emits typed commands; it does not send HTTP requests, own tokens, implement gateway reconnects, or decide server authorization.

`session-cache` owns the RAM working set; `local-store` owns explicit local persistence with bounded account-isolated data. Keep egui texture handles in the rendering side rather than generic domain entities. Keep voice transport and native media dependencies out of a text-only build. `apps/desktop` composes the parts and remains thin. Do not introduce a plugin system or generic enterprise framework.

## 5. Technology policy

Use Rust, egui/eframe, one selected rendering backend, Tokio, serde, maintained HTTP/WebSocket clients, and a maintained TLS implementation with normal certificate verification. Use typed errors and redacted structured diagnostics. Start with wgpu, retaining Linux Wayland/X11 integration and accessibility support as appropriate. Do not enable multiple renderers by default.

Pin mutually compatible egui ecosystem versions. Inspect actual feature flags and current API examples; do not copy stale application-trait implementations. Record the Rust version, native dependencies, target architectures, licenses, and resolved versions. The current eframe documentation is an implementation reference, not a guarantee that a different version uses the same API. [S10]

Do not assume a bot SDK can become a normal-user client by changing an authorization header. Audit candidate libraries for credential assumptions, event shapes, mandatory full caches, feature weight, and actual service compatibility. Reuse sound protocol primitives without inheriting a bot application's architecture.

Prefer safe Rust. Isolate unavoidable FFI, document ownership and callback requirements, and test teardown. Do not recreate cryptographic algorithms or add a large engine merely to render UI. No JavaScript messaging runtime; the owner-authorized authentication webview may execute Discord’s login page and the audited handoff script; a necessary audited C/C++ media library behind Rust bindings is an explicit, documented dependency rather than an “all Rust” claim.

## 6. Core runtime and state ownership

Run egui/windowing on the required UI thread. Run networking and bounded worker jobs outside it. No blocking HTTP, filesystem access, image decoding, large text parsing, or audio work in the render callback.

Use typed commands such as `SelectChannel`, `RequestHistory`, `SendMessage`, `EditMessage`, `DeleteMessage`, `SetReaction`, `MarkVisibleMessagesRead`, `JoinVoice`, and `Logout`. UI gestures produce commands; outcomes change core state; only relevant visible changes request repaint.

Choose a clear state owner, such as one client-core task receiving ordered semantic events. Publish bounded immutable views, revisioned snapshots, or small change sets. Do not deep-clone all account state per frame or retain an unlimited chain of snapshots. Do not hold a global mutex while rendering.

Maintain a session generation identifier. Tag pending work so responses from a prior account/session cannot update the current session. On logout, cancel fetches, uploads, image decodes, reconnects, and voice tasks; reject late callbacks; release textures and state; clear secret buffers where feasible. Do not delete already-sent Discord messages as part of local logout.

When authentication expires, stop authenticated traffic and present a clear state. Do not spin on 401 responses. Preserve unsent text in session RAM while the user handles the error, and save drafts locally unless they explicitly discard them.

All queues need item and byte budgets plus an explicit full-queue policy. Coalesce replaceable state such as presence/typing. Do not silently drop message mutations or permission changes while claiming the view is current. Under sustained overload, enter a controlled reconnect/resync or error state. Keep heartbeat/control processing independent of a slow UI or overloaded thumbnail worker.


## 7. Direct Discord networking and synchronization

### 7.1 HTTP adapter

Centralize authenticated request construction, origin validation, error mapping, cancellation, and response limits. Never send Discord authorization to attachment hosts, arbitrary redirects, user-supplied links, or diagnostic endpoints. Use a separate credential-free client for media URLs; validate signed upload/download targets and redirects without adding account credentials.

Respect service-provided bucket and global rate-limit information, and the retry delay on 429 responses. Do not hardcode a bot quota as the normal-user quota. Stop inappropriate retries on authentication/permission failures. Schedule interactive requests ahead of prefetch work without skipping a rate limit. [S5]

Bound concurrent operations and pending work. Deduplicate simultaneous requests for the same visible resource. Cancel obsolete history/search fetches after navigation. Use deadlines and a finite retry budget; do not repeatedly retry writes whose outcome is unknown.

Do not assume the client can add endpoints, indexes, message sequencing, acknowledgements, or event-replay features to Discord. Implement the service contract actually available and represent limitations in the UI.

### 7.2 Gateway adapter

Implement the verified gateway/session lifecycle: connection, Hello, heartbeats and acknowledgements, Identify or Resume as appropriate, readiness, dispatch, reconnect, invalid-session handling, and close-code handling. Keep the session identifier, resume URL, and dispatch cursor only in RAM. The documented gateway describes replay after successful resume; it is not an unlimited history service. Verify differences for the credential type used here. [S4]

Use a tested reconnect state machine, not nested retry loops. A disconnect with a resumable session and a fresh connection are different transitions. Use backoff and jitter without ignoring explicit server delays. Do not blindly re-identify after every network hiccup.

Negotiate a supported encoding/compression configuration and implement it correctly. Do not assume compressed frames can be decompressed independently: Discord documents connection-scoped streaming compression modes. Cap both compressed input and decompressed output, and fail clearly if a valid account snapshot exceeds the configured safe capacity. [S4]

Unknown event variants must not crash the process. Safely ignore explicitly irrelevant events, but record bounded, non-sensitive compatibility diagnostics. Unknown security-relevant states must not silently grant capabilities. Do not retain an unbounded raw-event archive.

### 7.3 Merge, freshness, and partial updates

Use typed Discord IDs and preserve wire precision; never route IDs through floating-point values. Distinguish absent, null, and present patch values where the wire contract distinguishes them. Decode partial updates into patch types rather than replacing complete records with missing-field defaults.

Correlate HTTP responses and gateway events so either arrival order produces one message. A late history response must not resurrect an already-deleted message or overwrite a newer edit with an older record. Use request generations, bounded pending mutation/tombstone state, and targeted revalidation. Set explicit lifetimes and eviction rules for reconciliation metadata.

After a non-resumable reconnect, do not claim that “fetch messages after the last ID” proves all old edits/deletions have been caught. Revalidate the active view and mark unrevalidated cached views stale. Only announce fresh state after the corresponding synchronization step succeeds.

Server-provided authorization is authoritative. Mirror current Discord roles and channel overwrites for UI affordances; do not invent Owner/Admin/Member roles or simplify Discord's permission hierarchy into local rules. Handle revocation by stopping access, invalidating affected content, and displaying an appropriate unavailable state. [S7]

Do not fetch the full guild-member directory or all channel histories on login. Keep compact navigation metadata and hydrate details on demand. Determine subscription capabilities from the verified session protocol; do not fabricate a “subscribe to only one channel” command because it would save RAM.

## 8. Memory budgets and egui rendering

### 8.1 Working set

Begin with approximately 200–500 messages around the active timeline, further limited by bytes. Keep a small global MRU of recent channel windows, not an unlimited cache per channel. Limit all hydrated history across views, search, replies, and side panels together. Navigation metadata, selected-item accessibility, and pending user input have separate explicitly bounded policies.

Set limits for message data, parsed Markdown, text layout, decoded CPU images, GPU textures, compressed downloads, inflight work, entity summaries, and diagnostics. A row-count limit alone is insufficient. Reject oversized single items or offer an explicit limited presentation; do not quietly defeat the budget for one attachment.

Suggested initial component ceilings for investigation, not benchmark claims:

| Component | Initial policy |
|---|---|
| Hydrated message records | At most 1,500 total AND a 16 MiB estimated content/metadata budget |
| Parsed text and reusable layouts | 8 MiB, revision/width-sensitive eviction |
| Decoded CPU preview images | 16 MiB shared cache |
| GPU preview textures | 32 MiB shared cache, measured separately |
| Compressed image work buffers | 8 MiB combined, plus an explicit per-item limit |
| Concurrent thumbnail decodes | 2 |
| General in-flight HTTP requests | 4 initially, with independent heartbeat control |
| Drafts and pending sends | 2 MiB combined; reject new work clearly when full |
| Redacted diagnostic ring | 1 MiB maximum |

These are not the whole-process memory footprint. Account for framework state, fonts, GPU surfaces, TLS, gateway buffers, native libraries, allocator overhead, and temporary peaks. Essential account metadata and initial snapshots need separately measured bounds; do not truncate protocol data and present a corrupted account as valid. Document what happens for unusually large accounts.

Unsent drafts are not an evictable image cache. Within the open session, do not discard pending user work silently to satisfy a budget. Ask for an explicit action or stop accepting additional work with a clear explanation. Save drafts within the documented local-store budget; never silently discard unsaved work to meet it.

### 8.2 Virtualized, variable-height timeline

Render only visible rows plus small overscan. Clipping after visiting every stored message is not acceptable virtualization. egui's `show_rows` uses a common row height; use a compatible variable-height virtualizer or a tested viewport-based implementation for mixed text, replies, code, and attachments. [S12]

Maintain a visual anchor consisting of a message ID and offset. Preserve it when earlier history loads, images become available, a message changes, or sidebar/font dimensions change. Reserve thumbnail geometry from safe metadata where available. Keep approximate off-screen heights and refine them without jumps.

Bound height/layout caches and invalidate by content revision, available width, font scale, wrapping rules, and attachment dimensions. Avoid relayout of every historical message on each frame. Handle an exceptionally long code block without rendering thousands of off-screen lines repeatedly.

Selection and keyboard focus must survive normal viewport changes. Virtualization must not make an accessible focused item disappear unexpectedly. Provide reliable per-message copy actions even before cross-message selection is complete.

### 8.3 Repaint only when useful

Request repaint after visible state changes or input. Use delayed repaint for a clock/typing expiry that is actually displayed. Never request an immediate repaint unconditionally at the end of every frame. egui provides immediate and delayed repaint requests and can be awakened from background work through the application integration. [S11]

Pause off-screen animation, throttle visual voice meters, and stop needless work in hidden/minimized views. Do not block protocol heartbeats when minimized. Idle typing indicators and irrelevant presence changes must not keep the entire UI continuously rendering.

CPU/GPU resource limits are separate. Release texture handles and image-loader cache entries on eviction; egui exposes image-forgetting APIs, but the application must supply its own eviction policy. [S11]

## 9. Desktop experience and functional behavior

### 9.1 Layout and navigation

Use a compact familiar layout: guild rail, channel/DM sidebar, channel header, central timeline, multiline composer, and optional member/details pane. Include a clear connection state and a compact voice control area only while relevant. Keep navigation resizable, keyboard-operable, and responsive at different display scales.

Use consistent typography, spacing, focus indication, contrast, dark/light themes, and sensible density. Persist useful settings locally and explain the storage and reset controls. Do not introduce remote theme downloads, ads, quests, store pages, or decorative activity that increases idle usage.

Keep unavailable channel kinds and unsupported message types recognizable with an honest placeholder and an explicit external-open action where safe. Never pretend unknown content does not exist merely because the renderer lacks a widget. Do not ship a button that appears functional but silently does nothing.

### 9.2 Text-first release

Implement normal-user access to existing guild text channels and existing one-to-one DMs, channel switching, recent history, backward pagination, live incoming messages, text sending, replies, edits/deletes where authorized, basic mentions/reactions, and attachment previews/upload/download where the chosen adapter supports them.

Then expand to existing group DMs, threads/forum navigation, pinned messages, richer embeds, search, presence, unread/read markers, and notifications. Every enabled feature must use verified Discord behavior. Do not substitute guild bot membership for the user's own navigation or friends.

Do not build guild hosting, account registration, password reset, custom role administration, server moderation dashboards, bot creation, paid entitlements, or a separate friends database. Links to official account/security settings may be opened explicitly rather than reimplementing sensitive settings in v1.

### 9.3 Message rendering and composer

Support a safe, useful Markdown subset, code blocks, quotes, spoilers, links, user/channel references, emoji, replies, edited state, and deletion state. Preserve original text separately from its display representation. Never execute scripts or HTML from a message. Restrict nesting and layout complexity for adversarial input.

The composer supports multiline editing, undo/redo, paste, attachment selection/drop, and a visible reply context. Enter sends and Shift+Enter inserts a newline by default. Do not send when Enter is confirming an input-method composition. Test actual behavior with CJK input, combining characters, mixed-direction text, emoji sequences, and keyboard shortcuts.

A send begins as a local pending item; drafts may be saved locally, but pending-send delivery state must remain honest. Do not claim delivery after a socket write. Use the actual service response/event to confirm the resulting message. Keep ambiguous outcomes visibly distinct from rejection. A timeout may mean the server accepted the message but the client missed the response.

Discord documents nonce-based create-message deduplication with limited temporal scope. Verify applicability for the selected endpoint/session before using it. Never promise indefinite idempotency or add an invented idempotency header. Reuse correlation identifiers for a logical retry only where the real contract supports it. [S6]

When offline, save the draft locally and make pending-send status visible. Do not automatically replay an outbox with uncertain outcomes. Before resending an ambiguous operation, reconcile when possible or ask for deliberate retry with a duplicate-risk explanation. Do not equate matching text with message identity.

### 9.4 Read state and notifications

Update remote read state only through verified APIs and deliberate viewing/mark-read semantics. When synchronization of read state is unavailable, label indicators as session-local instead of claiming cross-device consistency. Do not derive exact unread counts by subtracting message IDs.

Track whether the user is following the latest messages. Only autoscroll for incoming content while following the bottom; otherwise preserve their reading position and show an indicator. Replay/reload must not cause duplicate notification storms.

Use in-app notifications by default. Native OS notifications require a session-level opt-in that explains OS notification history can persist outside the client. Hide message bodies by default, honor verified mute/DND settings, and clear outstanding notifications on logout where the platform permits. Never claim clearing them erases OS records.

### 9.5 Accessibility and integration

Expose meaningful names, roles, selected/focused states, and actions through the supported accessibility integration. Test keyboard-only navigation and screen readers on the claimed platforms. Keep copy/paste, ordinary system shortcuts, DPI scaling, focus, and link handling functional.

A tray icon and background residency are optional, off by default. Closing the application should actually exit unless the user explicitly selects session-only background behavior. Do not add autostart, hidden agents, or a daemon by default. Local preferences are allowed.

## 10. Attachments, previews, and external content

Only load previews needed near the viewport. Prefer correctly sized service-provided images; cap downloaded bytes, decoded pixel dimensions, frame count, and decoder concurrency. The byte size of a compressed image is not a decoded-memory budget. Use placeholders and explicit load/open actions for unusually expensive media.

Use a dedicated credential-free media downloader. Restrict automatic fetching to verified service-provided media destinations. Do not automatically fetch arbitrary URLs from message text to generate previews, request loopback/private-network URLs, follow unsafe schemes, or leak account credentials through redirects. Bounded disk preview caching is permitted. Treat signed URLs as sensitive and exclude them from logs.

Avoid automatic animated avatars, stickers, GIFs, video playback, or preview audio. Allow a deliberate play action for supported media, stop it off-screen, and release decoder buffers on close. A static representation is the default lightweight experience.

Uploading should stream a user-selected file with bounded buffers, expose progress/cancel, enforce verified service limits, and report server rejection. A changed/missing source file is an explicit error, not a reason to copy it to a hidden recovery store. Upload protocols must be current; do not assume an old multipart example covers every normal-user attachment workflow.

Saving requires an explicit destination. Validate filenames and prevent path traversal. No automatic external execution. Opening an attachment in another application is deliberate and may create external history/cache records; do not promise otherwise.


## 11. Voice must interoperate with Discord

**Owner scope revision (September 10, 2026): implement voice calling in existing one-to-one DMs and guild voice channels.** Starting/answering DMs, joining/leaving server voice channels, participant rosters, microphone capture, mixed speaker playback, mute/deafen and required DAVE group encryption are in scope. Group DMs, Stage channels and camera video remain excluded; the September 11 revision below adds outgoing screen sharing. Keep an explicitly selectable text-only build and measure the additional voice package cost. The live two-way audio gate remains mandatory for claims of interoperability.

Voice is a later, separately verified release milestone, not a reason to create another service. Do not use a self-hosted LiveKit room, custom SFU, custom signaling backend, or an unrelated WebRTC demo as evidence of Discord voice support. The client must communicate with users already in actual Discord calls.

First validate a minimal voice integration in parallel with UI work so an authentication/protocol blocker is found early. Keep the default development build text-only until the media integration is ready, and report voice-enabled binary/RAM costs separately. No microphone access or media-runtime initialization before an explicit call/device-test action.

Discord's voice documentation states that its listed DM, group-DM, voice-channel, and Go Live conversations require end-to-end-encrypted calls starting March 1, 2026. Implement current DAVE compatibility; a legacy transport-only voice implementation is not sufficient for that requirement. [S8]

Use the current service voice signaling/transport contract and a maintained, interoperable media implementation. Keep audio capture/playback, packet processing, codec work, and DAVE state behind a narrow Rust API. Discord publishes `libdave`, including a C++ implementation, as an implementation reference/component. Evaluate audited bindings or a small reviewed FFI layer instead of inventing cryptography. `libdave` is not a complete capture/playback/call engine. [S13]

Verify entry, disconnect, reconnect, participant changes, encryption transitions, microphone permission, device selection, device loss, mute, deafen, gain, and push-to-talk. Do not send captured microphone audio before the user joins a call and the call is ready for the required security state. Do not silently downgrade required encryption to make a call connect.

Use session-ephemeral voice keys where supported by the verified protocol; any persistent voice identity requires separate verified protocol/security design. Explain any identity-verification consequence instead of implying keys are remembered across launches. Do not claim text-message E2EE merely because a voice call uses DAVE.

Keep audio callbacks real-time-safe: preallocated bounded buffers, no HTTP/filesystem work, no render locks, and no unbounded allocations. Test echo behavior, resampling, device switching, and jitter/packet loss with real audio, not just a connected participant list. Release audio devices, tasks, decoders, queues, and keys after leaving.

Focused-window push-to-talk is the initial guarantee. Global push-to-talk and Linux desktop integration need separate platform capability testing and an explicit fallback. Do not promise identical global-hotkey behavior on every desktop environment.

**Owner scope revision (September 11, 2026): add outgoing screen sharing on macOS and Windows.** Clicking Share opens settings for a screen/window, 720p or 1080p, 15/30/60 fps and cursor visibility before explicit capture. All quality choices are exposed without a local Nitro gate; service acceptance is not an entitlement bypass or a verified compatibility claim. Use native OS capture and Discord-compatible stream transport/DAVE encryption. Camera video and receiving streams remain later capabilities. Do not bundle large video dependencies into the text-only build preemptively. No recording or audio/video file cache by default.

## 12. Security, privacy, and source integrity

The app has no project-controlled data collection, relay, analytics, telemetry, crash submission, update tracking, or remote configuration. Default networking should be limited to the authenticated Discord features the user invokes and necessary validated service media endpoints. Dependency downloads during development are separate from runtime behavior.

Make credential handling reviewable in a small part of the codebase. Deny accidental serialization of secrets. Redact request headers, URL query parameters, session identifiers, message bodies, attachments, and user-specific payloads from diagnostics. Do not derive `Debug` over whole network state containing credentials. Write tests that use synthetic secret markers to detect accidental leakage.

Credentials never go into screenshots, Git commits, issue templates, CI variables for public workflows, or generated progress reports. Use synthetic accounts/content in visual regression images. Network fixtures must be handcrafted or thoroughly sanitized with consent; do not commit captured real user histories.

Validate TLS normally. Never add a release-mode “accept invalid certificates” convenience. Restrict automatic URI handling, prevent unsafe filename paths, and keep external link opening separate from native shell execution. Decode untrusted images and formatted text with resource limits.

Do not redistribute Discord's proprietary binaries, client source, logos, emoji collections, or fonts as if they were ours. Distinguish interoperating with user-provided service content from bundling proprietary brand assets. Review licenses of any copied open-source interoperability code and preserve required notices.

Account ownership and protocol compatibility do not resolve every platform/legal question. Keep the unofficial-client risk visible in the README and authentication information, without deceptive “safe mode” or “undetectable” claims. Do not use risk documentation as an excuse to invent functioning authentication or to label an offline prototype a completed client.

## 13. Performance acceptance contract

These are **starting engineering targets**, not measured results or promises. Establish reference devices and workloads before optimizing. Record misses honestly and improve them rather than silently weakening the test or eliminating required functionality.

| Measurement | Initial target |
|---|---|
| Native window interactive, before authentication | p95 under 1 second on the documented reference device |
| Logged-in, settled text-only idle | At most 80 MiB under the defined per-platform process-memory metric |
| Active text channel with bounded thumbnails | At most 150 MiB under that same metric |
| Switching to a channel already resident in this session | p95 under 100 ms |
| 60 Hz scrolling workload | UI/frame work within approximately 16.7 ms at p95, with missed frames reported |
| Idle CPU | Near zero apart from protocol/lifecycle work; no continuous animation loop |
| Local persistence | Documented bounded caches/drafts/settings; tokens only in OS credential store; no secret leakage |
| Initial text-only compressed distribution | Aim for at most 30 MiB; report required assets/native dependencies |
| Text-only installed footprint | Aim for at most 75 MiB; report the full package |
| Long-running repeated workload | Retained memory reaches a stable bounded range, not monotonic growth |

Report CPU memory, GPU allocations, application helper processes, and external system components separately. Do not measure only the Rust executable while excluding required helper processes. Do not count a shared allocation multiple times without saying so. State the memory metric, sampling method, warmup, renderer, display scale, hardware, OS, build profile, feature flags, and test corpus.

Network-backed cold login and history retrieval are different from opening an already-resident channel. Measure cold startup and local-cache startup separately. Measure bytes fetched per navigation/relaunch and local cache hit rate and refetch costs.

Measure voice-enabled installation and call memory separately with a defined participant count and codec configuration. Leaving a call should release the call working set; it need not remove already loaded code pages from the process instantly.

Experiment with compiler profiles and dependency features using release builds. Preserve useful separate debug symbols for development/release diagnosis without bundling them unnecessarily. Record actual packaged sizes. Rust, native rendering, and lack of Electron are architectural choices, not substitutes for measurements.

## 14. Testing and CI

### 14.1 Offline-first engineering tests

All default tests must run without a Discord account, a user token, a real network side effect, an external database service, or a deployed service; local SQLite tests use temporary synthetic files. Use deterministic clocks, synthetic Discord-shaped fixtures, local mock transports, and a labeled demo mode that cannot accidentally contact the real service. Test-only networking must not become a shipped relay.

Test reducers, patch merging, entity normalization, request cancellation, session generations, navigation, timeline anchors, pagination boundaries, and cache eviction. Test byte budgets as well as item counts. Ensure replacing a message releases obsolete layout/image references.

Test HTTP/gateway arrival-order permutations, duplicate events, missing fields, edits/deletes during history loads, stale responses after logout, failed permissions, credential expiry, slow responses, rate limits, reconnect, invalid sessions, and failed resumption. Include truncated/oversized compressed input and unexpected variants without panicking or retaining uncontrolled buffers.

Test send success, rejection, ambiguous timeout, safe retry, duplicate confirmation, and process termination. The expected termination behavior is recovery of successfully saved drafts and cached history, with uncertain sends never automatically replayed.

Test Markdown nesting limits, link safety, malicious filenames, enormous image dimensions, thumbnail eviction, and cancellation of a decode after channel change. Fuzz appropriate decoders and state-transition inputs.

### 14.2 Storage-policy verification

Use a fresh isolated test profile/home where practical, and instrument process file creation/write activity. A before/after directory diff alone is insufficient because a dependency could create and delete a temporary file during the run.

Exercise launch, authentication with synthetic credentials, browsing, composing, sending, image preview, settings, reconnect, voice teardown, logout, normal exit, and forced termination. Verify writes match the documented storage policy: account-isolated bounded SQLite/cache files and credential-store tokens, with no plaintext secrets or unbounded logs. Audit draft recovery, cache eviction, account separation, clear-cache/logout deletion, and authentication webview writes. Distinguish app writes from known OS/driver artifacts and report unresolved writes instead of ignoring them.

Test persistence configuration and dependency default-feature changes against the chosen storage policy. Explicit save/download/export tests should write only to an authorized test destination and clean up their own test files. Never use real user data in a write-trace report.

### 14.3 Desktop and accessibility tests

Build and test supported release configurations on Windows, macOS, and Linux. Define actual supported architectures/minimum OS versions based on dependencies and testing rather than assuming every historical OS works.

Exercise Wayland and X11 where supported, display-scale changes, clipboard, native dialogs, screen-reader navigation, IME, focus, window resizing, text selection, and suspend/resume. Mark platform tests not actually run as unverified. A successful cross-compilation does not prove working audio, accessibility, or packaging.

Use a synthetic large-account fixture and repeated channel navigation without preloading every fixture record into the measured client. Include a long-running replay/scroll soak test, repeated image open/close, slow-link recovery, and repeated call join/leave in the media test environment. Benchmark infrastructure must not change the shipping cache policy to manufacture results.

### 14.4 Live compatibility tests

Provide a separate manual, explicit opt-in path with an owner-controlled session and a private test conversation. Never run it in ordinary CI or treat a bot-token test as normal-user-client validation. Do not request credentials through the AI chat or commit them to the repository.

Verify that a message sent from the native client appears in the official client and that an official-client reply appears natively. Verify edits/deletes and reconnect in the same controlled environment. When voice is ready, verify actual two-way audio with an official client in a private call.

Use small normal interaction volumes. All rate/load/failure stress tests run locally with synthetic data, never against Discord production. Report the test date and the exact features validated, without identifying other users or exposing messages.

### 14.5 CI and packaging

Automate formatting, linting, unit/integration tests, feature-combination builds, dependency/license/security checks, and per-platform packaging smoke tests. Cache build dependencies in CI, not user runtime data. Do not package synthetic credentials, fixtures with secrets, debug logs, or developer-only credential tools by accident.

Provide straightforward source-build instructions and an `xtask` entry point for recurring development commands. Produce normal platform packages; minimize installed dependencies without hiding required runtimes. Signing/notarization is optional until signing identities exist; do not claim artifacts are signed without verification.

Start with manual release downloads. Do not add a background updater or updater telemetry by default. If an updater is later requested, its disk/network behavior needs a separate requirement change.

## 15. Milestones and honest completion gates

### Milestone 0 — Correct scope, native shell, and feasibility evidence

Inspect the repository. Replace obsolete instructions with this client-only scope while preserving unrelated work. Create a compact `AGENTS.md`, workspace, runnable egui shell, synthetic fixture mode, and initial storage/performance checks. Produce the capability/authentication matrix with current sources. Test real text composition and variable-height scrolling early.

In parallel, identify a technically viable authentication/session path and assess the current Discord voice/DAVE path. Do not spend the whole milestone styling a fake login screen while ignoring the account-access problem.

**Gate:** A runnable native app; no backend or external database runtime dependency; documented bounded local storage and secure saved-login behavior; documented integration assumptions and blockers. This is not yet a working Discord replacement.

### Milestone 1 — Genuine normal-user text vertical slice

Authenticate through the official Discord login webview or restore the securely saved token (or use an explicitly authorized developer-only owner-supplied session while testing the adapter), then connect that normal-user session through the actual adapter. List appropriate existing guilds/channels or DMs, load a bounded history page, receive messages through the real gateway, and send a user-composed message to the existing Discord conversation. Handle rate limits and invalid credentials without retry storms.

**Gate:** The native client exchanges a real message and reply with an official client in a private owner-controlled test. No bot substitution, new service, remote relay, or mocked live success. If credentials or a viable flow are unavailable, mark this gate blocked; do not label it complete.

### Milestone 2 — Reliable text client

Implement revisions/deletions, response/event reconciliation, backward pagination, visual anchoring, saved drafts, ambiguous-send handling, reconnect/resume, freshness states, and bounded caches. Validate repeated navigation and logout cancellation. Use the local database for bounded caches and saved drafts; do not invent Discord delivery guarantees.

**Gate:** Failure-mode tests pass; memory stabilizes under repeated synthetic usage; storage audit verifies bounded documented persistence without secret leakage; the active view becomes correct after supported reconnect paths.

### Milestone 3 — Everyday desktop messaging

Complete useful message formatting, mentions/reactions, attachments, search/read-state capabilities that are actually available, session preferences, accessibility, native integration, and richer channel kinds. Unsupported functionality remains visibly unsupported, not simulated.

**Gate:** Feature matrix distinguishes completed, partial, unsupported, and untested items. The ordinary text workflow is usable with keyboard and multilingual input, not just screenshots.

### Milestone 4 — Discord-compatible voice

Integrate real microphone capture and remote playback, current required DAVE behavior, call participant state, device selection, mute/deafen, focused push-to-talk, reconnect, and teardown. Document native library footprint and platform limitations.

**Gate:** Actual two-way audio with an official Discord client, relevant encryption/participant-transition testing, verified resource cleanup, and no recording/persistence. A successful socket connection or local loopback is not this gate.

### Milestone 5 — Open-source release hardening

Complete packaging, dependency notices, contribution/security docs, accessibility/platform reports, long-running tests, and benchmark reports. Provide an accurate unofficial-client risk statement and local-storage policy and secure saved-login behavior. Revalidate compatibility before release.

**Gate:** Reproducible documented build steps, actual platform evidence, measured resource reports, no embedded secrets, and an honest release feature matrix. Do not advertise full Discord parity.

Video and screen sharing follow as separately scoped, verified milestones. Do not defer the entire useful text client while chasing them.

## 16. Agent workflow and final deliverables

Start by reading this entire specification and the repository. Create a short implementation plan, then implement the next complete vertical slice. Do not finish a session having only generated planning documents when implementation work is possible.

Choose reasonable minor defaults and record consequential decisions in concise architecture notes. Do not repeatedly ask for file names, spacing values, or dependency preferences. Do not resolve an API limitation by changing the requested product into a new service. Do not take external account actions without the owner-controlled live-testing boundary.

At each session/milestone boundary update `docs/progress.md` with:

- The latest completed gate and exact implemented behavior.
- Commands actually run and their real results.
- Measurements, environment, and whether they used synthetic or live data.
- Compatibility assumptions and important sources/date checked.
- Known defects, blocked capabilities, untested platforms, and the next concrete step.

Never include credentials, message content, private account IDs, or real signed URLs in progress reports. Application diagnostics, if added, must be redacted, bounded, and locally controlled.

The final implementation deliverables are the source workspace, runnable native client, platform build/package instructions, offline tests, an explicit live-validation procedure, compatibility matrix, storage audit, performance report, dependency notices, and precise limitations. A prompt, plan, scaffold, or fixture-only UI does not satisfy the completed-client target.

The core invariant is:

> This is a native UI and protocol client for Discord's existing service. It owns no messaging backend and uses bounded account-isolated local storage, and keeps saved session tokens in the OS credential store. Every in-memory cache and queue has a limit; every network feature has verified behavior; every claimed success has evidence.

## 17. Primary-source reference checkpoints

These sources were consulted for this specification on September 9, 2026. Recheck them during implementation. They support the specific factual compatibility notes; performance budgets, architecture, and milestone choices are this project's proposed requirements. Public developer documentation is not blanket approval or proof of normal-user endpoint compatibility.

- **[S1] Discord OAuth2 and scope documentation:** `https://docs.discord.com/developers/topics/oauth2`
- **[S2] Discord policy on automated user accounts/self-bots:** `https://support.discord.com/hc/en-us/articles/115002192352-Automated-User-Accounts-Self-Bots`
- **[S3] Discord Terms of Service:** `https://discord.com/terms`
- **[S4] Discord Gateway documentation:** `https://docs.discord.com/developers/events/gateway`
- **[S5] Discord HTTP rate limits:** `https://docs.discord.com/developers/topics/rate-limits`
- **[S6] Discord message resource and nonce behavior:** `https://docs.discord.com/developers/resources/message`
- **[S7] Discord permissions and overwrites:** `https://docs.discord.com/developers/topics/permissions`
- **[S8] Discord voice connections and current DAVE requirement:** `https://docs.discord.com/developers/topics/voice-connections`
- **[S9] eframe NativeOptions and persistence controls:** `https://docs.rs/eframe/latest/eframe/struct.NativeOptions.html`
- **[S10] eframe integration and dependency features:** `https://docs.rs/eframe/latest/eframe/`
- **[S11] egui Context repaint and image-cache APIs:** `https://docs.rs/egui/latest/egui/struct.Context.html`
- **[S12] egui ScrollArea virtualization APIs:** `https://docs.rs/egui/latest/egui/containers/scroll_area/struct.ScrollArea.html`
- **[S13] Discord libdave source and documentation:** `https://github.com/discord/libdave`
