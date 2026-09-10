# Implementation progress — 2026-09-10

## Current scope and gates

Current slice: bounded Gateway compatibility diagnostics from main 31bf546. Full SPEC
completion remains active; native automation and live-account validation remain owner-controlled.
See the Gateway diagnostics entry for verification.

## Inline message spoilers (merged PR #36)

Historical slice: inline message spoilers (SPEC 9.3), based on merged main b92a082 / typing PR #35.
Only marked text regions are concealed; normal surrounding text and ordinary media remain
visible. Up to 32 regions have separate reveal bits, and spoiler-marked media has an independent
explicit reveal. Hidden text is skipped before links, references, emoji and selection rendering.
Original text and explicit whole-message Copy remain unchanged. Exact text/media changes,
deletion and navigation discard reveal consent; complexity-limited input stays conservative.
Reply previews retain conservative concealment; embedded cards retain their existing media gate.

All 78 focused UI tests pass, and `cargo xtask check` passes 287 offline Rust tests,
doctests, formatting, strict all-feature Clippy, text-only compilation and policy checks.
Coverage includes mixed styles/escapes/code/HTML, parser limits, hidden link/reference/image
actions, keyboard and pointer reveal, selection excluding concealed text, independent media
reveal, light/narrow and dark/wide layouts, and consent invalidation. The first focused run
found escaped leading pipes suppressing later valid delimiters; splitting the inert leading
character fixed that case without weakening its regression. Independent final review found
no remaining blocker. Both unsigned Windows packages passed; text and voice executables each
grow by 12,288 bytes. Full package measurements are in docs/performance.md. Original dirty
source hashes remain unchanged. This slice adds no protocol, storage or dependency change.
Native automation remains owner-paused, so native
screenshots, accessibility, resource measurements and live behavior are unverified. No account,
message, audio or native-window actions were performed. The complete SPEC goal remains active.

## Incoming typing indicators (merged PR #35)

Current work: incoming typing indicators (SPEC 8.3/9.2) from merged main dff0975,
after reply-target PR #34. Only service-supplied events for the fresh, readable selected
text conversation can appear. The fixed eight-user state expires after at most ten seconds;
the native composer uses up to three already-retained names and a generic remainder.
No profiles, members or outgoing typing actions are requested for this feature. Existing
People subscriptions remain unchanged, so guild delivery may depend on that subscription.
Off-channel signals are dropped before the desktop queue/repaint; repeated users and bursts
are limited to eight queued typing events per two seconds in a separate eight-slot inbox,
preserving every reliable-event queue slot. A full typing queue drops typing without
turning it into a session failure. Malformed or oversized typing does not interrupt messages.

`cargo xtask check` passed 281 offline Rust tests, doctests, formatting, strict all-feature
Clippy, text-only compilation and policy checks. Regressions cover decoder limits, malformed
typing followed by a message over a local Gateway socket, timestamp skew/expiry, fixed state,
generation and permission gates, no timeline/resident invalidation, queue isolation and
headless narrow/long-name/expiry/no-command rendering. Independent review found and resolved
shared-queue starvation and the two-inbox drain-order race. Both unsigned Windows text/voice
packages passed; executable growth is 26,624 / 22,016 bytes. Package and replay measurements
are recorded in docs/performance.md; the replay comparison was noisy with no speed claim.
The original seven dirty-source hashes were rechecked unchanged. Native automation remains owner-paused;
no account actions, screenshots, native resource measurements or live typing tests were run.
The complete SPEC goal remains open. Per the owner's standing instruction, locally checked
feature PRs are merged to main before continuing; missing native/remote evidence is disclosed.

## Reply-target navigation (merged PR #34)

Current work: reply-target navigation (SPEC 9.2/9.3) from merged main 33181a0.
Reply previews and the composer's View original action jump locally when the target is loaded;
otherwise one bounded history request opens the earlier range. Targets remain same-channel and
permission-gated. Old-range browsing never automatically acknowledges newer messages, and
explicit Reload/Jump to latest returns to recent history. Missing targets remain unavailable.

Explicit service-null referenced messages retain a deleted marker. Validated core admission
produces at most 50 deletion effects for one channel, then the desktop fences disk epochs,
deletes cached target bodies and retires the affected editor. Stale/malformed histories and
uncorrelated send results cannot emit these effects. SQLite schema 10 adds the bounded boolean
while preserving schema-9 message kinds, content markers, reading settings and drafts.

`cargo xtask check` passed: 268 offline tests, strict all-feature Clippy, formatting, text-only
compilation and policy checks. Protocol/store/UI focused checks and the deletion-capacity
regression passed. Independent review found and resolved the full deletion-guard SendResult
edge case; failures now clear the unreliable timeline while preserving drafts and disk effects.
Both `cargo xtask package` and `cargo xtask package-voice` passed. Package sizes and the
five-run release replay comparison are recorded in docs/performance.md. Native desktop automation remains paused after the earlier
owner Escape stop; opening the regular app on request did not run synthetic visual tests or
authorize interacting with that account. No new screenshots, account actions or audio tests
have been performed for this change. Native visual/RSS/IME/screen-reader evidence remains absent.
The original dirty checkout and old unpublished reply worktree are preserved; another open
theme-shell PR (#33) is independent of this branch.

## Prior completed integration

Current work: owner-requested integration of all twelve open implementation PRs (#17, #19,
#21, #22, #23, #24, #26, #27, #28, #29, #31 and #32) with main 85fde15. All twelve heads are
ancestors of published PR #32 at 57927c7. The isolated integration preserves main's image
aspect ratios, anchored profile popouts/profile cache and system-message descriptions alongside
the stack's permission, deletion, resident-history and reading-preference behavior.
Unpublished reply-navigation work stays in its separate dirty worktree and is not included.

Conflicts are resolved with combined schema 9: both independently introduced schema-7 message columns
(system message kind and unsupported-content markers), plus schema-8 reading preferences.
Migration regressions cover both schema-7 histories and schema 8 without losing drafts/settings,
including atomic rollback on failure. Integration review also repaired typed permission changes
leaving profile snapshots/in-flight responses alive, and presence updates leaving old custom-status
text visible. Compact presence updates invalidate that text until a new member-list snapshot.
Combined `cargo xtask check` passed: 248 offline tests, strict all-feature Clippy, formatting,
text-only compilation and policy checks. `node tests/login-handoff.cjs` passed. Both text and
voice release packages passed; measurements are recorded in docs/performance.md. Local `cargo audit --deny warnings` could not run because cargo-audit is not installed;
the unchanged lockfile passed both PR #32 security jobs (runs 34506026424 and 34506044467).
Fresh integration CI is still required for platform evidence. Existing native screenshots remain historical evidence;
no new native/account/audio automation is permitted by this merge request or was performed.

Current work: saved reading and layout preferences (SPEC 2.2 / 9.1). Add application-wide
display scale (80..150%), sidebar width (190..360 points) and wide-layout People visibility,
with reset/retry controls in sign-in and messaging settings. SQLite schema 8 adds one fixed
singleton; account logout preserves these non-account preferences, like the existing theme.
Native notification opt-in and narrow People overlays remain session-only. Delayed hydration
cannot replace user changes; saves coalesce for 300 ms with one write in flight and one latest
value. Failures remain visible and require deliberate retry; pending changes participate in
close confirmation. Entering the in-app preview finishes earlier real changes without saving
preview edits. A standalone --demo still opens no database or account session.

Baseline dfe9e3f (PR #31) text/voice executables were separately copied and hash-verified
before edits; baseline package size reports were preserved. Independent review identified
and fixed the preview-transition pending-save gap. The initial full check exposed sidebar
contents shrinking to the panel minimum; explicitly filling the panel fixed the real geometry
and its regression. Focused reading UI tests and the subsequent cargo xtask check passed,
including 237 offline Rust tests, doctests, formatting, strict all-feature Clippy and text-only
policy checks. Both unsigned Windows packages passed; measured executable/installed/ZIP sizes
and SHA256 hashes are in performance.md. Original seven dirty files remain hash-identical.
Native before/after screenshots, resource measurements and live compatibility remain owner-paused.

Next remaining implementation work includes reply-target navigation; native accessibility,
storage tracing, long-running resource measurements and owner-controlled live gates remain open.

Current work: the SPEC 8.1 in-memory MRU of recently visited conversations. The baseline
State::select drops the sole timeline, so returning to a channel waits for SQLite or the service.
The new slice retains at most two dormant windows, moves them back without cloning, and always
requests service revalidation. Cache hits remain Loading; they do not grant read/notification or
message-action authority. Mutation, identity, permission and lifecycle invalidations must remove
unsafe dormant windows. This restores an immediate preview, not an older-range scroll session.
Desktop SQLite admission uses visible row count, including deleted-only windows, and explicit
cache clear removes dormant history while preserving the conversation already displayed.

Baseline b4c66ac (PR #29) Windows packages were copied and hash-verified. The unchanged reducer
replay and the new navigation harness were built before core edits and copied separately. The
navigation harness uses the baseline API for 10,000 selections over three 50-message conversations;
baseline immediate previews were 0/10,000 while all selections still requested revalidation.
One warmup and five measured runs were recorded. Native automation remains owner-paused.
Windows cargo xtask check passed 228 offline Rust tests, doctests, formatting, strict all-feature
Clippy, text-only compilation and policy checks. Eleven new regressions cover move reuse, request
generations, row/byte eviction, permissions, mutations, archive retirement, deleted-only previews,
SQLite admission, allocation accounting and headless narrow/wide UI. Independent review found
and verified the archive retirement ordering fix and reported no remaining actionable issue.
The changed navigation harness produced 10,000/10,000 immediate previews while retaining all
10,000 revalidation requests, two dormant windows, 150 total rows and 199,654 estimated retained
bytes. Core selection median/p95 was 0.2/0.2 microseconds across five runs versus baseline
median 1.2..1.3 / p95 1.6..1.8 microseconds; these tiny timings exclude native rendering and I/O.
The ordinary 100,000-event replay median changed from 37.7021 to 38.4952 ms (+2.10%) with
overlapping ranges. Both unsigned Windows packages passed; executable, installed and ZIP sizes
and SHA256 hashes are recorded in performance.md. The seven original dirty files remain
hash-identical. Native before/after images and live validation remain pending.

The current source audit also found missing saved reading/layout preferences (only theme persists)
and inert reply previews without explicit target navigation. Those remain subsequent implementation
work, alongside the unresolved native/live, storage tracing and performance acceptance gates.

Current work: visible deletion state (SPEC 9.3) and known-deleted disk-cache safety (SPEC 2.1).
Only an already-loaded message leaves a Message deleted row at its existing ID; live message
lookup remains absent so edit/reaction/media paths cannot treat a placeholder as content.
Author/body/media data is released. Visible rows share the existing item/byte ceilings and
history reconciliation, while unknown deletion IDs remain internal guards. Pagination includes
deleted rows. An unchanged editor closes; modified unsent edit text remains available to copy
or cancel, with saving disabled while the message is absent. Focused session-cache/client-core/UI
tests passed (102 tests and doctests), including headless light/dark and narrow deleted-only views,
reading anchors, payload release, reconciliation limits and all-deleted pagination.
Disk deletion covers inactive/loading conversations and rejects stale queued cache work
independently of timeline freshness. Windows cargo xtask check passed 217 offline Rust tests,
doctests, formatting, strict all-feature Clippy, text-only compilation and policy checks.
SQLite tests cover reopen, transaction rollback, channel/account isolation and preserved drafts;
cache tests cover queue saturation, delayed hydration, stale saves, cross-generation cleanup
acknowledgements and the permanent failure latch. A hard storage failure disables history caching
until restart and reports that content may remain on disk. No restart-erasure guarantee is made.
Independent review found and verified fixes for retained editor undo snapshots and re-editing the
same message; the headless regression verifies actual hover Edit and Undo after deletion.
Deletion events also clean the single retained editor immediately while viewing another channel,
without suppressing that channel's input or sending the old channel's draft on a closing-frame Enter.
Both unsigned Windows release packages passed; measured executable, installed and ZIP sizes
and SHA256 hashes are in performance.md. The seven original dirty files remain hash-identical.
Native before/after images and live validation remain pending.
Baseline e0f18d0 (PR #28) packages were copied/hash-verified. The baseline replay executable
was copied separately and sampled once for warmup plus five measured runs. Native automation
remains owner-paused; this work uses synthetic data and does not open accounts or native apps.
The existing release replay median changed from 36.9497 to 37.0905 ms across five measured runs
each, with overlapping ranges; both retain 500 live records / 220,992..221,477 estimated payload
bytes. This ordinary-message workload does not measure deletion I/O, UI latency or process RSS.

Current work: preserve unsupported-content presence in ordinary message types (SPEC 9.1).
Polls, sticker_items, legacy stickers, components and the Components V2 flag have independently
patchable markers. Missing fields preserve state; explicit null/empty values clear only their
own source. The decoder discards the payload; SQLite schema 7 stores only five presence bits.
Native labels and the existing confirmed Open in Discord fallback accompany supported text/media.
Marker-only replacements invalidate timeline layout, and pending patches reconcile before history
without resurrecting deleted messages. Baseline 8c6976e (PR #27) packages were copied/hash-verified;
one warmup and five baseline reducer replay samples were recorded before edits. Windows
cargo xtask check passed 205 offline Rust tests, doctests, formatting, strict all-feature Clippy,
text-only compilation and policy checks. Seven new tests cover model bit validation/patches,
bounded presence decoding, absent/null/source independence, pending and stale-page reconciliation,
deletions, revision changes, schema migration/cache reopen/corruption/account isolation, and headless
egui layout with labels and explicit fallback. Independent review found no actionable issue.
The synthetic 100,000-event replay median changed from 37.0330 to 38.3804 ms across five measured
runs each, with overlapping ranges; both retain 500 records / 220,992..221,477 estimated bytes.
Both unsigned Windows release packages passed: text executable 49,810,944 bytes (+12,800),
voice 53,163,520 bytes (+12,800). Installed/ZIP measurements and hashes are in performance.md.
The seven original dirty files remain hash-identical. Native automation remains owner-paused; no live service,
browser, microphone or account action is part of this implementation testing.

Current implementation: SPEC 9.1 external fallback for unsupported channel/message content.
Unsupported channel rows gain a keyboard-focusable Open in Discord arrow; message placeholders
gain a labeled button. The existing timeline link confirmation is shared at the messaging-view
level so sidebar actions work without a selected conversation. Destinations use fixed HTTPS
Discord routes and typed IDs, with view-permission/metadata guards; no selection, fetch, call
or browser action occurs before deliberate confirmation. Cancellation/Escape dismiss the modal.
The one bounded pending link resets on logout. This branch starts at 36ab5e7 (PR #26), with
separately copied/hash-verified baseline packages. Native automation remains paused after owner
Escape stops; no browser, account, microphone or OS notification action is authorized for tests.
Windows cargo xtask check passed 198 offline Rust tests, doctests, formatting, strict
all-feature Clippy, text-only compilation and policy checks. Five new tests cover typed routes,
explicit confirmation/cancel/Escape, normalized displayed/emitted destinations, keyboard sidebar
actions, message fallback clicks, permission/metadata guards, logout reset and confirmation
without a selected channel. Review found and fixed Escape reaching background Search/Archives;
a regression test now preserves Search while canceling the foreground confirmation.
PR #26's deterministic Gateway fixture fix is integrated; its repaired macOS PR job passed.
Both unsigned Windows packages passed. Text executable: 49,798,144 bytes (+21,504 / 0.043%);
voice executable: 53,150,720 bytes (+22,016 / 0.041%). Full installed/ZIP sizes and hashes are
in docs/performance.md. Seven original dirty files were hash-verified unchanged. Packages
were measured without launching them. Native/browser/live evidence remains unverified.

Latest implementation: session-only microphone gain and speaker volume (SPEC 11). Both Audio
menu controls range from 0% to 200%, start at 100%, support keyboard input and offer Reset levels.
Changes apply to an active call without reopening devices, survive device/call changes in the
session and reset on logout/preview reset. Two bounded integer atomics feed callback-local gain
snapshots; PCM is finite and clipped. Existing readiness, permission, mute/deafen/PTT and stop
gates retain priority. No devices are opened by settings, and no protocol or persistence changes
are introduced. Already-captured input/resampler data keeps its existing bounded latency.

Baseline packages at 3f9aa0edcd35f82eb83d5576a63d453fc384725c (PR #24) were copied and hash-verified
before edits. The branch also integrates #24's GTK v4_10 CI feature fix; the measured Windows
baseline predates that Linux-only feature/docs adjustment. Original dirty checkout work is kept.
Windows cargo xtask check passed 193 offline Rust tests, doctests, formatting, strict all-feature
Clippy, text-only compilation and policy checks. The three new tests cover keyboard/clamping/reset
and real callback helpers with 0/100/200% gain, clipping, nonfinite PCM, independent runtime
changes, stereo playback and readiness/mute/deafen/stopped-buffer behavior without audio devices.
Independent review found no actionable issue. Native screenshots, real gain perception, callback
timing and live calls remain unverified; desktop automation remains paused after owner Escape stops.
Both Windows release packages passed: text executable 49,776,640 bytes (+119,296 / 0.240%);
voice executable 53,128,704 bytes (+112,640 / 0.212%). Installed/ZIP sizes and executable
hashes are recorded in docs/performance.md. Packages were measured without launching them.

PR #26 macOS CI exposed a pre-existing synthetic Gateway race: dropping TCP immediately
after invalid-session could discard that frame when a heartbeat reply remained unread.
The fixture now injects that heartbeat and waits for the client's post-Resume Disconnected
event before dropping the socket, under the existing 45-second test deadline. The focused
local test passed; no runtime or gain code changed. Repaired CI remains pending.

Linux CI follow-up: both initial builds failed in WebKit6 because GTK4 0.11.4 exports
Accessible only with its v4_10 feature. Enabled that feature and documented GTK >=4.10;
the CI apt log confirms GTK 4.14.5. No source API workaround or dependency version changed.
The repaired PR Linux job 102930632100 passed cargo xtask check, login-handoff tests,
replay and text packaging; voice packaging was still running when recorded. The duplicate
push job remained in progress. Both security jobs passed. This is CI, not live login evidence.

Latest slice: Linux GTK4/WebKit6 authentication migration from 512b5e7 (PR #23), in a new
isolated worktree with separately copied/hash-verified text and voice package baselines.
The temporary Linux login window uses an ephemeral NetworkSession and normal TLS. A protected
top-frame bridge keeps one bounded capability-prefixed candidate; IPC carries only a boolean
wake. One cancellable main-frame query, at least 100 ms apart, rechecks origin and ASCII bounds
before native capability/lifetime validation. Close/drop invalidates late results and clears
secrets/scripts/handler, cancels evaluation and terminates the web process. See authentication.md.

Windows/macOS Wry backend sources remain unchanged in a provenance-preserving fork removing
old Linux dependency declarations. Linux CI now installs GTK4/WebKitGTK 6 development packages.
The strict cargo-audit 0.22.2 gate passes with zero vulnerabilities and warnings, down from the
two Linux warnings at baseline, without suppressions. Cargo.lock drops from 738 to 726 packages.
New component notices are staged in both package variants; system engines remain prerequisites.

Windows cargo xtask check passed 190 offline Rust tests, doctests, formatting, strict all-feature
Clippy, text-only compilation and policy checks. node tests/login-handoff.cjs passes existing
handoff tests plus Linux bridge origin/frame, ASCII bounds, protected one-shot slot, expiry,
navigation invalidation and token-free wake checks. Independent API/security review found one
orphan manifest-table cleanup (fixed) and no remaining actionable issue. Linux's Rust handoff
regression and backend compilation require Linux CI; local WSL lacks GTK4/WebKit6 development
libraries. Native screenshots, X11/Wayland input, actual teardown/storage tracing and live login
remain unverified. Desktop automation remains paused; no native window or account action ran.

Both unsigned Windows packages passed; text executable size is down 1,024 bytes and voice is
unchanged. Eight login notice files are included in each; installed/ZIP totals and limits are
in performance.md. All 45 Wry backend source files match the pinned registry archive byte-for-byte.
The original seven dirty source files were hash-checked unchanged. Linux CI remains pending.

The full SPEC objective remains open. Further implementation gaps found in current sources
include visible deletion state;
native/live acceptance and release evidence remain separate gates. Historical entries follow.

Read the original SPEC.md completely before implementation. Repository initially contained only the tracked two-line README and an untracked SPEC.md; no existing source or agent instructions were removed. The owner explicitly revised authentication and storage during implementation. The final spec now requires the official Discord login in a temporary webview, secure remembered login, and allows bounded local SQLite caches, saved drafts/settings and files. Those changes were applied throughout SPEC.md and AGENTS.md.

| Milestone | Actual status |
|---|---|
| 0 — native shell / feasibility | Native egui/eframe/wgpu app, Cargo workspace, pinned Rust, lockfile, synthetic fixture, real composition/variable-height timeline, compatibility evidence and initial tests implemented. macOS native launch verified. OS credential-store and login-method round trips remain unverified |
| 1 — real normal-user message exchange | **BLOCKED: no owner-controlled authenticated session/private live conversation was supplied or exercised.** Direct REST/Gateway adapters and own-webview credential handoff are implemented, but normal-user acceptance is not established. No real message/reply exchange with an official client is claimed |
| 2 — reliable text | Partial: bounded cache/queues, partial patches, timestamps, deletes/tombstones, late-history reconciliation, session generations, ambiguous-send state, back-pagination, cancellation, heartbeat/finite reconnect/resume, SQLite history/drafts and bounded resident conversation previews. Scoped history failures, page validation/exhaustion, authoritative refresh and a local WebSocket lifecycle test added September 10. Full failure matrix, long process soak and live freshness recovery remain open |
| 3 — everyday messaging | Partial native text UI, server categories/icons, loaded thread/forum-post navigation and archived-thread browsing, bundled Unicode emoji and server emoji picker, grouped timeline and hover actions, native embeds/static images, bounded CommonMark formatting, spoiler concealment, explicit link confirmation, CJK/Arabic fallback fonts, copy/reply/edit/delete controls, clickable user/channel mentions and autocomplete, service profiles, reaction counts/add/remove controls, image viewing and general attachment downloads, single-file picker/drop uploads, conversation search, explicit remote read markers, paginated pinned-message browsing, history clear/logout, saved theme, loaded-user presence, in-app alerts and opt-in native notification adapters. Saved reading/layout preferences passed offline validation; reply-target navigation and actual native IME/screen-reader/notification validation remain open; richer unsupported behaviors are tracked in the capability docs |
| 4 — voice | **Partial; live gate blocked.** Optional one-to-one DM and guild voice UI/signaling, bounded participant rosters, native CPAL/Opus mixed playback and DAVE group encryption implemented. Synthetic crypto/transport/mixer tests pass. No real Discord call, physical microphone/speaker, device-permission or cross-platform audio validation |
| 5 — release | Partial: docs, dual licenses, dependency inventory, xtask, CI matrix and locally ad-hoc-signed macOS package. Inherited dependency graph passed strict security CI on PR #29. Native platform execution, signing/installer work, complete transitive license-text review, storage tracing and performance/platform gates remain open |

This is a runnable native implementation with experimental service adapters, **not a completed Discord replacement**. Offline fixtures and bot behavior do not count as live success. No other application’s credentials, existing browser profiles, account IDs or private history were read. No Discord account action or production message was performed.

## Exact implemented behavior

The login view is Wry with incognito requested, official `https://discord.com/login`, main-frame-only handoff initialization, strict origin checks, per-webview random IPC capability, 2048-byte token limit, one-slot admission, and ten-minute timeout. It observes only that fresh webview’s own authenticated Discord API headers. It does not intercept passwords, implement QR exchanges, inspect localStorage, or bypass challenges. Popup-dependent login methods may be unsupported. The view is dropped on handoff/cancel; REST verifies a non-bot current account before Gateway readiness. Same-account reconnect is enforced. Tokens use OS credential storage; no plaintext fallback.

REST is origin-fixed, uses normal TLS verification, disables redirects/cookies/proxy discovery, caps bodies at 4 MiB, serializes requests and shares service cooldowns conservatively. Writes are not automatically retried. Gateway uses bounded uncompressed JSON, heartbeat/ACK, resume URL validation, finite reconnect attempts and explicit close handling. No bot SDK, bot intents, member scraping, relay or backend exists.

The native core owns one active timeline (500 messages / 4 MiB), 1024 deletion tombstones, 1 MiB pending-patch content and bounded inputs. It discards old-session callbacks, suppresses duplicate confirmations, preserves newer timestamped edits and deletion mutations against late results, and treats cached data as stale until revalidation. UI virtualizes variable-height rows by viewport and retains a message/offset anchor. Long display text is capped, original content retained for copy.

SQLite stores account-isolated draft and history tables, 20 channel windows globally, 48 MiB estimated message content/metadata, a 64 MiB main-database page ceiling, and 64 / 2 MiB drafts. Storage and keyring workers run outside rendering. Cached history clears independently; logout deletes the active account cache/drafts and credential. Errors remain visible. Appearance now persists as a single application-wide SQLite override; System removes it. Window geometry remains session-local.

## September 10 continuation

Implemented three parallel slices and reviewed their integration:

- Reliability: history requests/errors are scoped to channel/request/connection; late pre-disconnect pages cannot mark the view current. Recent-page refresh replaces old records, pagination validates IDs and stops at exhaustion, pending edit timestamps do not regress, and bulk deletions use one bounded UI event. An older window at capacity preserves its reading position; Reload explicitly returns to latest. Resume triggers active history revalidation.
- Persistence: schema-v2 application-wide System/Light/Dark preference, worker startup before authentication, transactional account logout, visible failures, next-account draft hydration, and fixture logout isolation. Viewing an empty composer no longer creates a draft; explicit clearing survives delayed hydration. Exact own-author/channel/nonce confirmation preserves other pending recovery text. Hidden-channel/stale events do not rewrite the active SQLite window, and each event drain queues at most one accepted active snapshot. Replacing a login attempt clears any prior pending credential save.
- Native text: 8192-byte/128-line formatting input, 512 parser events, nesting depth 16, 64-entry/1 MiB estimated parsed/source cache; emphasis, strike, code, quotes, lists, inert HTML, and explicit normalized HTTP(S) destination confirmation. Spoiler syntax conservatively hides the whole message, including literal code syntax; reveal consent expires on actual content change. Deleted anchors move to a surviving neighbor. Height-cache keys cover content/metadata and typography/scale. Edit/delete actions retain their original channel when navigation changes.
- Multilingual assets: two unmodified OFL Noto fallback fonts, 17,312,412 raw bytes (16.51 MiB); license texts packaged. Focused checks cover CJK, Arabic and combining-mark glyphs, not full shaping/IME or regional Han parity. Provenance and the egui glyph-query test limitation are documented in assets/README.md.

Commands actually run on the same macOS arm64 development host:

- `cargo xtask check`: **passed**, formatting, all-target/all-feature Clippy with warnings denied, **18 Rust tests** plus doctests and policy checks. Intermediate runs caught a UI nested-if lint and test-module placement lint; both fixed before this passing run.
- `node tests/login-handoff.cjs`: **passed**, synthetic origin/size/one-shot/fetch/XHR handoff checks.
- The new loopback WebSocket test exercised actual TCP/WebSocket handshakes, Hello/Identify, dispatch-sequence heartbeat/ACK, abrupt drop, Resume, non-resumable invalid session, fresh Identify and terminal 4004 expiry. It took approximately 7 seconds in the final aggregate run. It validates our transport implementation against a synthetic server, not Discord normal-user acceptance.
- SQLite tests cover schema upgrade, appearance reopen/reset, read-only failure, draft-limit rollback and atomic logout rollback in temporary files. Native headless tests cover empty-composer hydration, bounded formatting/link safety, changed-content spoiler invalidation, deleted-anchor fallback and actual font charmaps.

No Discord account was authenticated or messaged. Milestone 1 remains blocked on an explicitly owner-controlled live session; milestone 2/3/5 gates remained partial and voice was still unimplemented at this text-only stage; see the later DM voice continuation below. The final `cargo-audit audit --deny warnings --json` run still exited 1: zero vulnerability-class entries, two warnings (glib unsoundness and proc-macro-error unmaintained). The Linux dependency paths and current upstream migration options are in dependency-audit.md; no warnings are suppressed.

## Initial checks — September 9

- `cargo check --workspace`: passed on macOS arm64.
- `cargo xtask check`: passed after fixing native API changes and restricting dependency metadata to the host target. Runs rustfmt check, Clippy across workspace/all targets/all features with warnings denied, nine Rust tests plus doctests, and persistence/renderer/REST-cookie/ephemeral-webview/dependency-license metadata checks.
- `cargo test --workspace`: nine tests passed. Synthetic coverage includes redaction/header injection, local HTTP redirects/401/429/challenge mapping, Gateway heartbeat/close/origin state, malformed/oversized JSON and ID precision, patch/tombstone/cache revalidation, duplicate confirmation/logout generations, variable-height visible range, and real temporary SQLite reopen/isolation/eviction/logout cleanup. Tests are not a complete failure-mode suite.
- `node tests/login-handoff.cjs`: passed. VM-only synthetic tests cover fetch/XHR handoff, origin exclusion, oversized values, one-shot delivery and non-Discord pages. Node is a development test tool, not the messaging runtime.
- `cargo replay`: 100,000 synthetic reducer events in 21.99 ms; 500 retained messages, 156,992–157,477 estimated timeline bytes in the measured tail. Not a whole-process/UI/transport benchmark.
- `cargo run -p serein -- --demo` and the local macOS app bundle: launched a native window. Computer-use inspection showed accessible controls and a synthetic keyboard-composed message in the timeline. A clipboard automation attempt timed out; later multiline attempts were inconclusive. Missing CJK/Arabic glyphs were visibly observed. Real IME, screen-reader and full keyboard-navigation behavior are unverified.
- `cargo build --release --locked -p serein` and `cargo xtask package`: passed. Unsigned `dist/Serein.app` and compressed macOS arm64 archive produced. Initial executable: 19,346,816 bytes; archive approximately 7.9 MiB. Final size may change slightly with verification fixes; see the final package measurement below.
- Debug synthetic `ps` sample: RSS 118,896 KiB (~116.1 MiB), CPU 0.0%. Hardware/environment and measurement limits are in performance.md. No logged-in idle, GPU, webview-helper, 60 Hz p95 or long process-soak claim.
- `sudo -n fs_usage -w -f filesys -t 3 <synthetic-app-pid>`: **blocked**, OS required a password. No write trace obtained; source policy and SQLite tests do not replace runtime tracing.
- Installed cargo-audit 0.22.2 in the ignored development `target/audit-tool` directory. `cargo-audit audit --deny warnings --json`: **failed** due to the two advisory warnings below; reported vulnerability-list count was zero. No warnings were suppressed, and strict CI remains failing until dependency review/remediation.

- `RUSTSEC-2024-0370` — proc-macro-error 1.0.4: proc-macro-error is unmaintained ([RustSec](https://rustsec.org/advisories/RUSTSEC-2024-0370.html)).
- `RUSTSEC-2024-0429` — glib 0.18.5: Unsoundness in `Iterator` and `DoubleEndedIterator` impls for `glib::VariantStrIter` ([RustSec](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)).

## Evidence and next concrete work

Sources rechecked 2026-09-09: Discord OAuth scopes, self-bot policy, terms, Gateway, messages/rate limits, voice/DAVE and libdave; Abaddon normal-user DTO/Identify evidence; Discord Userdoccers authentication research; resolved eframe 0.36.2/Wry 0.57.0/keyring 4.2.0 APIs. Distinguish official UI/documentation from unofficial normal-user protocol/handoff in discord-compatibility.md.

Next: the owner completes one controlled login in Serein and performs the private official-client message/reply procedure in authentication.md. Diagnose actual normal-user Identify/READY/subscription differences without spoofing or challenge bypass. Then complete live failure/reconnect validation, actual multilingual IME/bidirectional/screen-reader tests, process storage tracing/soak and attachment/reaction/read-state capabilities. Resolve or review the Linux GTK dependency advisories before a supported release, and run the Windows/Linux jobs and native smoke tests. The optional DM voice adapter now needs its separate physical-audio and official-client live gate in voice.md.

## Initial package measurement — September 9

Unsigned macOS arm64 executable: 19,346,816 bytes; app bundle including original licenses/docs: 19,408,341 bytes; ZIP: 8,361,954 bytes. System webview/Metal/credential-store components are external. No signing, notarization, Windows/Linux runtime validation or completed release claim.


## September 10 package and runtime checks

`cargo replay`: 100,000 synthetic reducer events in 19.791708 ms; retained 500 records / 156,992–157,477 estimated timeline bytes. This remains a reducer measurement, not process/frame/live performance.

`cargo xtask package` builds the default locked release. The initial staged executable was killed by macOS with exit 137; the source executable launched, and signature verification identified a malformed staged bundle. Packaging now replaces the executable through a fresh sibling inode, then seals the completed macOS bundle with local ad-hoc `codesign --force --sign -` and verifies it strictly. Two consecutive package runs, executable-inode replacement, `codesign --verify --strict --verbose=2 dist/Serein.app`, Info.plist validation and targeted xtask Clippy passed. The resulting executable is 36,933,088 bytes (about 35.2 MiB), including the two embedded fallback fonts. This is not Developer ID signing or notarization.

The release source executable was launched in fixture mode, then that test window was closed when the owner asked for login/real data. `target/release/serein` was launched normally and computer-use inspection verified the visible native sign-in screen, unchecked owner-session acknowledgement and Sign in with Discord button. Saved-login lookup reported no existing saved session. The normal window is left available for the owner to operate; the agent did not select the acknowledgement, submit credentials, open a real conversation or send a message. The final signed bundle has strict static verification; a separate post-fix bundle launch is still unverified because the owner-facing normal window was left undisturbed.

Final macOS artifact measurement: app bundle 37,026,503 bytes; `dist/Serein-macos-arm64.zip` 22,533,682 bytes (~21.5 MiB). These figures were measured after archiving; the bundled report precedes this measurement line.


## Interface design continuation — September 10

Implemented a shared graphite/teal and warm-light design, responsive sign-in composition, clearer guild/channel navigation, account Settings footer, roomy composer and compact per-message action menus. Existing auth/storage/transport behavior remains. Offline preview stays explicitly labeled; it cannot authenticate. Fixed unbounded row-header height discovered during the redesign, sidebar selection on initial channel, theme restoration after logout memory reset, and header alignment. Details and measured palette contrast are in design.md.

Native macOS screenshots checked wide/narrow layouts and both themes; synthetic Settings/theme and message-menu/Reply interactions were exercised. Authentication was not performed. The original owner-facing login window was not restarted. Full workspace checks passed all 18 offline Rust tests and Clippy; final package validation follows below. Screen-reader, actual IME, minimum-height and other-platform visual checks remain unverified.

Final design checks: `cargo xtask check` passed (18 tests, Clippy, formatting and policy); `cargo xtask package` passed with strict local ad-hoc signature verification. Updated executable: 37,084,416 bytes. The temporary sign-in layout window was closed; the conversation preview remains available.

## September 10 continuation — People and profile pictures

Implemented native People/member pane, basic profile cards and circular pictures in the timeline, account footer, DM navigation and member rows. DM recipients survive READY and incremental recipient add/remove updates. Guild lists use on-demand unofficial opcode 14 subscriptions for the first 100 service list positions, including group rows; indexed SYNC/INSERT/UPDATE/DELETE/INVALIDATE events are bounded to 128 KiB, scoped by service list ID and current request, and canceled/reissued across navigation and reconnect. Missing permission metadata or a 15-second missing/unsupported response is visible as unavailable. No full-directory collection, membership database or bot request was introduced.

Avatar IDs/hashes/discriminators flow from actual wire DTOs into the SQLite v3 history cache. The credential-free static-PNG worker has per-account disk caching (1 GiB / 4096 files / 90 days), one bounded decode/download at a time, and a completion fence before replacement or cache deletion. Visible textures are capped at 64 / 4 MiB. Clear cached history also clears pictures; explicit logout clears the account’s picture cache even after expiry or an in-progress worker shutdown. The existing PNG dependency is reused, with no additional codec or bundled bitmap assets. SPEC.md records the owner’s preference for disk caching over minimizing disk use, while RAM/package limits retain priority.

Validation: `cargo xtask check` passed formatting, workspace/all-target/all-feature strict Clippy, all **26 Rust tests**, doctests and runtime feature policy. New checks cover member list wire/index operations and real loopback subscription/unsubscribe, wrong-list/request/permission rejection, DM recipient changes, static avatar validation/oversized PNG dimensions, declared and chunked response limits, no credential headers, redirect denial and 429 cooldown, real cache reopen/account isolation/retention/item+byte eviction, cancellation while output is full, texture eviction and member-row virtualization. SQLite testing now migrates an actual old message schema and preserves avatar metadata on reload. All network fixtures were local and synthetic.

A macOS native synthetic preview was inspected at approximately 1088px width: People rows, avatars and account/timeline layout rendered correctly, with accessible profile actions. Further profile/narrow-window interaction checks were interrupted by concurrent user interaction and closure of that preview; these were not claimed completed. No production Discord account or CDN image was fetched by the agent. Owner-controlled live message/member/avatar validation remains blocked by the absent authorized test session; M1 is still incomplete. Windows/Linux runtime validation and existing GTK dependency-audit blockers remain open.

macOS arm64 release packaging passed with strict ad-hoc signature verification. Executable: **37,443,712 bytes** (35.71 MiB), up **359,296 bytes** (0.97%) from the preceding UI build. No Developer ID/notarization or Windows/Linux packaging validation is claimed.

## September 10 continuation — optional DM audio calls

Implemented the `voice` feature for existing one-to-one DMs, with native Start/Answer/Decline/Hangup controls, participant state, mute/deafen, session-local device selection and focused V push-to-talk. Default builds remain text-only. The adapter uses Discord's main Gateway call signaling, a separate voice WebSocket/UDP path, bundled Opus, CPAL and Davey/OpenMLS for DAVE v1; there is no project voice service. Voice identities, credentials and audio are session-only, and cryptographic tracing is compiled out.

Ringing is an explicit once-per-call REST command after transport allocation; it does not race the initial join and does not wait for a peer-dependent encrypted group. Answer never rings. A departure barrier waits for the owner's null state or CALL_DELETE before another join; a ten-second missing acknowledgment is visible and cannot retag old credentials/events onto a new request. Rejected Join/Ring commands fail immediately, while failed remote mute signaling preserves local mute. Slow REST writes run through a bounded serial worker so call controls remain responsive.

The media adapter validates voice destinations, performs UDP discovery, authenticates RTP, encodes/decodes Opus and gates media on required DAVE group transitions. Unsupported encryption, extra participants and failed resumption stop the call. Two bounded voice WebSocket resumes preserve the call's existing UDP/MLS state; main Gateway disconnect requires deliberate rejoin. Device initialization follows encrypted readiness, and teardown/focused push-to-talk are polled outside rendering so a hidden window does not retain a stale microphone gate. There is no AEC, global push-to-talk, automatic device fallback, recording, group-DM/guild calling, video or screen sharing.

Validation: `cargo xtask check` passed formatting, workspace/all-target/all-feature Clippy with warnings denied, **41 Rust tests**, doctests and policy checks. Synthetic coverage includes the core call intent/generation/queue rules, local HTTP ring/decline routes, main Gateway op13/op4 join/leave and departure correlation, real two-party MLS/DAVE transitions and authenticated encrypted media across local WebSocket/UDP, Opus, RTP tamper/replay boundaries, jitter/loss handling, bounded voice socket resume, and device-free capture/resampling gates. No production Discord traffic or hardware audio was used for these tests.

`cargo run --locked --features voice` selects the optional media build; `cargo xtask package-voice` stages it separately under `dist/voice`. Source builds add CMake for static libopus and Linux ALSA development headers. Package/performance measurements are reported separately when actually obtained. Physical microphones/speakers, OS permissions, sound quality, echo, device loss, suspension, resource cleanup under real calling, and Windows/Linux execution remain unverified. No owner-controlled live Discord session was exercised, so both the real text exchange gate and official-client two-way voice gate remain blocked. See [voice scope and manual procedure](voice.md) and [adapter details](../crates/discord-voice/README.md).

The voice dependency audit found an active affected libcrux SHAKE dependency beneath HPKE. The minimal MPL-2.0 source patch in `vendor/hpke-rs` replaces only its fixed-size SHAKE adapter/dependency with RustCrypto sha3 0.10.9. Independent 32/64-byte output vectors and the full encrypted transport checks pass. The voice package includes the corresponding modified component source and license. All six vulnerability-class findings now belong to unused optional packages in Cargo.lock; the strict lockfile audit still fails and warnings remain enabled. See [the detailed audit](dependency-audit.md).

Both macOS arm64 build variants packaged and passed strict ad-hoc verification: text executable 37,634,384 bytes; voice 40,491,296 bytes. The voice release launched in offline demo with DM controls and disabled microphone access; the validation session was closed. See [performance evidence](performance.md).

## September 10 continuation — server categories, embeds and icons

Implemented ordered, collapsible server categories from channel `parent_id`/`position`, with live create/update/delete reducers, selected-channel visibility and keyboard-accessible category controls. Orphaned channels remain reachable. Unsupported voice/stage/forum channels retain explicit disabled labels; this does not expand the preceding DM-only calling scope.

Native embed cards now render bounded descriptions, titles, authors, fields, footer/timestamp, thumbnails and static images. Partial embed patches, suppression and edits during history loading flow through the existing reducer/cache. SQLite schema 4 preserves existing history and drafts while storing embed attributes. Spoiler concealment includes embed text and images; embed-only changes revoke reveal consent. Links retain explicit confirmation, and video cards offer a browser action. A regression check covers natural card height at the bottom of a small viewport.

Server icons use validated Discord CDN paths and the existing account image cache. Both flat guild objects and the unofficial normal-user nested `properties` shape are decoded; guild updates preserve omitted fields and invalidate changed icon keys. Embed media shares the credential-free, origin-restricted worker. PNG conversion is unofficial and live-unverified. Disk allowance remains 1 GiB / 4096 files / 90 days; the shared texture payload limit is now 16 MiB / 64 entries, with two decoded-result slots. No new codec, bundled bitmap or locked dependency package was added. See [categories](categories.md), [embeds](embeds.md), [icons](icons.md) and the updated [storage policy](storage-policy.md).

No Discord account, production message, CDN image or physical audio device was accessed for this change. The owner-controlled live text gate remains blocked by the absent authorized test session. These implementations and synthetic checks do not establish normal-user service acceptance, actual image-proxy conversion or Windows/Linux runtime behavior.

Validation: `cargo xtask check` passed formatting, workspace/all-target/all-feature Clippy with warnings denied, **50 Rust tests**, doctests, the no-default-features build and runtime policy checks. Added coverage includes category ordering/collapse and channel moves, guild metadata patches, bounded embed decoding/patch reconciliation, real SQLite upgrade/reopen, image URL/cache limits and native presentation. An initially flaky new loopback category test was corrected to keep its synthetic server socket alive until the client observed terminal close; repeated forced-heartbeat race checks passed before the final aggregate run.

The voice-enabled macOS release was inspected in `--demo` at approximately 1088×768. Generated server icons, category arrows and the full embed card/image rendered; category collapse hid siblings while preserving the selected channel, and clicking the synthetic embed title displayed the expected destination-confirmation dialog. The dialog was canceled without opening a browser, and the preview was closed. Headless coverage additionally checks narrow/short viewport card sizing and spoiler reveal invalidation. Neither this preview nor the loopback fixtures used a real Discord account.

Both macOS arm64 variants packaged with strict local ad-hoc signature verification: text **37,972,480 bytes**, voice **40,844,544 bytes**. The voice package retains the preceding DM-call implementation. No Developer ID/notarization, live service or Windows/Linux verification is claimed. The synthetic JavaScript authentication-handoff checks also passed; the dependency-audit blockers recorded earlier remain unresolved.

## September 10 continuation — clickable inline Markdown links

Replaced the message/embedded-text renderer's detached numbered link buttons with native inline links. Markdown link labels retain emphasis/strike/code styling and target their declared destination; bare HTTP(S) URLs are colored and clickable with surrounding punctuation excluded and balanced URL parentheses preserved. Hover shows the normalized target, and keyboard Tab/Enter uses the existing external-destination confirmation. Code, image syntax, unsafe URLs and hidden spoilers do not automatically open content. Message and embed descriptions/field values share this renderer. Adjacent parser text events are merged with the existing pulldown-cmark utility before URL recognition, preventing truncated destinations at literal Markdown punctuation; the existing input/cache bounds and 512 merged-event limit remain.

Validation: `cargo xtask check` passed strict Clippy, formatting, **51 Rust tests**, doctests, the no-default-features build and runtime policy. The new regression test checks styled masked links, complete bare URLs with parentheses, inert code/unsafe link labels, and keyboard activation of separate destinations in a narrow viewport, without emitting an external-open command. Existing spoiler and embed tests still pass. The renderer also restores the link accessibility role after egui's selectable-text handling and avoids allocating button-height padding around text.

The rebuilt macOS voice variant was checked in offline preview: inline Markdown labels and bare URLs rendered alongside italic text, without detached link buttons or extra button-height spacing. Pointer clicks on each displayed the correct distinct normalized destination in the confirmation dialog; both dialogs were canceled and no browser/network action was invoked. Accessibility inspection exposed Link roles, while keyboard activation was tested headlessly; a complete screen-reader workflow remains unverified. The preview was closed. No production Discord validation was performed.

Both final macOS arm64 packages passed strict local ad-hoc signature verification: text **37,988,976 bytes**, voice **40,844,624 bytes**. Existing DM voice support is retained.

## September 10 continuation — chat image attachments

Incoming attachment metadata now reaches native chat rendering from history, creates and partial updates. Image attachments show cached inline previews with a click-to-enlarge native viewer, available alt text, dimensions and an explicit original-file browser action. The viewer fits the viewport, scrolls long metadata and closes on Escape, deletion, navigation or invalidated spoiler consent. Recognized raster types use the existing static PNG service-proxy path; non-image files retain an honest unavailable-preview label. Uploading, animated playback and native save-as remain outside this viewing change.

Schema 5 preserves earlier messages, embeds and drafts while storing bounded attachment JSON. Reconciliation includes attachment-only revisions, null/empty removals, late-history protection, pending-patch bytes and layout/reveal invalidation. Hidden attachments do not request media before reveal; documented spoiler metadata and conservatively handled unofficial sensitive flags are covered. No new decoder dependency, bitmap asset, independent storage service or image-memory allowance was introduced. The shared 16 MiB / 64 texture and 1 GiB / 4096 disk-item limits remain. See [image attachment scope, limits and sources](chat-images.md).

Validation: `cargo xtask check` passed formatting, strict workspace/all-target/all-feature Clippy, **55 Rust tests**, doctests, the no-default-features build and runtime policy. New checks exercise JPEG/PNG metadata, nullable dimensions, MIME-versus-extension classification, spoiler/sensitive flags, oversized arrays/strings, attachment-only updates and deletion during history loading, actual schema-4 upgrade/reopen/account separation, and bounded cache JSON. UI checks verify that embed suppression does not suppress attachments, hidden attachments queue no media request, reveal permits preview, and subsequent attachment edits revoke reveal and close the viewer. Existing image URL/decoder/cache and link-confirmation checks still pass.

No owner-controlled Discord session, production image or microphone was accessed. Real image delivery/conversion, expired signed-URL recovery and Windows/Linux presentation remain unverified. The live text and voice milestone gates remain blocked on the absent authorized test session; these fixtures do not count as live service success.

Native macOS validation used the voice release in offline demo: the inline attachment and filename rendered, clicking the image opened a larger native viewer with alt text/dimensions/size, and Open original showed the exact synthetic CDN destination for confirmation. That dialog was canceled without browser/network access; Escape dismissed the viewer and the preview session was closed. Both final macOS arm64 packages passed strict local ad-hoc signature verification: text **38,030,064 bytes**, voice **40,902,080 bytes**. The voice build retains the previous DM-call implementation. No Developer ID/notarization or Windows/Linux runtime claim is made.

## September 10 continuation — mentions, large images and service profiles

Added @ autocomplete over already loaded users, keyboard insertion without sending, and clickable incoming mentions that open profiles. Exact user notification allowlists preserve disabled role/everyone/reply notifications. Mention metadata follows history, patches and SQLite schema 6; migrations preserve existing cached messages/drafts. [Scope and checks](mentions.md).

Image attachments open in a large native overlay with Close/Escape and Download controls. The native Save As flow streams original CDN attachment bytes off the render thread with progress, cancellation and a 100 MiB cap. Local HTTP/filesystem checks exercise successful output, truncated/oversized transfers, existing-file preservation, atomic publication, stalled-body cancellation and cleanup failure reporting. Download is deliberately disabled for synthetic demo URLs. [Limits](chat-images.md).

Replaced the basic profile window with a banner/avatar card and on-demand real profile adapter. Available bio, pronouns, badge descriptions, connections, mutual servers and guild-specific fields are rendered; unavailable service data is not replaced by fixture data. Profile metadata remains one bounded RAM result; banner/avatar pixels use the shared cache. A separate cancellable profile task and four REST permits prevent a slow profile body from blocking an admitted message write. [Unofficial protocol evidence and offline checks](profiles.md).

Workspace checks pass, including 70 offline tests, strict all-feature Clippy, formatting and policy checks. Native macOS arm64 offline preview verifies clickable mention → profile, @Ro → Robin suggestion, Enter insertion of `<@2>` without sending, large image layout/Close, and profile detail scrolling/Close. A regression test verifies profile scope follows the open DM/server conversation while another server is browsed in the sidebar. No authenticated Discord request or message was performed. Real profile/media delivery, mention notification behavior, native Save As dialog interaction, Windows/Linux presentation and full-resolution/animated viewing remain unverified or outside this slice. Offline evidence does not satisfy the live Discord interoperability gate.

Both final macOS arm64 variants packaged with updated documentation and native-dialog license texts. Strict local ad-hoc signature and archive CRC/content checks pass. The rerun dependency audit retains the existing six advisories/five warnings; no new finding names rfd or pollster. See [performance sizes](performance.md) and [audit details](dependency-audit.md).

## September 10 continuation — idea-to-PR agent workflow

Adapted the working method and Git/PR conventions from the owner's AICaller instructions for
Serein's Rust/native boundaries. `AGENTS.md` now directs feature requests through implementation,
focused verification, synthetic native before/after screenshots, relevant release performance
comparisons, task-only commits, push and PR creation. Native demo verification is authorized by
default; live Discord actions retain the explicit owner gate. Dirty work stays preserved, blocked
checks/evidence produce a draft, and merging/release publication is not automatic. Added the
repository delivery skill, PR template and contributor link; no runtime code/dependencies changed.

Installed optional `gh-fix-ci` and `gh-address-comments` locally from
[OpenAI's curated skills](https://github.com/openai/skills/tree/49f948faa9258a0c61caceaf225e179651397431/skills/.curated),
pinned to that source revision. These user-local installations are not vendored in the repository;
the committed delivery skill and AGENTS instructions work without them. Existing Ponytail and
visual-design-polish skills are reused when available. Skill guidance must respect the owner's
standing task authorization and runtime tool permissions. No extra approval loop, global Git
configuration, background agent, paid service or fixed delivery-time guarantee was introduced.

Validation: `cargo xtask check` passed (70 offline tests, strict Clippy, formatting and policy).
The skill creator's `quick_validate.py` passed for all three skills using an isolated `uv --with
pyyaml` tool environment; the system Python lacked PyYAML. `git diff --check` passed. Independent
read-only scenario review found no blocker for dirty UI work, docs-only work with existing CI
failures, or unavailable screenshot export. Native screenshots and runtime benchmarks are not
applicable to this instructions/template-only change; no feature-level/live claims were added.

Baseline `778b71a` is current `origin/main`. Its
[native-client workflow](https://github.com/ViceVerse-cz/rustcord/actions/runs/34423149411)
passed macOS, Windows and Linux native jobs but failed `cargo audit --deny warnings` in the security
job before this task. This setup is delivered as a draft PR with that inherited blocker, without
suppressing audit checks or expanding into dependency repairs.

## September 10 continuation - message reactions

Implemented native reaction counts, selected own-reaction buttons, an eight-emoji picker, and adding/removing normal reactions using the existing authenticated REST adapter. Existing custom emoji can be toggled by name; deleted custom emoji remain visible and disabled. Counts include super reactions, but creating/removing super reactions, animated emoji, a full emoji browser and reactor-user lists are not implemented.

All four Gateway reaction events invalidate only the affected active message. One cancellable message readback runs at a time, repeated invalidations coalesce in a bounded set, and a result superseded by another event is discarded. This deliberately trades one bounded message GET for reliable counts without maintaining per-user reaction membership. Only loaded messages are fetched; events arriving during history loading invalidate matching eventual records. Navigation, permission loss, deletion and session generations reject stale results. A separate read task preserves the existing serial write worker's responsiveness; existing REST permits, cooldowns and deadlines apply. A failed/ambiguous write is never automatically repeated. Failed readback leaves an explicit Reload reactions control.

Reaction lists are capped at 64 entries with 128-byte emoji names and count toward existing timeline/event/patch budgets. Counts are session-only: the existing SQLite schema remains unchanged, cached messages expose unknown reactions until service history/readback refreshes them, and no cache hit authorizes a reaction. No dependencies or emoji assets were added. Demo controls modify synthetic RAM only.

Validation on Windows 11 x64: `cargo xtask check` passed formatting, strict all-feature Clippy, **74 offline Rust tests**, doctests, the text-only build and runtime policy. Added checks cover bounded/nullable/duplicate reaction decoding, percent-encoded Unicode/custom emoji HTTP paths, correct GET identity, all four reaction events over a local WebSocket, coalescing, stale readback/history, deletion, queue rejection, ambiguous writes, 429 and permission-loss behavior, and keyboard activation/disabled controls at a narrow viewport. Existing SQLite checks confirm reactions are not restored as fresh cached counts. No Discord account action was performed. Windows native visual inspection remains blocked by the unavailable Computer Use pipe; headless interaction is not a screen-reader or live-service claim.

Next feature work at that point: attachment uploads or remote read markers (read markers implemented below). Owner-controlled normal-user text/reaction and two-way audio validation remains a separate blocked milestone. The earlier Windows preparation also fixed reserved superscript device names in Save As suggestions, with regression coverage based on [Microsoft's filename rules](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file). Before reactions were added, both unsigned Windows packages built and created responsive native windows in synthetic process checks; those startup checks do not establish this new feature's native visual behavior.

`cargo xtask package` then passed for the reaction-enabled Windows text build: `dist/serein.exe`, **41,648,640 bytes**, unsigned. The previously staged `dist/voice/serein.exe` predates reactions; the voice-enabled source passed the all-feature checks but that artifact was not repackaged for this slice.


## September 10 continuation - remote read markers

Implemented unread channel/DM badges and an explicit **Mark read through here** action in each message menu. Viewing, selecting or scrolling a conversation does not acknowledge it. Only a loaded message in a fresh authenticated view can initiate the action, with one pending write across conversations. The offline demo includes synthetic unread state and applies its actions only in RAM.

The adapter consumes bounded READY read-state snapshots, MESSAGE_ACK and PASSIVE_UPDATE_V2 latest-message updates, and sends a scoped acknowledgement through the existing REST write worker. Newer service updates, including manual mark-unread from another client, supersede delayed HTTP results. Navigation preserves the original operation's scope; disconnect, permission invalidation and removed conversations cancel pending local results. Queue rejection and uncertain failures never advance the local marker or automatically repeat the write. Missing service read state stays unknown; badges express a boolean, not an invented unread/mention count. No bulk or automatic acknowledgements, outgoing mark-unread, new dependencies or database migration were added. [Wire evidence and bounds](discord-compatibility.md).

Validation on Windows: `cargo xtask check` passed formatting, strict all-target/all-feature Clippy, **77 offline Rust tests**, doctests, the no-default-features build and runtime policy. New regression checks cover bounded snapshots, zero/invalid cursors, partial latest-message updates, exact HTTP acknowledgement bodies and session-token chaining, Gateway dispatch, explicit-action gating, version/race handling, unknown snapshots, request identity after reset, queue rejection, navigation, disconnection, logout and conversation removal. These tests use synthetic data and local HTTP/WebSocket servers. No Discord account action was performed. Live cross-device read behavior remains unverified; native visual/screen-reader inspection remains unavailable because the Computer Use pipe failed earlier in this session.

`cargo xtask package` passed for the read-marker-enabled Windows text build: `dist/serein.exe`, **41,708,032 bytes**, unsigned. The staged voice executable was not rebuilt in this slice; the voice-feature source passed the all-feature checks.


## September 10 continuation - conversation search

Implemented native search within the current guild text conversation or existing DM. The Search window supports explicit button/keyboard submission, newest and older result pages, bounded author/text snippets, service-reported totals and partial-index status. Opening a result reloads its history and positions the timeline at the requested message; missing results receive a visible status. The existing Reload control returns to latest history. The demo searches only its generated synthetic messages, without network access or storage.

One cancellable search task uses the existing authenticated REST permits, cooldowns and deadlines without blocking the message-write worker. Guild searches always include the current channel filter; DMs use the channel route. Queries are encoded as data. Indexing responses expose an explicit retry message and respect the service delay without automatic polling. Requests/results are tied to the session, conversation and request ID. Navigation, disconnect, permissions and relevant edits/deletes invalidate old snapshots. Search snippets never hydrate the timeline or SQLite; opening a result uses normal history reconciliation. Spoiler snippets stay concealed, with no media loading or automatic external actions. Search retains at most 25 result summaries / 64 KiB, with 256-character queries and 512 KiB response bodies. No dependencies or database schema changes were added. See [compatibility evidence](discord-compatibility.md), [memory bounds](architecture.md) and [storage behavior](storage-policy.md).

Validation: `cargo xtask check` passed formatting, strict workspace/all-target/all-feature Clippy, **81 offline Rust tests**, doctests, the text-only build and runtime policy. New checks cover guild/DM HTTP scope, query encoding, older-page cursors, indexing without automatic retry, permission errors, malformed/missing/duplicate/oversized results, snippet limits and spoiler concealment, stale query results, mutation/navigation/logout invalidation, queue rejection, history revalidation, keyboard submission and IME commit suppression. Existing render tests also caught and prevented a needless command on initial display. A full-check attempt initially ran out of disk space in the Windows linker; package-scoped `cargo clean --profile dev` removed regenerable artifacts and the subsequent full check passed.

No authenticated Discord search or external-account action was performed. Current Discord acceptance, search-index completeness, actual IME/screen-reader use and native visual interaction remain unverified. Global/advanced search and attachment uploads remain incomplete; owner-controlled live text/voice gates remain blocked on the absent authorized test session. Offline fixtures are not live Discord evidence.


## September 10 follow-up - saved-login startup status

Fixed a definite stale-status bug: `Loaded(Ok(Some(secret)))` started the Discord connection without replacing the initial Checking saved login label. The UI now distinguishes found, absent, invalid, unavailable and timed-out lookup outcomes, and displays the result even before authentication begins. Startup queue failure and a disconnected credential worker are handled explicitly. A single delayed repaint enforces a 10-second UI lookup deadline without idle polling. Late loaded credentials are discarded after timeout, manual sign-in, preview, connection replacement or logout; they cannot unexpectedly start a second session.

The OS credential call remains on the one existing serial worker; timing out stops waiting in the UI, not the synchronous OS call. No new worker retry, credential deletion, plaintext fallback, token logging or reading of another application's store was added. A persistently blocked OS backend itself remains an external limitation, and this change does not prove an existing saved credential is accepted by Discord.

`cargo xtask check` passed again with **82 offline Rust tests**, formatting, strict all-feature Clippy, doctests, the text-only build and runtime policy. The new synthetic-channel test covers absent/found/invalid status, bounded waiting, worker failure and discarded late/cancelled results without opening the OS store. No real credential or authenticated login was inspected. The preceding search-only Windows text package also built successfully (41,810,432 bytes); it is superseded by the combined build below.

The combined search and saved-login fix passed `cargo xtask package`: unsigned Windows text executable `dist/serein.exe`, **41,810,944 bytes**. Release linking initially also exhausted disk space; another package-scoped debug-artifact cleanup allowed packaging to finish. No source files or credential-store entries were removed by cleanup. The staged voice executable was not rebuilt; current voice-feature source passed the all-feature checks.

## September 10 follow-up - unsupported login response

Reproduced a login regression offline: a synthetic READY carrying the legacy read_state array failed with DecodeError because the new read-marker parser accepted only a versioned object, while Identify never requested the versioned_read_states capability. The parser now accepts both shapes with the same 4000-entry bound. Versioned partial snapshots preserve unknown state for omitted channels. No Identify capabilities or authentication behavior were broadened.

Account verification, Gateway discovery, READY decoding, resume-address rejection and remaining Gateway protocol failures now carry static stage labels instead of the single Unsupported service response label. Existing expiry, challenge, permission, rate-limit and capacity outcomes remain intact. Raw payloads, account details, credentials and service error bodies are never incorporated into these labels.

Validation: the legacy READY regression failed before the parser correction and passed afterward. `cargo xtask check` passed formatting, strict all-target/all-feature Clippy, **83 offline Rust tests**, doctests, the text-only build and runtime policy. Coverage includes both read-state shapes and oversized lists, partial-state semantics, legacy READY through the local WebSocket connection, malformed account responses with private synthetic fields, HTTP rejection and preserved expiry/challenge/rate-limit categories. No real credential or Discord account request was used. This establishes a concrete compatibility defect, not confirmation that it was the owner's exact failure; another rejection will now identify its stage. Live login and native visual inspection remain unverified.

`cargo xtask package` passed for the corrected unsigned Windows text executable, `dist/serein.exe`, **41,813,504 bytes**. The staged voice executable was not rebuilt; voice-feature source passed the all-feature checks. Package-scoped cleanup removed only regenerable debug build artifacts before release linking.

Known owner-reported issue before main synchronization: reacting can make the chat view lose its messages. Investigation has not yet established the cause, and the offline reaction tests do not cover the owner's failure. The next implementation task is to reproduce and fix that path before adding further features.

Main synchronization: incorporated the incoming repository workflow commits through e9fb3e4, preserving both progress histories. The integrated tree passed `cargo xtask check` again (83 offline Rust tests and all configured checks) and `node tests/login-handoff.cjs`. No reaction-crash fix or new live-validation claim is included in this synchronization.

## September 10 continuation - reaction refresh and pinned-message browsing

Baseline: clean main at 74c0d79, Rust 1.98.1, Windows 11 x64; work is on feat/pinned-messages-and-reaction-fix. Fixed the reaction refresh path's single-message GET by using bounded history-around lookup for the exact message. The old endpoint's rejection flowed into real channel-permission invalidation, which explains how a reaction could empty the chat. Fixed-array decoding rejects absent/deleted targets, wrong messages/channels, excess records and object-shaped replies. Genuine history-access rejection still clears inaccessible content and cached history. Successful reaction readback preserves message text and freshness. The owner said the current build works, without a scoped live-test trace; this is not treated as verification of the new change.

Added a native Pins control and a manual snapshot of up to 25 newest pins, in service pin order. The panel supports explicit Reload, Close/Escape and Open message, clearly reports omitted older pins, conceals spoiler previews and revalidates actual history before displaying a selected message. Search and pins share one cancellable request task and one bounded result slot; late, wrong-mode/channel and queue-rejected responses are covered. No pin/unpin, automatic acknowledgement, pin polling, new dependency or database migration was added. Existing group-DM text handling was inspected and already exists; this slice makes no new group-DM or voice-support claim. See compatibility and storage notes for wire evidence and bounds.

Validation: focused reaction HTTP/reducer tests passed, followed by `cargo xtask check` with **86 offline Rust tests**, formatting, strict all-target/all-feature Clippy, doctests, text-only build and runtime policy. Added pin tests cover the current route, forbidden response, bounded parsing, duplicates, pin order, spoiler concealment, request/mode/scope isolation, history opening and keyboard Reload/Escape at 420x480 in light and dark themes. Independent code review found no blocking issue. These are synthetic checks, not Discord-account validation.

Native baseline evidence: rebuilt the baseline text and voice packages, captured the text --demo window and clicked its synthetic reaction from 3 to 4 while messages stayed visible. That demo bypasses the HTTP adapter and did not reproduce the real-account failure. Before the after-change inspection, Computer Use reported the owner's physical Escape stop; desktop automation stopped immediately and no further desktop interaction was attempted. The after screenshot, changed-build native long-content inspection and comparable after process-memory sample remain unavailable. Delivery stays a draft pending those checks. Development screenshots are excluded from installed packages.

Next unfinished spec slice: bounded user-selected attachment uploads; older-pin pagination and pin/unpin remain outside this first browsing slice. Owner-controlled live text/reaction/pins and two-way voice gates remain unverified.

Both corrected Windows packages passed: text `dist/serein.exe` **41,835,008 bytes**, voice `dist/voice/serein.exe` **45,182,976 bytes**, unsigned. The packaging check verified neither package contains docs/pr-evidence; package-scoped debug cleanup freed space before linking and removed no source or credential data. Five-run release reducer medians were 26.2139 ms before / 26.3168 ms after, with identical bounded retained timeline range; see performance.md for package sizes, sampling and limitations. The baseline main security job failed before this change with six cargo-audit vulnerabilities and five denied warnings; dependencies and audit policy remain unchanged. No release was published or main branch merged for this slice.

## September 10 continuation - single-file attachment uploads

Baseline: clean e99bc8817454335789b84140c2998394bca899cf on the preceding Pins/reaction branch. Work is isolated on feat/attachment-uploads, stacked on that branch without merging main. The parent's Windows, macOS and Linux native CI jobs passed; its security job still failed with the inherited six vulnerabilities and five denied warnings.

Implemented Attach file using the existing native picker, one nonempty regular file up to 20,000,000 bytes, selected filename/size and Remove, streamed progress and Cancel. Selection is local until Send. Attachment-only messages and text/reply attachments use the existing pending nonce and confirmation path. The upload worker negotiates Discord staging, streams 64 KiB chunks with a separate credential-free client to the exact validated storage host, then posts the message. Selection is scoped to conversation/session; navigation, disconnect and logout cancel pending work. One slot stays occupied until a cancelled chooser/worker finishes. Paths and signed URLs never enter persisted drafts or diagnostics; failed attachment recovery keeps text and requires reselection.

Cancellation before the final message POST means no message was sent; cancellation after it starts is ambiguous and never triggers automatic retry. Staged bytes can remain remotely even when no message is created. Missing or observably changed files fail explicitly, without a hidden recovery copy. Metadata checks are not immutable snapshots. Fixed the shared send-error reducer so nonterminal failures and queue rejection preserve a healthy conversation's freshness, including a newly selected channel; terminal authentication failures still invalidate the session. Offline preview can select/remove a local file but attachment Send is disabled and explained.

Validation: `cargo xtask check` passed formatting, strict all-target/all-feature Clippy, **95 offline Rust tests**, doctests, text-only checks and runtime policy. New coverage exercises real loopback HTTP staging/PUT/message creation, credential isolation, file bounds and changes, redirects, cancellation during PUT and final POST, pending reconciliation and filename budgets, healthy-channel rejection handling, chooser/upload slot retention and attachment-only keyboard sends. Independent review found no blocking issue. An intermediate existing image-cache test failed with Windows StorageFull; package-scoped generated debug cleanup and CARGO_INCREMENTAL=0 allowed the full rerun to pass. Source and credential data were untouched.

Both unsigned Windows packages passed: text **42,109,440 bytes**, voice **45,423,616 bytes**. Added native tokio-util's original MIT license is present in both packages; no PR screenshots are bundled. Five-run release reducer medians were 26.8048 ms before / 26.5324 ms after with unchanged retained bounds; this small difference is noise. Package and process measurement details are in performance.md.

Native baseline --demo screenshot and process samples were captured from the verified baseline executable. Computer Use then reported the owner's physical Escape stop, and no further desktop-control tools were called. After screenshot, native picker/cancel and changed-build light/dark/narrow checks, and comparable process samples remain blocked; delivery is a draft. No live Discord upload, normal-user service acceptance, cross-platform native picker behavior or physical audio was exercised. Multiple files, drop selection, larger account-tier limits and remaining spec milestones are still open. This fixture and test suite are not proof of a working Discord client.

PR CI caught a macOS-only test-fixture race: Tokio write_all can finish before the background file write is visible to a separate metadata read. The shared synthetic upload-file helper now flushes before returning, fixing all three affected tests at their common source. This is test-only and does not change the measured release code. Native/security CI on the corrected head must be checked separately from the passing local run; the PR remains a draft for the evidence gaps above and inherited audit failures.

## September 10 continuation - attachment drag-and-drop

Continued the existing attachment PR from clean b83b41a. One native file dropped into the active, fresh conversation now enters the same asynchronous selection/validation slot as the picker. Exactly one absolute local path within 4096 encoded bytes is admitted; an existing selection or busy operation is preserved. Dropped handles are moved out of egui input once and their whole-file bytes API is never called. Selection does not send anything. Conversation/session changes discard late results; login, leave confirmation, editing and active download flows reject drops with a visible explanation. A composer hover hint explains the one-file/20 MB cap and Send boundary. No dependency, protocol, cache or database change was introduced.

`cargo xtask check` passed with **96 offline Rust tests**, formatting, strict all-target/all-feature Clippy, doctests, text-only checks and policy. The added synthetic handle test covers success, multiple/unsupported/oversized paths, existing/busy selection rejection, wrong-channel source access and late navigation cancellation; its whole-file reader panics if called. Independent read-only review found no blocker. Native default drop registration was checked in the pinned winit/egui sources; actual OS drag/drop remains unverified.

The previous file-picker build's native --demo composer was inspected during this continuation. Computer Use then reported another physical Escape stop on the attempted chooser click, and desktop interaction stopped. No final drop-build screenshot, picker/drop execution, light/dark/narrow interaction or comparable after process sample is claimed. The macOS and Linux push CI jobs for b83b41a passed; that revision's security job still reported the inherited six vulnerabilities and five denied warnings. New-head CI and native evidence remain separate gates; PR #7 stays a draft.

Both updated unsigned Windows packages passed: text **42,137,600 bytes** (+28,160 over picker-only), voice **45,450,240 bytes** (+26,624). Full package measurements and a fresh run of the unchanged reducer workload are recorded in performance.md. These package builds are local verification, not native drop or live Discord evidence.

## September 10 continuation - channel references

Started from clean attachment branch head 14dd066 on feat/channel-references. The composer now offers up to eight locally loaded same-server text/announcement/thread channels after `#`, using the existing mention menu and keyboard/IME handling. Selection inserts `<#id>` without sending. Message-body references to loaded server text channels open normal channel navigation/history; unknown or unsupported targets remain inert. Embed channel references remain literal. User/channel links share the 100-reference cap, and no directory request, notification expansion, dependency or storage migration was added. See [mention behavior and wire source](mentions.md).

Fixed source-provenance handling in the shared formatter: escaped/entity-expanded text remains inert even when an identical raw reference appears later. A focused regression first exposed an escape-offset case and passed after correction. Loaded channel name/removal changes invalidate cached offscreen row heights while retaining normal scroll anchoring. The synthetic demo's message 500 now includes a channel reference; it is generated data, not live Discord content.

Independent code review found no blocker. Native desktop automation remains paused following the owner's repeated physical Escape stops in the preceding task; no before/after screenshot, changed-build native keyboard/screen-reader, theme/narrow-window inspection or comparable process-memory/CPU sample is claimed. Delivery remains a draft for those evidence gaps. No live Discord action or microphone use was performed, and synthetic checks do not prove normal-user interoperability.

Validation: `cargo xtask check` passed with **99 offline Rust tests**, doctests, strict all-target/all-feature Clippy, formatting, the no-default-features build and runtime policy. The 23 UI tests include channel autocomplete/Enter behavior, literal parsing/keyboard link activation and offscreen rename invalidation. Earlier attempts hit disk exhaustion in the existing 1 GiB cache fixture and one existing local Gateway test timeout; the Gateway test passed its focused recheck and the complete suite passed after removing regenerable package artifacts/debug symbols. No test limits were weakened and no unrelated source was changed.

Both unsigned Windows release packages passed: text **42,149,888 bytes**, voice **45,462,528 bytes**, each +12,288 bytes over 14dd066. Full installed/ZIP comparisons and the fresh synthetic reducer benchmark are recorded in [performance.md](performance.md). Replay retained the same 500 records and estimated byte range; the small timing increase is noisy and does not measure UI behavior. Parent PR #7's native Windows/macOS/Linux CI jobs all pass; its security audit still fails with six inherited vulnerabilities and five denied warnings. The new branch's CI is separate from those parent results.

## September 10 continuation - general attachment downloads

Started from clean ae360f3 (channel-reference PR #9) on feat/file-attachment-downloads. Non-image attachments now offer native Save As through the existing one-job download worker; PDFs, archives and unknown MIME types are preserved as bytes without decoding or execution. The same exact CDN/attachment-ID/signed-query checks, credential isolation, nonzero 100 MiB cap, size validation, bounded streaming, cancellation and atomic destination publication apply to images and other files. Filename sanitization preserves ordinary extensions and uses an extensionless fallback. No dependency, protocol route, cache or database migration was added. [Attachment behavior](chat-images.md) and storage/compatibility docs were updated.

Download progress/cancellation are visible in the channel header as well as the image viewer. The timeline previously reset these controls on navigation while its worker kept running; navigation now retains the active controls and pending cancel request. One UI request slot prevents duplicate selection during a frame. Spoiler reveal remains required before attachment actions appear. The synthetic demo's message 500 gained a text-file attachment with downloads disabled; no real file or message content was embedded.

Validation: `cargo xtask check` passed **101 offline Rust tests**, doctests, formatting, strict all-target/all-feature Clippy, the text-only build and runtime policy. The existing local HTTP test now verifies non-image admission and exact binary bytes alongside prior redirect, overflow/truncation, cancellation, destination-race and cleanup cases. New synthetic egui tests verify keyboard Download/Cancel, demo/busy/pending rejection, absence of automatic image/external actions and navigation continuity. A final test-only fixture-ID correction and explicit platform-command assertion passed its focused recheck. Independent read-only review found no blocker. To keep space for the existing 1 GiB disk-cache test, tests were compiled first and regenerable debug symbols removed before running the complete check; no test limit or runtime profile changed.

Native desktop automation remains paused after the owner's repeated physical Escape stops. Before/after screenshots, actual Save dialogs, native keyboard/screen-reader/theme/narrow-window checks and comparable process samples remain unavailable, so delivery is a draft. No live CDN/account action or microphone use was performed; offline tests do not establish Discord interoperability. Parent #9's security job reports the same six inherited vulnerabilities and five denied warnings; its native jobs and this branch's new CI remain separate from local evidence.

Both release packages passed: text **42,150,400 bytes** (+512), voice **45,463,552 bytes** (+1,024). Installed/ZIP measurements and fresh reducer samples are in [performance.md](performance.md); retained state is unchanged, and timing differences are noisy. Parent #9's Windows push job failed in the unchanged image-cache test waiting five seconds for worker shutdown/cleanup (`avatars.rs:681`, Timeout); that test passed locally in this slice. This parent CI failure and the inherited audit findings are recorded as blockers, not waived. New-head CI must be checked separately.

## September 10 continuation - loaded threads and forum posts

Started from clean 985bfc6 (file-download PR #10) on feat/thread-navigation. READY guild threads and THREAD_CREATE/UPDATE/DELETE/LIST_SYNC/MEMBERS_UPDATE now populate and reconcile bounded navigation. Scoped snapshots preserve unrelated guilds/parents and validate before mutation; archive/delete/owner removal invalidates selected history/search and rejects late pages while preserving drafts. The sidebar groups loaded text/announcement threads and forum/media posts, retaining the selected thread and parent when a category collapses. Malformed/orphan hierarchy falls back to root rows without recursion. Existing text history/composer behavior handles selection. No new dependency, REST endpoint, thread join/create action or subscription expansion was added; unknown-thread updates await a later create/snapshot/READY. See categories.md for the partial discovery boundary and primary wire sources in discord-compatibility.md.

Independent review caught account-wide history clears on ignored thread events; cache invalidation now requires an actually removed thread after reduction. Empty-scope, unknown-guild, mismatched/nonthread removal and rejected snapshots preserve navigation/cache eligibility. Actual removals reuse the conservative account-wide history clear and retain drafts. The synthetic demo adds one forum post and one text-channel thread; it is generated data, not live Discord content.

Validation: cargo xtask check passed with **106 offline Rust tests**, doctests, formatting, strict all-target/all-feature Clippy, text-only build and runtime policy. Tests cover READY merge/collisions, bounded wire lists, omitted versus empty scope, malformed/cross-guild snapshots, archive/delete/member events through a loopback Gateway, late history rejection, draft preservation, hierarchy/cycles/collapse and headless keyboard post selection. A task-scoped test-only repair separates 4096-file eviction/deletion from the image worker's five-second shutdown assertion, retaining the capacity and backpressure checks; this addresses the parent Windows CI fixture timeout without changing production behavior or increasing deadlines. A local rerun initially exhausted disk in the existing 1 GiB cache fixture; the full check passed after removing only regenerable debug symbols.

Native desktop automation remains paused after the owner's repeated physical Escape stops. Before/after screenshots, native keyboard/screen-reader/theme/narrow-window inspection and comparable process CPU/memory samples remain unavailable, so delivery remains a draft. No authenticated Discord action or microphone use was performed. Archive browsing and thread create/join remain unfinished, as do the owner-controlled live text/voice gates; offline fixtures do not establish Discord interoperability. Parent PR #10 currently has passing Linux jobs, pending Windows/macOS jobs and failed inherited security checks; this branch's CI must be assessed separately.

Both unsigned release packages passed: text **42,330,624 bytes** (+180,224 / 0.428%), voice **45,643,264 bytes** (+179,712 / 0.395%). Installed/ZIP measurements and five-run replay samples are in performance.md. Replay retained the same 500-message estimated byte range; the median increased 1.8137 ms with overlapping sample spread, without a native performance claim. Independent review confirmed the final cache invalidation correction. Parent #10's completed security job was inspected directly and reports the inherited six vulnerabilities and five denied warnings; dependencies and audit policy are unchanged. Next concrete thread work is a bounded archive/directory view opened by the user; live interoperability and native evidence remain separate gates.

## September 10 continuation - older pinned-message pages

Started from clean 188c555 (thread-navigation PR #12) on feat/pins-pagination. The existing Pins panel now supports explicit Older pins and Retry older pins; Reload returns to the newest page. Pagination uses the last service pin timestamp, preserving nanoseconds and pin order independently of message creation IDs. The parser rejects invalid/unsorted timestamps, nonprogressing pages, wrong channels, duplicate messages and oversized results. A has_more response requires a usable next cursor. One 25-item / 64 KiB page replaces the previous page; no background paging, accumulation, automatic retry, pin mutation or pin acknowledgement was added.

Core request/channel/session guards prevent late pages replacing a newer request or another conversation, stop duplicate loading requests and retain the failed older cursor for deliberate retry. Exhaustion removes the older action; Open message still revalidates normal history. The explicitly labeled demo now supplies two synthetic pages. Compatibility/storage docs describe the current timestamp-cursor contract and its live-unverified status. Existing time 0.3.55 gains formatting support; Cargo.lock, package versions and license requirements are unchanged.

Validation: cargo xtask check passed **106 offline Rust tests**, doctests, formatting, strict all-target/all-feature Clippy, text-only build and runtime policy. Existing protocol/HTTP tests now cover cursor precision and exact encoded request paths, older pins with newer message IDs, malformed/repeated cursors, exhaustion and invalid-cursor rejection before network I/O. Reducer checks cover replacement, retry after queue rejection, stale results, reloading newest and no further request after exhaustion. The existing 420x480 light/dark headless keyboard check exercises Older/Retry, loading, unavailable and exhausted states. Independent read-only review found no blocker. Tests were compiled first, then regenerable debug symbols removed before the existing 1 GiB cache fixture; package-scoped cleanup preserved source, profiles and the copied baseline.

Native automation remains paused following the owner's repeated physical Escape stops. Native before/after images, actual keyboard/screen-reader/theme/narrow-window inspection and comparable process CPU/memory samples remain unavailable, so delivery remains a draft. No live Discord/account request or microphone action occurred; synthetic fixtures are not interoperability evidence. Parent PR #12's security job was inspected and reports the inherited six vulnerabilities and five denied warnings; native jobs were pending at that check. This branch's new CI must be assessed separately. Broader thread discovery, pin mutations, notification/presence completion and remaining platform/live gates are still open under the full specification.

Both unsigned release packages passed: text **42,341,888 bytes** (+11,264 / 0.027%), voice **45,655,040 bytes** (+11,776 / 0.026%). Installed/ZIP totals and fresh five-run reducer samples are in performance.md. Retained replay state is unchanged; the median increased 0.7021 ms with overlapping samples, without a native performance claim. Parent #12 now has passing PR macOS/Linux and push Linux jobs; Windows and the duplicate push macOS job remain pending, with security failing as documented. The new pin-pagination head requires its own CI. Next concrete feature work remains bounded thread archive/directory navigation, alongside the separately blocked native/live gates; full-spec completion is not claimed.

## September 10 continuation - archived-thread browsing

Started from clean 293c72b (pin-pagination PR #13) on feat/thread-archives. Native Archive actions now browse public, joined-private and private archived-thread pages for eligible loaded parents. Reload, Older, Retry, empty/loading/exhausted states and explicit Open history use the existing cancellable read worker. Page replacement retains at most 25 summaries / 64 KiB from a 512 KiB response; cursor/type/guild/parent/ordering validation rejects malformed results. No archive join/reopen/create mutation, background polling, new dependency or schema change was added.

Open admits one transient thread within existing account navigation limits. Normal navigation retires it while preserving drafts; matching Gateway data can adopt it, and active-list omission alone does not revoke it. Explicit thread removal and metadata updates invalidate affected archive snapshots even before a result enters navigation. Request/session guards reject late responses. Parent revocation and normal history failure safeguards remain active. The demo supplies explicitly synthetic archive pages, and the header identifies archive origin without claiming current remote archive status.

Validation: cargo xtask check passed formatting, strict all-target/all-feature Clippy, **111 offline Rust tests**, doctests, text-only checks and runtime policy. New tests cover all three loopback HTTP routes, timestamp/ID cursor encoding, invalid/oversized pages, permission errors, retry/replacement, transient bounds/adoption/removal, stale responses and 420x480 light/dark headless keyboard navigation. Independent review found a stale unloaded-row deletion case; it was fixed with regression coverage and the complete check rerun. Initial Clippy test-module ordering was also corrected. Disk pressure required a separate build/temporary directory on E:; packaging now respects CARGO_TARGET_DIR instead of copying a possibly stale executable from the default target directory.

Native automation remains paused following the owner's repeated physical Escape stops. Required native before/after images, actual keyboard/screen-reader/theme/narrow inspection and comparable native CPU/memory samples remain draft blockers. No live Discord/account or microphone action occurred. Parent PR #13's inspected security job reports the inherited six vulnerabilities and five denied warnings; dependencies and audit policy are unchanged. Full active-thread discovery, create/join controls, notification/presence completion and remaining platform/live specification gates are still open.

Both unsigned release packages passed: text **42,437,632 bytes** (+95,744 / 0.226%), voice **45,750,784 bytes** (+95,744 / 0.210%). Full installed/ZIP totals and both five-run replay comparisons are in performance.md. The initial replay median increased 10.96%; a warmed, alternating-order comparison did not reproduce it (-2.47% with overlapping samples), so neither a speed improvement nor an established regression is claimed. Retained timeline bounds are unchanged. Required native evidence and inherited security failures keep delivery draft; full specification completion is not claimed.

## September 10 continuation — chat timeline delivery

Resumed the uncommitted `feat/chat-timeline` slice from e9fb3e4, preserving its work before
integrating origin/main 74c0d79. The new main already implemented service read markers, reactions,
search and login fixes; the final change reuses them and removes the redundant session-local
read-state implementation. No automatic read ACK is sent by scrolling.

Added grouped author rows, UTC timestamps/day separators, service unread dividers, bounded loaded
reply previews, scroll-triggered history paging and explicit jump-to-latest behavior. Older-page
retention now begins at request admission, protecting the reading window against live arrivals
even when history fails. The existing history/reaction/search/authentication paths remain.
Independent review found a hidden edited-label regression in grouped rows; edited messages now
start a full row, with a regression check. See [scope and reproduction](chat-timeline.md).

`cargo xtask check` passed: formatting, strict all-target/all-feature Clippy, **86 offline Rust
tests**, doctests, the text-only build and policy checks. `node tests/login-handoff.cjs` passed.
Focused UI/core/cache/fixture tests also passed. No dependency or schema changes were needed.

Native screenshot/interaction evidence is blocked by `Sky Computer Use native pipe startup
failed`, repeated after resetting the tool session. The obsolete before-only screenshot is
preserved in ignored local artifacts rather than offered as a current comparison. Headless
layout checks are not native visual or accessibility acceptance. This delivery remains a draft.
Main's [CI run](https://github.com/ViceVerse-cz/rustcord/actions/runs/34462932351) passed all three
native jobs but failed security audit with six vulnerabilities and five denied warnings before
this task; no check was suppressed. The earlier owner-reported reaction disappearance and live
text/voice gates remain unresolved. Performance/package results are recorded below and in
[performance](performance.md).

Both final macOS arm64 package variants passed strict local ad-hoc signature verification,
and their compressed archives passed CRC checks. Text executable: **38,622,688 bytes**
(+18,176); voice: **41,464,160 bytes** (+18,192), against the rebuilt 74c0d79 baseline.
Five-run median reducer time: **27.005 → 27.224 ms** (+0.81%, noise-sized); retained timeline
range is unchanged. Ten launch-only offline idle samples gave settled text RSS
**113,568 → 113,712 KiB**, with median CPU 0.0% in both. Native interaction, display scale,
GPU/helper memory and frame latency remain unmeasured; details and full package sizes are
in the performance report. No release publication, real account messaging or microphone
operation was performed.

### September 10, 2026 — bundled Twemoji

Implemented Twitter/contributor Twemoji 17.0.3 artwork for formatted message text,
Unicode reaction counts and the existing eight-choice reaction menu. All 4,009 assets
are bundled in one fixed atlas; no runtime CDN requests or new resolved Cargo packages.
Complete grapheme matching handles skin tones, flags, keycaps and ZWJ combinations,
normalizing emoji presentation selectors without partially matching unknown sequences.
Code and explicit text-presentation stay literal. Message source, whole-message Copy,
composer input and protocol payloads are unchanged. Custom server emoji/animation and
color glyphs inside the editable composer remain outside this artwork change.
Drag-selection excludes inline image widgets; use the existing message Copy action to
copy complete original text. CC BY 4.0 attribution and full license ship in both packages.

Baseline: clean `main` at `f708cb21fcefa741ce294afb49f1d34211a8160e`, fetched/up to date;
task branch `feat/twemoji`. Native macOS offline baseline and changed builds used the same
synthetic message, viewport and dark appearance. Inspected both light/dark, narrow layout,
inline sequences, and adding a reaction in the offline fixture. Before/after evidence is
in `docs/pr-evidence/twemoji`. No Discord messages, calls or microphone tests were performed.
Focused tests cover complete sequence lookup, atlas/index bounds, code exclusion, real
image rendering and keyboard reaction actions with the installed atlas. Full checks and
both release packages are recorded in this task's performance/PR evidence. Windows/Linux,
live compatibility, screen-reader output and frame/startup p95 remain unverified.

Verification: `cargo test -p ui emoji` passed 2 focused tests; final `cargo xtask check`
passed all 88 workspace tests, both strict Clippy configurations, format and policy checks.
`cargo xtask package` and `cargo xtask package-voice` passed including strict ad-hoc
codesign verification. The generator reproduced the atlas/index from a hash-verified
upstream archive. Staged diff review passed except preserved upstream license whitespace.
Text executable +6,170,256 bytes; compressed .app +6,075,785 bytes; median sampled RSS
+16,256 KiB. See `docs/performance.md` for full results and measurement limits.

Delivery: [PR #6](https://github.com/ViceVerse-cz/rustcord/pull/6) is a draft while
GitHub's macOS/Windows/Linux native jobs and security job are pending. Local checks
and both host packages passed; pending CI is not reported as success.

### September 10, 2026 — custom server emoji and composer picker

Follow-up to Twemoji: receive bounded guild emoji catalogs over READY/GUILD_CREATE and
GUILD_EMOJIS_UPDATE; render standard/static custom emoji in formatted text and reactions
through the existing credential-free media worker; show a searchable 3,953-entry standard
palette and current-server picker in the composer. Emoji selection inserts/replaces at the
saved cursor without sending and respects message/draft capacity limits. Unknown/restricted
catalog eligibility is visibly disabled. Two explicitly synthetic server emoji demonstrate
the same insertion/rendering path offline; animated markup uses a still preview.

Fixed inline selection/copy for both Unicode and custom image widgets. Original text stays
in egui's selection model, with atomic emoji hit targets; dragging across images and beginning
inside an image cannot copy a partial sequence/markup. Right-click Copy emoji is also available.
Code/unknown sequences and the editable composer remain literal. No new runtime dependency,
account action, schema migration, cross-server entitlement claim or live Discord test.

Baseline `a270437cb6c1e69030420cda4e0fb6efa957bcb0` on clean `feat/twemoji`, fetched from origin;
new task branch `feat/server-emoji-picker` is stacked on PR #6. Baseline packages reuse the
previously verified exact runtime binaries (compared byte-for-byte); their packaged docs
predate the previous final measurement report. A detached baseline source worktree supplies
the reducer replay. Synthetic screenshots are in `docs/pr-evidence/server-emoji-picker`.
Inherited CI blocker: PR #6's native jobs passed on macOS/Windows/Linux; its security job
fails `cargo audit --deny warnings` with six vulnerabilities and five denied warnings
(run 34466516925). This change does not modify Cargo.lock or those dependencies.

Verification: final `cargo xtask check` passed 98 workspace unit tests, formatting,
policy checks and default/all-feature strict Clippy. Both `cargo xtask package` and
`cargo xtask package-voice` passed with signature verification. Pointer-event tests
copy exact Unicode/ZWJ/custom markup across image labels and forbid partial emoji
endpoints; picker tests cover filtering, replacement budgets, reset and viewport media
requests. Native before/after screenshots show the same synthetic message; custom
images and Emoji trigger are visible in the changed build. Native automation returned
unchanged UI state on repeated AX/coordinate picker activation (and intermittent
user-changed-state errors), including after resetting the session with only the final
preview running. Full native picker/clipboard/light/narrow checks remain incomplete;
unit selection output is not claimed as native clipboard verification.

Measured text executable +224,240 bytes (+0.50%); compressed package +65,520 bytes
(+0.22%); median RSS +21,600 KiB (+18.82%), with unequal extra picker activation attempts.
Reducer median 27.093 → 26.771 ms is a noisy difference. See `docs/performance.md` for
methods, full package comparisons and limits. Draft delivery is required for the native
evidence gap and inherited dependency audit failure; no live Discord test was performed.

Delivery: [PR #8](https://github.com/ViceVerse-cz/rustcord/pull/8), stacked on #6,
contains the implementation and verified commit-pinned before/after images. New macOS,
Windows, Linux and security checks are pending at handoff; local success does not
represent CI success. The PR remains draft for the evidence/audit blockers above.

## September 10, 2026 — message hover controls

Implemented `feat/message-hover` from clean `main` at `d802b2a` (fetched origin/main;
Rust 1.98.1). Consecutive same-author messages hide continuation times until hover/focus,
share aligned content, retain edited labels and highlight with the existing theme surface.
The upper-right hover toolbar provides reactions, reply, own-message edit and More;
copy/delete/mark-read stay in the existing menu. Empty reaction rows take no space.
Keyboard focus and open menus retain the toolbar without reflowing message content.
No dependencies, network commands, storage policy or cache budgets changed.

Verification: `cargo xtask check` passed workspace tests, formatting, strict Clippy and
policy checks. A new offline egui check covers hidden/revealed timestamps, edited grouped
messages, unchanged row heights, pointer reply and keyboard reply at 900-point dark and
360-point light widths. Existing reaction toggle/disabled and virtualizer tests pass.
Native macOS synthetic screenshots compare the unchanged `--demo --demo-chat` fixture
at the top of history: `docs/pr-evidence/message-hover/before.png` and `after.png`.
`own-message.png` shows the additional Edit control. Native checks verified reply context,
adding/removing a reaction without losing other messages, the edit dialog/cancel, More menu
and light-theme hover contrast. Window resize automation did not alter native dimensions;
narrow and long-text layout is covered by the 360-point offline egui test.
Release text and optional-voice packages are built locally; measurements are recorded in
`docs/performance.md`. Screenshot evidence proves only offline presentation, not live
Discord compatibility. Windows/Linux and screen-reader verification remain unperformed.

Delivery: [PR #11](https://github.com/ViceVerse-cz/rustcord/pull/11), draft while macOS,
Windows, Linux and security CI checks are pending. Implementation/evidence commit
`7cb25ba`; commit-pinned screenshot paths verified on origin. Local checks and both
release packages passed; remote CI is not yet a success claim.


## September 10, 2026 — server voice channels

Implemented `feat/guild-voice` from clean `main` at `619071c`, after fetching origin/main
with Rust 1.98.1. Existing server voice channels are selectable and show bounded gateway
participant rosters with avatars/names, separate mute/deafen indicators, a connected
channel timer, explicit Join/Leave and existing audio/PTT controls. Browsing never starts
a call. Group media extends the existing optional engine: guild-scoped signaling and
Identify/Resume, DAVE membership transitions, empty-room waiting with devices off, up to
64 participants, independent SSRC decoders/jitter buffers, mixed playback, server mute/
deafen enforcement, and teardown on permission/removal/session changes. DMs keep their
existing peer restriction/ringing. No dependencies, backend, voice-key storage or recordings.

Verification: `cargo xtask check` passed formatting, strict workspace Clippy, **111 tests**
(plus one deliberately ignored performance workload), text-only compilation and policy
checks. Focused checks cover guild signaling/correlation/departure, initial/supplemental/
passive roster hydration, bounds and permission invalidation, 3-party MLS join/remove and
unauthorized/duplicate identity rejection, real local WebSocket/UDP encrypted mixed audio,
voice resumption, empty-room silence, and native-device-free teardown. The ignored release
mix benchmark was run separately and passed. Both `cargo xtask package` and
`cargo xtask package-voice` built and verified local ad-hoc signatures.

Native macOS evidence: `docs/pr-evidence/guild-voice/before.png` and `after.png` use the
unchanged standard offline fixture and show the disabled voice row becoming selectable.
`roster.png` and `roster-light.png` use the disclosed additional `--demo --demo-voice`
fixture; all identities, status flags and elapsed time are synthetic. Inspected dark/light
rosters, long-name truncation, separate status icons, empty-room navigation, return to text
and call-bar retention while browsing another room. Native resize/drag automation did not
alter the window/sidebar size; 190-point narrow rows, keyboard join and bounded viewport
rendering passed offline egui tests. Screen readers and Windows/Linux native behavior were
not exercised. No Discord call, microphone or speaker test was performed.

Measured voice executable +184,896 bytes (+0.39%), voice ZIP +72,507 bytes (+0.24%);
settled demo RSS +4,672 KiB with the comparison limitations in `docs/performance.md`.
Both sampled idle CPU medians were 0%. New 63-remote-speaker synthetic mixing measured
1.002 ms per 20 ms tick; this excludes encryption/network/hardware and is not live latency.
Reducer median changed 26.700→27.443 ms (small noisy slowdown).

`cargo-audit audit --deny warnings` was re-run and still exits 1 with the same six
vulnerabilities and five warnings documented in `docs/dependency-audit.md`; this task
changes neither dependencies nor Cargo.lock. Draft delivery is required for that inherited
audit failure and the unperformed owner-controlled official-client two-way/multi-party
voice gate. Current normal-user roster/signaling behavior remains unofficial/live-unverified;
Stage/group DMs/video/screensharing, acoustic echo cancellation, automatic region/move
rejoin and global push-to-talk are not implemented. Live validation procedure is in
`docs/voice.md`; implementation, fixtures and a connected label do not pass milestone 4.


Delivery: [PR #14](https://github.com/ViceVerse-cz/rustcord/pull/14), draft. Implementation
and inspected native evidence are committed at `0a07e38`; all four commit-pinned screenshot
paths and the PR body were verified on origin. macOS, Windows, Linux and security CI are
pending at handoff. Local checks passed; remote CI success and live audio compatibility
are not claimed. The inherited strict audit failure and owner-operated live voice gate
remain the draft blockers described above.


## Server people subscription repair — September 10, 2026

Baseline: clean `main` at `dc48391`, matching fetched `origin/main`; task branch
`fix/server-people-list`, pinned Rust 1.98.1. The owner's already-open app showed
an unavailable people pane despite a nonzero server total. Only its visible UI was
inspected and its existing Reload people control retried; no credentials, messages,
calls, microphone, captured account payloads or live screenshots were used in artifacts.

The active member request now uses opcode 37 and enables the prerequisite guild
subscription (`typing:true`). Closing or changing the pane clears channel ranges and
that subscription. It still requests just 100 list positions and retains the existing
128-KiB member bound; no role UI or full-directory fetch was added. Role colors,
role headings and member role display are not implemented. See compatibility notes
for the dated primary implementation evidence and remaining unofficial behavior.

Validation: `cargo test --locked -p discord-gateway member_tests` passed both tests;
`cargo xtask check` passed workspace tests, formatting, strict Clippy and policy checks.
An independent read-only review found no blocker in the final request/cleanup change.
Native screenshots: not applicable to this wire-request-only patch; native layout and
synthetic people rendering are unchanged, and identical demo pictures would not show
whether Discord accepts a subscription. The repaired build remains live-unverified.
Package and reducer comparison results are recorded in `docs/performance.md`.

Both text and optional voice release packages built and passed strict local signature
verification. Executables each changed by -32 bytes; installed bundles by +2,963 bytes.
Synthetic reducer median 26.474→27.128 ms, with the same retained timeline range;
this workload does not exercise the changed subscription. No performance improvement
is claimed. Delivery remains draft pending CI and repaired-build live verification.


## Composer, shared editing and notifications — September 10, 2026

Clean baseline `efa724b` on `main`, fetched from existing origin; task branch
`feat/composer-notifications`, pinned Rust 1.98.1. Known mentions now display as
`@name` in the native text editor, with bundled Twemoji and static server artwork.
Wire text remains the draft/send/copy format. Message editing uses the same composer;
ordinary unsent drafts remain separate, cancel restores them, and confirmation cannot
discard input typed in the same frame. No separate edit dialog remains.

Incoming DMs get avatar shortcuts with red counts. Servers/channels show unread dots
and mention badges, with focused latest-view ACKs and replay/self/history deduplication.
System notifications are session-opt-in, generic-content-only and gated by known
normal-user mute/DND preferences. Generation changes cancel pending alerts; logout
requests dismissal. Unknown settings fail closed. The synthetic history generator now
returns the latest advertised fixture message, so DM badge-clearing is testable offline.

`cargo xtask check` passed formatting, workspace checks, 123 Rust tests (one existing
ignored performance test), strict Clippy and policy checks. Focused tests cover native
copy/undo/IME/atomic token movement, edit cancellation/retry/ack races, read-state ordering,
new-DM first-message handling, zero-entry pruning, queue/item/byte bounds, and mute/DND
filters. Independent reviews found and fixed stale-alert invalidation, historical-view
suppression and a notification-map admission issue. Native testing also exposed the
macOS library's unreliable blocking run-loop check; the worker now awaits its async
show API without blocking rendering.

Native screenshot evidence lives under `docs/pr-evidence/composer-notifications`.
Composer/edit comparisons use the same offline chat fixture and viewport; badges use
a separately labelled incoming-DM/server-mention fixture. OS alert tests require an
explicit additional demo flag and never contact Discord.

Both release packages built successfully. macOS permission and actual generic
notification delivery were observed in Notification Center; disabling returned
the app to off. OS-side dismissal was not conclusively verified because the
notification window was no longer accessible to automation. Opening the synthetic unread
DM cleared its rail badge and left the unrelated server mention badge intact.
Dark and light layouts were inspected; minimum-width resize could not be verified
with the native automation tool (window/resize errors). Shared editing was exercised
natively through Save, with the original unsent draft restored.

Validation incident: the UI tool auto-launched a closed temporary app without its
demo arguments and briefly showed “Checking saved login…” on the sign-in screen.
It was closed immediately; no credentials or conversations were inspected. We
cannot assert that no credential-store lookup occurred. Subsequent validation
copies forced offline mode at startup, used distinct bundle identifiers and were
not included in shipping artifacts. The source was restored after that temporary
build. Windows/Linux delivery, minimum-width native inspection and normal-user
live interoperability remain unverified; this PR stays draft for those gaps.
See `docs/performance.md` for measured package and runtime deltas.
## September 10: owner-requested merge of all open PRs

The owner explicitly requested "merge everything to main". PR #15 (People subscriptions) was merged into main; the dependent #16, #13, #12, #10, #9 and #7 stack was consolidated into #4 because the repository allows only squash merges. Integration of that combined tree with main efa724b was performed in a separate clean worktree, preserving the original uncommitted channel-access work unchanged.

Conflict resolution retains both the stack's reaction repair, uploads/downloads, channel references, pinned pages and thread/archive navigation, and main's grouped timeline, hover controls, bundled/server emoji and encrypted guild voice. Thread/archive budgets now include Guild::bytes so the new emoji catalogs remain inside navigation byte limits. A protocol emoji test was updated for navigation's Result return; no assertion was removed. Both documentation histories, licenses and existing native evidence were retained. Independent integration review found no concrete remaining blocker.

The integrated tree passed cargo xtask check (139 offline Rust tests, doctests, formatting, strict Clippy, text-only and runtime policy), node tests/login-handoff.cjs, cargo xtask package and cargo xtask package-voice. Unsigned Windows executable sizes are 49,067,008 text and 52,400,128 voice bytes. Full package sizes and five-run reducer comparison are recorded in performance.md. New native interaction/live account/audio verification remains unperformed; these checks are synthetic. Prior security CI still reports six inherited vulnerabilities and five denied warnings; no audit policy or repository protection was changed. The merge is explicitly owner-authorized despite those known evidence gaps, not a claim of full-spec completion or clean security status.


## Channel visibility and stale-history admission - September 10, 2026

Baseline: main 92e82e7b72c716340a20c2643069c482933acd4e after the owner-requested PR
integration. Work is isolated on fix/channel-visibility; the original dirty
fix/channel-access-revocation checkout is preserved. Its permission patch was ported and
adapted to the integrated guild voice, emoji, cache, threads and archive implementations.
Pinned toolchain: Rust 1.98.1, Windows text-only default plus optional voice builds.

Explicit CHANNEL_OBFUSCATED updates now remove inaccessible navigation instead of leaving
placeholder channels selectable. READY filters hidden channels/direct child threads without
inferring permission inheritance for visible category children. Valid full unflagged updates
can restore missing channels in a known guild, while ordinary partial updates preserve omitted
fields. Accepted READY cancels old history requests. Shared reload and HTTP/cache admission
require current text navigation, so revoked messages cannot return through a late response.
Selected replies and views clear while recovery drafts remain. Existing account-wide disk
history clearing also covers navigation removals and readable-to-unsupported transitions.

Guild voice admission, snapshots, mute commands and negotiation secrets honor visibility
revocation. Removal stops affected core calls and rosters; existing desktop polling stops media
and requests leave. Restoration never autojoins. READY-known guild identities stay bounded and
survive temporary guild unavailability so returning guild voice can be deliberately rejoined.
No new dependency, database schema, credential fallback or full permission mirror was added.

Native evidence remains unavailable because the owner's earlier physical Escape stops paused
native desktop automation; it was not resumed for this task. No Discord messages, live account
validation, microphone or audio capture occurred. Synthetic assertions cover state transitions,
not service compatibility. Manual owner reproduction: select a text channel, start history
loading, revoke visibility from the controlled account context, verify the view clears and
Reload cannot restore it, then restore access and deliberately reopen. Repeat with a thread
sync and an active guild voice channel; restoration must not silently rejoin.

Full role/overwrite permission mirroring and the remaining SPEC backlog are still unfinished.
The delivery remains draft while native evidence, remote checks and inherited security audit
findings remain unresolved. Local validation and package/replay evidence are recorded below.

Final local validation: cargo xtask check passed 148 offline Rust tests, doctests, all-feature
strict Clippy, formatting, text-only build and runtime policy. cargo xtask package and
cargo xtask package-voice both produced unsigned Windows packages. cargo replay plus one
warmup and five direct runs retained 500 records / 220,992-221,477 estimated timeline bytes.
Each executable grew by 25,600 bytes; reducer median 26.2823 to 26.5260 ms. See performance.md
for installed/ZIP sizes and comparison limitations. No authentication code changed, so the
previously passing login-handoff test was not redundantly rerun for this slice.

Independent review found and resolved temporary-guild voice readmission and explicit hidden
thread-sync handling. The latter now validates a whole scoped snapshot and carries all removed
IDs in one bounded event, including the browsed archived transient; a regression covers 12 IDs,
more than the eight-event queue capacity. Final independent review found no remaining concrete
blocker. The original seven dirty-source hashes were verified unchanged. The inherited strict
security audit reports six vulnerabilities and five denied warnings; no dependency/audit policy
was changed. New remote CI status is pending at draft delivery, not claimed successful.

## Permission-aware actions - September 10, 2026

This slice builds on channel-visibility PR #17 at 6fb81da12942b03e0a51599803d74ea37cbd3503,
in the separate feat/permission-aware-actions worktree. It implements SPEC 7.3's self-account
role/overwrite calculation for existing messaging, history and voice actions. It does not
claim the remaining full SPEC is complete. Original checkout work remains untouched.

Bounded READY/guild snapshots and role/self-member/owner/channel deltas distinguish unknown
metadata from denied permissions. Shared UI and command checks cover send/attachment,
own edit/delete, existing/new reactions, history/search/pins/archives and listen/speak/PTT.
Read-history loss clears loaded content and cancels stale admission; VIEW-only live messages
and separately authorized sends remain possible. Thread parent changes and deleted-role
overwrite cleanup invalidate previous decisions. Cached decisions respect timeout boundaries
and clock rollback. Protocol references and storage limits are in discord-compatibility.md
and storage-policy.md. The only dependency edge added is the existing test-support crate as
an UI dev-dependency; no new external runtime dependency or schema migration is introduced.

Independent review identified and fixed deleted-role overwrite retention, thread reparenting,
late content after revocation, missing disk-cache invalidation for navigation changes,
unknown reaction snapshot eligibility, and loss of the failed-call state on disconnect.
Older synthetic tests now explicitly provide the loaded guild/self permission metadata they
need, retaining their behavioral assertions. Validation results and measurements follow below.

Native before/after screenshots, process RSS/idle CPU and physical UI interaction remain
unmeasured because owner physical Escape stops paused desktop automation. It was not resumed.
No Discord message, account or microphone action was performed. Manual owner acceptance:
change VIEW/READ/SEND/ATTACH separately in a controlled conversation; check that controls and
late history follow each permission, drafts survive, and restoring access does not send or
join automatically. Repeat parent overwrite/self-role changes for a thread, and CONNECT,
SPEAK and USE_VAD changes for voice. Live normal-user delivery/compatibility remains a gate.

Local cargo xtask check passed: 164 offline Rust tests plus doctests, formatting, strict
all-feature Clippy, text-only compilation and runtime policy. Independent final review found
no remaining concrete blocker. Replay uses one warmup plus five direct release runs with
100,000 events: median 27.9407 to 29.2795 ms, retaining the same 500 records and
220,992-221,477 estimated timeline bytes. Access reconciliation runs only for events that
can change navigation/permissions; message and action admission remain guarded individually.

Base PR #17 CI completed native checks on Windows, Linux and macOS. Its strict security job
still failed with six vulnerabilities and five denied warnings (run 34481036861); those
inherited findings are not waived. New branch CI remains pending until pushed. The delivery
stays draft under the delivery skill while native evidence and audit gates are unresolved.

Both unsigned Windows release packages passed their packaging commands. Each executable grew
by 211,456 bytes over #17; installed/ZIP totals and five-run measurements are recorded in
performance.md. Original seven dirty-source SHA256 values were checked unchanged. No new
native screenshot or live compatibility claim is made; the new PR is stacked on #17.


## Rich-editor and notification permission integration - September 10, 2026

Main advanced to 2879fbc7de8fb7de30664fb64666f6d93482b827 with PR #18 while permission
PR #19 was in progress. This continuation merges that actual main revision into #19,
preserving the rich shared composer, automatic viewed-message ACKs, notification preferences,
platform adapters, package notices and existing documentation/evidence. Source baseline is
#19's 67bff236e6186a306023c156f8027712049681ba; its Windows package hashes were checked before
copying separate comparison baselines. Original dirty checkout work remains untouched.

Shared inline edit Save/Enter now uses permission-aware command admission while preserving
rich text, mentions/IME, pending confirmation, retry and unsent-draft isolation. The old modal
editor was removed during conflict resolution. Notification badge/dot getters and new-message
observation/delivery require VIEW access. Permission revocation clears affected observed/service
counts and queued alerts, retaining dedup high-water state; revoke/restore cannot resurface
old queued notifications. VIEW-only live activity remains usable without READ_MESSAGE_HISTORY.
Desktop notification work is invalidated on navigation/permission events using its existing
generation and app-specific dismissal path. OS history erasure is not claimed.

Windows compilation exposed an inherited notification adapter error: notify-rust 4.18 does
not publicly export its Windows NotificationHandle. The Windows path now retains a success
marker and uses the existing app-ID history dismissal; the private response handle is dropped.
Linux/macOS retain their actual notification handles and macOS's asynchronous show path.
This follows the installed pinned dependency source and received independent review. No new
external dependency beyond PR #18 is introduced, and no OS alert was triggered for testing.

cargo xtask check passed 178 offline Rust tests, doctests, formatting, all-feature strict
Clippy, text-only compilation and runtime policy. New regression assertions cover inaccessible
badges/dots, revoke-before-dequeue, hidden replay suppression, unaffected channels, VIEW-only
live activity and unsaved edits after access loss. Release reducer median rose from 29.2795
to 37.5618 ms for 100,000 events while retaining 500 records / 220,992-221,477 estimated bytes.
The new observation/count work is included; no performance improvement is claimed.

Native desktop automation remains paused after owner Escape stops. Imported #18 macOS
screenshots describe that prior upstream build, not this integrated Windows build. No new
native screenshot, OS notification delivery/dismissal, account operation or microphone test
was performed. The live/storage/platform gates and the complete SPEC objective remain open.
PR #19 remains draft; remote checks and inherited strict audit findings are not waived.


Both integrated Windows package commands passed; executables grew by 306,688 bytes each
relative to the pre-integration #19 baseline. The installed/ZIP comparison is in performance.md.
The original seven dirty-source hashes were rechecked unchanged. The existing PR is retargeted
to current main and includes the channel-visibility prerequisite; no PR was merged to main.
Next concrete SPEC 9.2 implementation: consume bounded PRESENCE_UPDATE events for users already
loaded in the open guild People pane. Existing member-list snapshots display presence, but
standalone live status transitions currently have no dispatcher. Keep that work scoped to the
existing subscription and reject late navigation/generation updates; no full directory fetch.


## Loaded People presence updates - September 10, 2026

This SPEC 9.2 slice starts at f20042a73ca101c377a4c96734aa9f0d833d6fa4 (draft PR #19),
with separate, hash-verified text/voice package baselines. A new isolated worktree preserves
the original checkout's unrelated changes. The existing open guild People subscription now
consumes PRESENCE_UPDATE for already loaded users. Partial user identities are sufficient;
only online, idle, dnd and offline survive decoding. Missing status preserves the previous
value; null/unknown status clears it and the pane says Presence unavailable. Activities,
client device status, profile patches and guildless/global presence are not retained.

The Gateway coalesces each loaded user's latest status until 100 ms after the first change,
without postponing the deadline on each packet. At most 100 users and an 8-KiB compact event
are admitted. Presence cannot grow the existing 128-KiB member-row budget. Full snapshots
supersede pending deltas; subscription cancellation/replacement, invalidation, timeout and
READY/reconnect discard them. Core checks generation, active channel/guild/request, VIEW
access, loaded rows and fresh member state. Status-only events leave timeline revision and
history persistence unchanged. No new dependency, subscription, directory fetch or stored
presence table was added. This does not implement friends presence or role display.

Synthetic coverage exercises wire partial identities/status patches, coalescing and a
1,000-user flood, local WebSocket dispatch/unsubscription, stale scope/access/generation,
atomic byte/item admission and rendered status labels. Native desktop automation remains
paused following the owner's Escape stops, so screenshots and process RSS/idle CPU are
unmeasured. No Discord account, OS notification or microphone was used. Real normal-user
presence delivery through the existing subscription remains unverified; all remaining SPEC
and live/platform gates stay open.

cargo xtask check passed 186 offline Rust tests plus doctests, formatting, strict all-feature
Clippy, text-only compilation and runtime policy. The first full test run hit the existing
Windows terminal-close race in the synthetic reconnect fixture; an unchanged isolated retry
passed. The fixture now keeps its final socket alive until the client observes termination,
using the neighboring test's existing oneshot pattern. The timeout was not increased and
runtime reconnect behavior was unchanged. The subsequent complete check passed. Independent
review found no remaining actionable issue after guild-scope and byte-budget fixes.

This branch remains stacked on draft PR #19. Main separately advanced to 74709e2 (image
aspect-ratio PR #20); those unrelated changes are not included in this measured baseline.
Native evidence remains unavailable, and the base branch security job has failed; neither
gate is waived. Package sizes and replay measurements are recorded in performance.md.


Both Windows package commands passed. Text/voice executables grew by 13,824 / 13,312 bytes;
installed and ZIP totals are in performance.md. Five-run reducer median was 37.4956 ms versus
37.5618 ms at baseline, retaining the same 500 records / 220,992-221,477 estimated bytes; the
small difference is noise. Original seven dirty-source hashes were rechecked unchanged.


## Keyboard conversation navigation - September 10, 2026

This SPEC 9.1/9.5 slice starts at f8baa2df1f705b771b5a2cd99a10bb13b4d8a4c3 (draft PR #21),
with separate text/voice package baselines verified against their recorded executable hashes.
The new isolated branch adds Find conversation in the sidebar and Ctrl/Cmd+K. Search covers
already loaded, VIEW-accessible text channels, DMs, group DMs, threads and voice channels;
unsupported kinds and categories are excluded. Server/channel names and known DM recipients
match case-insensitive query words. Results identify their server or DM scope and show voice
as a roster destination. Selection uses State::select; it never joins a call or sends text.

The picker keeps at most 128 query characters / 512 UTF-8 bytes and 20 bounded result labels.
It recomputes from current navigation/permissions while open and adds no service request,
relationship directory, background index or saved query. Up/Down, Enter, Escape, Tab and mouse
interaction use native egui controls. IME preedit/commit cannot activate or close the picker,
and the composer does not process picker keyboard input, including its closing frame. Cancel
restores prior focus; selecting a text conversation focuses its composer after the modal closes.
Existing drafts and unfinished inline edits remain intact; ordinary navigation cancellation
continues to govern uploads and history. Selecting the current conversation does not reload it.

Native desktop automation remains paused following owner Escape stops. Headless input/shape
checks are synthetic evidence, not native screenshots, screen-reader verification or real IME
validation. No Discord account, OS notification, browser or microphone action was taken. This
branch is stacked on #21; main's independent image-aspect-ratio PR #20 is outside the baseline.
The full SPEC objective, live/platform gates and inherited security findings remain open.


cargo xtask check passed 190 offline Rust tests plus doctests, formatting, strict all-feature
Clippy, text-only compilation and policy checks. Focused input regressions verify query arrows,
Tab to results/Close, Enter targeting, Unicode paste limits, IME keyboard/pointer dismissal,
restored focus, preserved drafts/inline edits and no accidental send or voice action. An extended
post-selection test confirms subsequent typing reaches the newly selected conversation's draft.
Independent review found no remaining actionable issue after the input/focus fixes. Native
screenshots, physical IME/accessibility and process RSS/idle CPU remain unmeasured; no native
or live behavior is inferred from these tests. Base #21 native Linux/macOS CI passed in its PR
run while Windows remained pending; its inherited security job failed. This delivery stays draft.


Both unsigned Windows packages passed. Text/voice executables grew by 33,792 / 33,280 bytes
(under 0.07%); full installed/ZIP comparisons are in performance.md. Native picker latency
and memory remain unmeasured. Original seven dirty-source hashes were rechecked unchanged.

Next concrete hardening step: repair the inherited dependency/security audit failures without
waiving checks, then continue the remaining text/platform/live gates from the full specification.


## Native dependency hardening - September 10, 2026

This SPEC 12/14.5 slice starts at e4ef4a852415e5061735f9183ca65dd202406ecd (PR #22) in an
isolated worktree. Baseline text/voice packages were copied before edits and their recorded
SHA256 hashes verified. Main's independent image-aspect-ratio PR #20 remains outside this
stack's baseline; the original dirty checkout is preserved.

The existing HPKE fork removes its unused optional libcrux backend. A documented Davey 0.1.4
manifest-only fork removes OpenMLS browser timer features from the native build; all Davey
Rust sources and the selected RustCrypto provider are unchanged. No new runtime dependency,
service route, UI behavior, persistence, queue or payload limit is introduced. Original crate
checksums, manifests and licenses are retained; voice packages continue shipping the modified
MPL HPKE source and Davey's existing MIT notice.

The unchanged strict cargo-audit 0.22.2 command improves from six vulnerability-class findings
and five denied warnings to zero and two, dropping 37 packages net from Cargo.lock. It still
exits 1 for Linux glib 0.18.5 unsoundness and proc-macro-error 1.0.4 maintenance. No finding
is suppressed. The Linux authentication migration/backport decision remains a release gate;
see dependency-audit.md. This delivery remains draft while those findings and platform/live
evidence are open. No native UI, OS notification, account, browser, call or microphone action
was taken; desktop automation remains paused after owner Escape stops.


cargo xtask check passed all 190 offline Rust tests, doctests, formatting, strict all-feature
Clippy, text-only compilation and policy checks. This includes the exact SHAKE adapter vectors,
two-party DAVE encryption/decryption and tampering/transition handling, and localhost encrypted
Opus transport. These checks do not establish Discord interoperability. Feature trees confirm
OpenMLS no longer enables js and retains its existing RustCrypto provider. Independent review
verified all 17 vendored Davey Rust source files byte-for-byte against the pinned registry
release, checked the lockfile scope and license/source packaging, and found no actionable issue.


Both unsigned Windows packages passed. Text executable size is unchanged; voice grows 25,600
bytes (0.048%). Exact installed/ZIP deltas and hashes are in performance.md. Packaged modified
HPKE source and Davey's existing MIT notice were verified. Original seven dirty-source hashes
were rechecked unchanged. The complete SPEC objective, native/live acceptance and Linux audit
remediation remain open; this slice does not claim a working live Discord client.

## Image aspect ratios — September 10, 2026

Task baseline: clean `main` at `2879fbc`, fetched `origin/main`; branch `fix/image-aspect-ratios`, Rust 1.98.1. The shared texture painter previously stretched decoded pixels into metadata/layout rectangles. It now contains them at their actual aspect ratio without changing reserved message geometry. Server icons use a square 38-point image slot, profile avatars use their requested square size, and non-square custom emoji preserve proportions in messages, the composer and picker. Existing banner cropping and bounded worker/cache policies remain intact.

Validation: focused media rendering and custom-emoji checks pass; `cargo xtask check` passes (workspace tests, format, strict Clippy, policy). Rendering coverage includes portrait, landscape and square textures, absent/mismatched metadata, inline/enlarged media, and unchanged geometry after loading. Native synthetic before/after evidence and release measurement details are recorded with this task. The existing demo landscape is a vector placeholder, so its screenshots establish layout/viewer behavior; decoded-pixel aspect correctness is verified by the rendering tests. No Discord session, messages, downloads, calls or microphone were used. Windows/Linux and live CDN behavior remain unverified.

Final host text/voice packages pass. Text executable +16 bytes; voice executable unchanged. Settled median CPU remained 0%; RSS samples were noisy (see performance report). Native dark/default-size timeline and enlarged viewer were captured and inspected. Further light/narrow native interaction was blocked by CUA `noWindowsAvailable` / ScreenCaptureKit invalid-parameter errors after repeated target refresh; those variants remain unverified. PR stays draft for that visual verification limitation and pending CI.

## Profile popout — September 10, 2026

Task baseline: clean `main` at `74709e2`, fetched `origin/main`; branch `feat/profile-popout`, Rust 1.98.1. The modal profile card was replaced by a compact 300-point popout anchored beside the clicked user (People row, message author, mention, footer avatar). It flips left near the right edge, stays in the window, and closes on Escape or an outside click. The card now paints the account's two profile theme colors as a gradient when returned, shows the server tag (primary guild/clan) with its badge, badge artwork, a presence dot and custom-status bubble from the retained People rows, and a full-width Message action. The People pane lists custom status under names. Parsing adds `theme_colors` (exactly two bounded RGB values), `primary_guild`/`clan` (disabled tags hidden, 8-character text), `badges[].icon` and the type-4 custom-status activity (128 characters, unicode emoji only). Badge and server-tag artwork use two new validated CDN key forms through the existing bounded image worker. The profile event payload is boxed so the core event enum did not grow. No animated decoration or profile effect is rendered.

Follow-up in the same task: the owner asked that profiles be cached instead of reloaded on every click. A RAM-only cache in `State` keeps up to 32 profiles / 1 MiB for 15 minutes, keyed by user and server scope, cleared on session start, resync/permission change, server removal, session failure and logout. Reopening a cached card issues no command.

Validation: `cargo xtask check` (see the final line of this section), focused UI tests for anchor placement/flip/Escape/outside-click/no requests, parser tests for the new fields, CDN key tests. Native macOS dark-mode capture used the new disclosed `--demo --demo-profile` fixture because pointer automation was unavailable without accessibility permission; before/after images are under `docs/pr-evidence/profile-popout`. Light mode, narrow windows, keyboard-only use and any live payload remain unverified. No Discord session, messages or media requests were used.

## September 10: welcome and other system messages

Implemented native descriptions for all 33 documented nonordinary message types, including welcome joins, recipient changes, pins, boosts, channel/thread notices and calls. Preserve the numeric type through REST/Gateway models and SQLite; render original content separately. Copy and loaded reply previews include descriptions. Unknown types retain a numbered fallback. System rows stay ungrouped and cannot use the own-message edit control. Rich call/subscription/poll details are not invented; search/pins snapshot excerpts remain content-only.

Checked all repository PRs and inspected #27/#28: they cover external fallback and unsupported content markers, not welcome rendering. This PR is based on main, not stacked on them. Integration must combine the timeline controls and the independently introduced schema7 columns. Legacy unsupported cached messages migrate to unknown255 and need ordinary history reload to recover actual kinds. No dependencies/notices changed.

Verification: `cargo test --locked -p discord-protocol -p local-store -p ui` passed. `cargo xtask check` passed 160 offline tests, doctests, format, strict all-feature Clippy, text compilation and policy checks after correcting the fixture-placement lint. Protocol checks cover all documented kinds, ordinary/unknown variants, missing recipient data, bounded Unicode names, original/copy text and invalid type ranges; SQLite checks cover migration, roundtrip/reopen and invalid values; egui checks cover known/unknown rows and light/narrow/wide/dark rendering. Independent diff review found no correctness blocker. Text and voice macOS release packages passed strict ad-hoc signature verification. Native synthetic before/after screenshots are under `docs/pr-evidence/system-messages/`; no account, Discord message, call or microphone action was performed.

Performance methods/results are in the system-message section of `docs/performance.md`: text executable +18,464 bytes, voice +2,064 bytes; idle CPU median 0% both; short native RSS median +1,312 KiB; ordinary reducer replay 29.332→28.687 ms, unchanged retained byte range, no speed claim.

Reproduce: `cargo run --locked -p serein -- --demo --demo-system-messages`, scroll to top for welcome, addition, pin and boost events; scroll down for rename/thread/call/unknown examples. Before screenshot uses the same fixture backported to the baseline without production changes. Native dark screenshots/scrolling were inspected; native light selection via automation did not visibly apply and native keyboard behavior remains unverified (headless light/narrow checks pass). Live account behavior, other OSes and p95 timing remain unverified. Delivered draft [PR #30](https://github.com/ViceVerse-cz/rustcord/pull/30), branch `feat/system-messages`, implementation commit `49a2459`. Native macOS/Windows/Linux and security CI jobs were queued/in progress at handoff; no remote pass is claimed. Both commit-pinned screenshot files were verified through the GitHub contents API. Draft also records outstanding native light/keyboard verification.


## Discord-style theme shell — September 10, 2026

Task baseline: clean `main` at `85fde15`, fetched `origin/main`; branch `feat/discord-theme-shell`, Rust 1.98.1. The owner asked for the application to look like the real Discord client with support for custom recolours (deep black, gradients, light). The shared design module now carries Discord-role tokens (base/sidebar/chat/raised/hover/selected/border, strong/normal/muted text, link, blurple accent, presence and mention colours) and seven presets: Default (refresh dark or light following the system preference), Onyx, Ash, and four gradient recolours (Midnight Blurple, Crimson Moon, Forest, Sunset) that paint a background mesh under translucent surfaces. Presets are chosen from swatches in the account-card settings menu and persist in a new application-wide SQLite `theme_variant` row beside the existing appearance row; unknown keys fall back to Default.

The shell was rebuilt to Discord's layout: inline macOS title strip with centred context title and the offline/experimental pill, 72px server rail with edge pills and mention badges, rounded content surface, 240px channel sidebar with category eyebrows, glyph rows, unread pills and the account card, 48px conversation header with icon tools and a search field, 16/40/72px message rows with medium-weight author names and a floating hover toolbar, rounded composer with attach/emoji/send icons and contextual placeholder, and an ONLINE/OFFLINE-grouped member list. Vector icons are painted from egui primitives (`crates/ui/src/icons.rs`); Inter Regular/Medium/SemiBold (OFL 1.1, 799,444 bytes) are bundled as the proportional face and the `medium`/`semibold` families because egui has no synthetic bold. Forum rows open their post archive directly (the row-level Archive button is gone; text channels keep a Threads icon in the header). Fixture flags `--demo-theme=<key>` and `--demo-light` preview presets. Unofficial/offline state remains visible in the title strip; storage, notification and logout controls stay in the settings menu.

Validation: `cargo xtask check` passed (formatting, workspace tests including new palette-contrast, icon, theme-variant persistence and updated keyboard tests, strict Clippy, policy). Native macOS captures of `--demo`, `--demo-chat`, `--demo-notifications` and `--demo-voice` in Default dark, Onyx, Ash, Midnight Blurple and light were inspected; evidence is under `docs/pr-evidence/discord-theme-shell/`. Sizes and idle CPU/RSS are in `docs/performance.md` (text executable +831,232 bytes). Pointer automation is unavailable on this host, so the settings-menu swatches, hover toolbar and emoji picker were exercised only through headless egui tests; native light/narrow interaction, screen readers, IME and Windows/Linux (where the native title bar remains) are unverified. No Discord session, message, call or microphone was used. One early capture accidentally targeted the owner's separately running Serein window; the image was deleted immediately and the capture script now matches the launched process ID.


## Theme integration follow-up — September 10, 2026

Resumed `feat/discord-theme-shell` at `37a0956` with an existing UI diff and baseline screenshot. The theme commits had already been rebased onto `b92a082` (main, including reply navigation and incoming typing). Reviewed and retained that integration, then preserved the earlier published branch as a merge parent so PR #33 can be updated without rewriting remote history.

The header now gives its controls their actual width before truncating the channel name. Clicking a People avatar opens its profile; draft-storage status remains visible beside the composer. Independent review caught and fixed two additional regressions: title/status text now occupies separate regions (full status available on hover), and active download progress/cancellation remains visible after switching to voice or clearing selection. A regression test covers both download cases. A native gradient capture also exposed text bleed-through in the archive window; the shared window fill is now opaque for every preset.

`cargo xtask check` passes, including all 75 UI tests, workspace tests/doctests, strict Clippy and policy checks. Final text/voice release packages and strict ad-hoc signature verification pass. Native macOS synthetic before/after dark, after-light and Midnight Blurple captures were inspected under `docs/pr-evidence/discord-theme-rebase/`. Baseline is the existing clean `dff0975` worktree/package; it precedes the incoming-typing main commit. Both runs use `--demo --demo-chat`, the same default 1120×760 requested viewport and 2× capture scale; the baseline outer window includes its old native title bar. No account session, messages, calls or microphone use occurred. Pointer/keyboard-only interaction, narrow-window interaction, screen readers, IME, Windows/Linux and live compatibility remain unverified. The gradient archive window itself has not been recaptured after the opacity fix. The PR remains draft for those interaction evidence gaps and pending CI. Refreshed package and process measurements are in `docs/performance.md`.


## Server member list hydration repair — September 10, 2026

Baseline: clean `main` at `9fcce51`, matching fetched `origin/main`; task branch
`fix/server-member-sync`, pinned Rust 1.98.1 on macOS 27 / Apple M1 Pro / 16 GiB.
The prior opcode-37/typing repair was already present. A GUILD_CREATE refresh replaced
READY channel records with records lacking their computed member-list ID, making Reload
people unable to request the server list. New/restored channels and changed overwrites
had the same dependency on stale navigation metadata.

Member requests now derive their ID from the existing bounded permission mirror, sharing
the existing hash with the protocol layer and retaining u128 permission precision.
An identity change clears the open request for the visible pane to refresh; identical
hydration preserves pending replies. Missing metadata stays unavailable, stale replies
remain rejected and threads retain their separate-protocol limitation. No dependency,
additional directory request, UI layout or DM recipient behavior changed.

Verification: the synthetic hydration/reload regression fails against `9fcce51` and
passes after the fix. It also covers overwrite changes, late replies from the active
old request, missing metadata and subsequent hydration. Shared hash tests cover known
vectors, ordering, capacity and high permission bits. An independent review found no
blocking issue. The baseline regression used a detached worktree; affected Cargo package
caches were cleared before final validation to eliminate reuse across the two source trees.
Native screenshots are not applicable: this fixes state/request selection; the synthetic
member rendering is unchanged and identical pictures cannot demonstrate service acceptance.
No saved account, Discord messages, calls, microphone or private payloads were accessed.

Release package measurements and synthetic reducer results are recorded in
`docs/performance.md`. Owner-operated live validation and Windows/Linux runtime testing
remain unrun; this repair is not a claim of complete normal-user interoperability.

Final local verification: `cargo xtask check` passed formatting, all workspace tests,
strict all-feature Clippy and policy checks; the strengthened focused hydration regression
also passed. `cargo xtask package` and `cargo xtask package-voice` built and verified their
ad-hoc signatures. Text/voice executables grew by 1,216/1,200 bytes; synthetic reducer
median was 37.244→37.569 ms with identical retained bounds, within run variation.

Delivery: [draft PR #38](https://github.com/ViceVerse-cz/rustcord/pull/38), implementation
commit `c11b7ab`. macOS, Ubuntu, Windows and security checks were pending at initial
inspection; no remote CI success is claimed. The reported server still needs the owner's
live verification with the repaired build.


### Member-list diagnostic follow-up — September 10, 2026

The owner reports that the live member pane still becomes unavailable after the initial
identity repair. Read-only inspection found the owner-launched `target/debug/serein`
writing stdout/stderr to its terminal, with no member-sync logger in the implementation.
The app lookup accidentally launched a separate packaged copy; that extra copy was closed,
leaving the owner's debug process running. No credentials or private payloads were read
from disk/process memory, and no messages, calls or microphone actions were taken.
A visible server total with no member rows does not identify the remaining failure.

Added explicitly enabled, fixed-label stderr diagnostics via
`SEREIN_MEMBER_DIAGNOSTICS=1`: maximum 64 lines per Gateway run, disabled by default.
These distinguish absent subscriptions, empty/populated SYNC, identity mismatch,
decode/range/capacity failure and timeout without logging service identifiers or content.
Focused Gateway tests (7) and `cargo xtask check` pass; a new debug build is ready.
The owner must restart that build and reproduce once before a concrete live cause can be
claimed. The prior synthetic hydration repair is not evidence that this remaining live
failure has been fixed. The draft PR remains open for that diagnostic result.


Further diagnosis: the owner-run fixed-label trace shows member reply decoding failures,
followed by incremental-only updates and eventual timeout. The current primary protocol
schema permits SYNC group headers containing only an ID; our unused required `count`
field rejected those rows. Removed that field and added a regression preserving the group
index and following member. No raw member payload or serde error text was recorded.

The owner also reported intermittent Safe capacity exceeded. A single GUILD_CREATE can
synchronously emit more channel events than the old eight-slot queue accepts. The reliable
queue now fits one bounded navigation fanout (4,008 items) in the unchanged 32 MiB estimated
byte budget, with nonblocking admission, FIFO order, permit release and repaint while
backlogged. Seven focused connection tests pass, including full navigation burst, exact
byte exhaustion, oversized rejection, cleanup and typing isolation. Debug build includes
both repairs. Detailed temporary structural introspection used during debugging was
removed; shipped diagnostics remain fixed labels only. The owner restarted the repaired
build and confirmed the server member list is working. Long-running live capacity behavior
has not been independently measured. Final `cargo xtask check`, debug build, text package
and voice package passed. Final measurements are in docs/performance.md.

### Member roles and name colors — September 10, 2026

Starting branch `feat/member-role-display` at `7221390`, based on repaired member-sync PR #38.
The owner confirmed that repair loads the affected server list, then requested role headings
and name colors from a visual reference. Reuse bounded role metadata and active member rows;
group loaded online members by their highest hoisted role, with independent highest-colored
role names. Offline and DM behavior remains present. Heading labels support accessibility,
truncation and hover text; colors adapt to the current background for legibility.

The synthetic preview adds two role definitions plus ungrouped online/offline members. Before
evidence shows the original two-member roleless fixture; after evidence explicitly uses the
extended offline fixture. No private reference image or account data is included in evidence.
Focused model/protocol/core/Gateway and UI tests pass, covering current/legacy colors, role
hierarchy, create/update/delete, member SYNC/UPDATE, caps, byte accounting, grouping and
virtualization. Full checks, package measurements and native review recorded below on completion.

Final validation: `cargo xtask check`, debug build, text package and voice package pass.
All four checks on member-repair PR #38 passed, and that PR is ready for review. The role
feature is a dependent PR based on `fix/server-member-sync`. Native before/after PNGs were
inspected at 1120×760 / 2×, each under 0.5 MiB; light-mode rendering was also inspected.
The CUA tool exposed only window controls for the new preview, and click/keyboard/resize
actions produced no observed state change. Thus native profile/keyboard/narrow/scroll checks
remain unverified; the role PR stays draft. Existing headless virtualization/grouping and
contrast checks pass. Live role behavior remains owner-unverified. Release text/voice
executables grew 40,720/40,592 bytes; reducer median 37.213→38.166 ms. Higher noisy native
RSS samples are disclosed in docs/performance.md. The debug binary includes both the member
loading repairs and role display. Only agent-owned offline preview processes were closed.

## Voice completion and recovery fixes - September 10, 2026

Baseline main `9fcce51`, isolated branch `fix/voice-completion`, Windows Rust 1.98.1.
The original dirty checkout and the unfinished custom-status worktree remain separate.

Implemented bounded resumption for voice-server close 4015 (terminal 4014 still ends the call),
and ignored bounded pre-group DAVE proposals according to the initial-group procedure.
Device readiness now acknowledges the current device/security revision, preventing rapid
pause/resume or late callbacks from leaving the UI stuck or accepting obsolete readiness.
Denied SPEAK opens playback without selecting/opening a microphone; mute/PTT remains independent
of device configuration. Revoked VIEW_CHANNEL removes stored voice rosters and prevents late
updates or sidebar/central rendering from exposing inaccessible participants.

The mixer now fills each 20 ms playback tick from short Opus packets, starts a full eight-packet
jitter queue before it can repeatedly reset, and preserves 5/60/120 ms lost-packet timing within
the existing 5,760-sample per-speaker PCM bound. At most eight packets are decoded per tick and
three consecutive missing packets are concealed; no encoded/PCM queue or dependency grew.

Validation: final `cargo xtask check` passed all 296 offline tests, strict all-feature Clippy,
formatting, text-only compilation and policy checks. Tests use synthetic MLS keys, localhost
WebSocket/UDP, device-free callbacks and independent Opus decoding. Independent review found
and resolved long-packet concealment timing and stale device-error handling issues. Native
automation remains owner-paused; no new screenshots, microphone/speaker access, Discord calls,
or account actions were performed. Live two-way audio, physical device loss/switching, echo,
real-time callback behavior and cleanup on each OS remain unverified; milestone 4 has NOT passed.
Both `cargo xtask package` and `cargo xtask package-voice` passed. The voice executable stayed
at 54,142,464 bytes; text grew 2,560 bytes. Release replay and 1/8/63-speaker mixer workloads
passed with no measured slowdown; results and limits are in docs/performance.md.


## Voice stuck during connection - September 10, 2026

The owner reported that the voice-enabled build starts a call but neither direction has audio
and the call remains in a connection state. Baseline main `b72b3b1`; work is isolated in
`fix/voice-negotiation`, preserving the running owner process and unrelated custom-status work.

The outgoing key package contained an extra four-byte MLSMessage header compared with libdave
and the inspected reference client's actual send chain. Removed that header and replaced the
fixture's matching assumption with exact raw-KeyPackage parsing and cryptographic validation.
The source/whitepaper discrepancy is documented in the voice adapter README. This is a strong
candidate for the reported negotiation failure; the owner's live result has not been reverified.

Connection progress now distinguishes transport, UDP, encryption and native-device opening;
static negotiation timeout messages identify the missing stage. Device opening now has a
20-second Desktop watchdog. CPAL's synchronous default-device format lookup can wait without a
bound; the watchdog disables audio and requests departure while retaining the retiring worker,
preventing repeated stuck workers. It cannot forcibly release an OS call that never returns.

`cargo xtask check` passes: 298 offline tests, format, strict all-feature Clippy, text-only compile
and policy. Synthetic tests cover raw key-package validation, localhost encrypted DM/guild
negotiation and resume, status order, missing group versus pending transition, and the device
watchdog boundary/reset. Independent source review found no remaining blocker in this diff.
No live call, microphone, speaker, native automation or account action was performed by the
agent. The previous native pause remains respected; screenshots and physical audio remain
unverified. Milestone 4 remains open. Both unsigned Windows packages pass; text executable size is
unchanged and voice grows 4,096 bytes. Package/replay measurements are in docs/performance.md.


After implementation, the owner explicitly requested running the build. Launched the new voice
release executable from the isolated negotiation package; process 28944 exposed a responding
Serein window. This confirms launch only. No call controls, microphone, account contents or
native screenshots were accessed; the owner performs the live retry.


## Incoming custom-status updates (September 10, 2026)

- Baseline: main `5b2cc9e873f0040c9fdf85c37a456872b685000e`; resumed six paused task files
  on `fix/custom-status-updates` and fast-forwarded without changing their hashes.
- Complete presence values now carry custom text through protocol, the loaded Gateway mirror,
  bounded event admission and reducer to People and an open profile. Status and custom-text
  patches resolve independently; bursts retain the latest complete values at a fixed 100-ms
  deadline. No additional profile requests, subscriptions, persistence or dependencies.
- Shared snapshot/update activity parsing caps count/input strings and normalizes only first
  custom activity text. Atomic member admission retains the existing 100-row/128-KiB ceiling;
  compact events allow 64 KiB including vector/string capacities.
- Native automation remains owner-paused; before/after screenshots, native resource sampling
  and live account delivery are not verified. Headless synthetic UI assertions exercise both
  displayed copies without a refetch, and localhost Gateway tests exercise wire/coalescing.
- `cargo xtask check` passed: 304 offline Rust tests, doctests, formatting, strict all-feature
  Clippy, text-only compilation and policy checks. The first focused run caught serde structs
  accepting positional emoji arrays; a map-only visitor and update/snapshot regressions fixed
  that shape validation. Independent code review found no remaining blocker.
- `cargo xtask package` and `cargo xtask package-voice` passed. Text/voice executables grow
  13,824/13,312 bytes; installed packages and ZIPs are measured in docs/performance.md.
  `cargo replay` plus one warmup/five direct runs passed, retaining 500 messages within the
  unchanged estimated 228,992..229,477-byte range. Timings are noisy; no speedup claim.

## September 10, 2026 — message images and continuation spacing

Implemented responsive rows for adjacent image attachments (two columns, one below 280 pt),
with aspect-preserving previews and individually accessible viewer actions. Image filenames
are no longer captions in chat; file attachment names/downloads and viewer metadata remain.
Removed final Markdown block newlines that created a blank line after normal messages,
preserving internal breaks and existing author/reply/date/unread grouping boundaries.
Updated width-aware timeline estimates and added a second synthetic image to the default demo.
No network, credential, storage, decoder, dependency or cache-limit change.

Baseline: clean task branch `t3code/improve-message-images-spacing`, commit `c4ae54d`, equal
to fetched `origin/main`; Rust 1.98.1, macOS 27.0 / Apple M1 Pro / 16 GiB. Reused this task's
branch. Baseline and changed release packages are kept separately under ignored `target/`.
`cargo test --locked -p ui`: 85 passed. `cargo xtask check`: passed strict all-feature Clippy,
307 workspace/doc tests, text-only check and repository policy checks. Regression coverage
checks image wrapping/click targets/hidden captions and compact rows with internal newlines;
the existing short-viewport embed assertion now accounts for removal of its blank text line.
Native screenshots, release package measurements and remaining platform limits are recorded
in `docs/pr-evidence/message-images-spacing/` and `docs/performance.md`.

`cargo xtask package` and `cargo xtask package-voice` passed; host packages were ad-hoc
signed/verified, not notarized. Native dark/light gallery screenshots and unchanged chat-fixture
before/after screenshots were inspected. The second image opened via its accessible action;
Escape closed the viewer. Narrow layout was checked headlessly because native resize automation
did not resize the window. Native control reconnection after relaunch required unique local
bundle IDs (evidence copies only). Executable deltas: text +16,960 bytes (0.036%), voice +544
bytes (0.001%). Idle CPU median 0.0% in both matched runs; RSS varied with desktop conditions,
so there is no established memory improvement. See the full measurements and target miss.
Windows/Linux visual checks, live Discord behavior, screen readers and latency remain unverified.

Conflict refresh: merged `origin/main` at `9ce22a9`, preserving its unread-navigation and
forward-history implementation alongside the gallery/spacing changes. Conflicts were limited
to this append-only progress log and `docs/performance.md`; both sets of records were retained.
`cargo xtask check` passed after the merge, including formatting, strict all-feature Clippy,
workspace tests, the text-only build and policy checks.

## Unread navigation and forward history (September 10, 2026)

- Baseline: clean main `c4ae54def29b3cb87984a59d171b47f0a483283a`; isolated worktree/branch
  `feat/unread-navigation`. SPEC8.2/9.4 requires preserving reading position and deliberate read
  state. Jump to unread now starts after the service read boundary and Next messages walks
  forward in bounded replacement pages. Existing older history and explicit present navigation
  remain available, with drafts/reply context preserved.
- A known unread gap does not auto-ack just because the first recent page opens at its bottom.
  UI actions suppress automatic ACKs while browsing, including short pages and empty results.
- History requests retain existing permissions, cancellation, request/session generations and
  response limits; after pages validate every returned ID above the cursor. Latest-page SQLite
  hydration never substitutes for a forward request. Demo pages follow the same cursor shape.
- Review found and fixed distant Gateway/HTTP-confirmed messages splicing a live tail into old
  history, deletion knowledge lost while suppressing a distant source, and progress lost when
  every page message/newest navigation message was deleted. Bounded accepted cursors/full-page
  state preserve forward progress without treating an after-page maximum as the channel latest.
- Native automation remains owner-paused. Native before/after screenshots, frame/resource
  measurements, screen-reader inspection and live account delivery are unverified. No account,
  message or audio-device actions were performed.
- `cargo xtask check` passed 313 offline Rust tests, doctests, formatting, strict all-feature
  Clippy, text-only compilation and policy checks. Synthetic tests cover local HTTP cursor
  queries/exclusivity/capacity, demo after-pages, deleted/empty pages, stale requests, old-window
  sends in either arrival order, cursor replacement, and keyboard/pointer UI navigation.
  A UI test initially clicked the explanatory history notice; targeting the actual button
  fixed the test without weakening assertions. Independent review findings are resolved.
- Both unsigned Windows packages passed. Text/voice executables grow 8,704/10,752 bytes.
  Package/ZIP deltas and one-warmup/five-run replay results are in docs/performance.md;
  retained 500-message data remains 228,992..229,477 estimated bytes. No speedup claim.

## Server member sync main refresh — September 10, 2026

Fast-forwarded `fix/server-member-sync` to its published head `71ee144`, then merged
`origin/main` at `5f11cb9` without rewriting branch history. The Gateway test conflict keeps
the member diagnostic cap and role-membership coverage alongside main's newer partial-presence
coalescing regression. Append-only performance, progress and storage records from both branches
were retained. No feature behavior or resource limit was intentionally changed by the merge.
The focused Gateway member suite passed 9 tests. `cargo xtask check` and
`cargo build --locked -p serein` passed after adding neutral display metadata to two new
main-branch test fixtures that construct the extended role/member models.
While validating, `main` advanced to `192b40c`; a second normal merge retained its group-mention
roles with neutral display metadata and kept both progress entries. Five focused notification
tests, `cargo xtask check`, and `cargo build --locked -p serein` passed against that final base.

## Group mentions and silent notifications (September 10, 2026)

- Baseline clean main `9ce22a9585d3db0cbff4772fb571dfb4113c05d8`; isolated branch
  `feat/notification-mentions`. SPEC9.2/9.4 mention/notification handling now includes supplied
  role IDs, everyone/here and silent-message flags without scanning message text or requesting
  directory data. Self roles and explicit suppression preferences determine group pings.
- Silent messages retain badges but never enqueue alerts. Direct/DM mentions remain independent
  of group suppression; existing mute/DND and incomplete-settings gates remain conservative.
  Queued role alerts recheck current membership, with 32 items/16 KiB including slot capacity
  and role allocations. Edits/history do not alert; fields are not persisted to SQLite.
- Decoder/model tests enforce 100 positive unique role IDs/800 retained bytes, null/shape/duplicate
  rejection and allocation accounting. Core tests cover suppression/unknowns, dedup, silent
  badges, role removal before delivery and queue byte pressure. Storage roundtrip proves
  notification metadata is not restored. Independent review found no remaining blocker.
- `cargo xtask check` passed 320 offline Rust tests, doctests, formatting, strict all-feature
  Clippy, text-only compilation and policy checks. Both unsigned Windows release packages passed.
  Text/voice executables grow 13,312/10,752 bytes; replay median 36.6610 to 39.9284 ms (+8.91%)
  on the shared host, with retained timeline estimates +8,000 bytes for 500 messages. Full
  samples/package deltas are in docs/performance.md. Native screenshots,
  resource measurements, OS notification delivery and live account behavior remain unverified
  because native/live validation is owner-controlled. No account or audio actions occurred.


## Automated dependency license policy (September 10, 2026)

- Baseline main `192b40c69aafd7fcab9a10f0c10bf1a75e981fda`; isolated branch
  `chore/dependency-license-checks`. The original checkout has unrelated rich-presence edits
  and remains untouched. This baseline includes the separately merged gallery/spacing PR #43.
- SPEC14.5 now has an automated declared-license check: pinned cargo-deny 0.20.2 checks the
  locked all-feature/all-platform graph, including development and vendored path dependencies.
  The new CI job fetches sources first; `cargo xtask licenses` itself runs offline.
- Existing MPL components and egui font licenses have exact-version exceptions. No private/path
  package exemption, custom SPDX parser, runtime dependency or lockfile change. Temporary local
  dependency fixtures exercise accepted expressions, rejected AND/GPL/missing licenses and
  exact versus mismatched vendored exception versions.
- `cargo xtask licenses` and all six offline policy fixtures passed. The initial policy run
  rejected existing Boost clipboard bindings and MPL CSS dependencies; source manifests/license
  texts were inspected and the policy records these existing dependencies. `cargo xtask check`
  passed 323 offline Rust tests, doctests, strict all-feature Clippy, formatting, text-only
  compilation and the existing policy checks. This also checks the merged PR #43/#45 code.
  Independent review found no blocker.
  No application runtime changes or native screenshots are required.
  Complete transitive notice/source assembly and external library redistribution review remain
  separate gates; a passing declaration check does not complete them. Remaining implementation
  candidates include fuzzing, Linux distribution packaging, authorized guild message deletion,
  and bounded unknown-event diagnostics. Live/native evidence gates remain owner-controlled.


## Authorized single-message deletion (September 10, 2026)

- Baseline clean main `899fca77347553516186e8686133f29c6ef6a66f`; isolated branch
  `fix/authorized-message-deletion`. SPEC9.2 now separates deletion permission from editing:
  loaded guild messages from other authors admit the existing delete confirmation with effective
  MANAGE_MESSAGES. Own messages remain deletable without SEND_MESSAGES; edits stay author-only.
- The shared admission gate validates user, channel, message, connection and current VIEW access,
  with an explicit guild boundary for deleting others. Existing role/overwrite/thread-parent,
  timeout and administrator calculations apply. Known non-deletable/unknown kinds are denied;
  automoderation notices always require MANAGE_MESSAGES. Confirmation rechecks the shared gate.
- Reuses the single-message DELETE endpoint and reconciliation; no bulk action, new queue,
  optimistic deletion, dependency, persistence or role-directory request. Synthetic HTTP checks
  distinguish confirmed success, forbidden access and uncertain server failure without retries.
- Focused checks passed 12 core deletion/reconciliation tests, one headless menu test and one
  local HTTP test. `cargo xtask check` passed 340 offline Rust tests, doctests, formatting,
  strict all-feature Clippy, text-only compilation and policy checks. Independent review found
  no blocker. Both unsigned Windows release packages passed; each executable is 512 bytes
  smaller. Paired replay medians 39.6040 to 39.5085 ms; retained estimates unchanged. Initial
  host-load differences motivated the paired rerun; full samples/deltas are in docs/performance.md.
  Native screenshots,
  UI resource measurements and screen-reader checks remain unavailable while desktop automation
  is owner-paused. No live account, deletion, microphone, speaker or call action was performed.
  Official bot-facing message documentation supplies protocol evidence, not normal-user proof.


## Bounded protocol and state-transition fuzzing (September 10, 2026)

- Baseline main `dc49d640302c5244c84953dfc8345396e82ce971`; isolated branch
  `test/bounded-protocol-fuzzing`. SPEC14.1 now has two real coverage-guided libFuzzer targets
  with AddressSanitizer: decoder/conversion boundaries and state-transition invariants.
- The separate fuzz workspace has a committed lockfile, cargo-fuzz 0.13.2, libfuzzer-sys 0.4.13
  and nightly-2026-09-09 (Rust 1.100.0-nightly). Shared dependency identities/checksums match
  the application lock; no application dependencies or production APIs changed. Both graphs
  are covered by offline license checks; the development-only NCSA exception is exact-version.
- `cargo xtask fuzz` uses fresh copies of 26 small synthetic seeds, one worker per target,
  30-second/one-million-run budgets, five-second input timeouts and a 512 MiB RSS ceiling.
  Decoder inputs reach the 4 MiB wire boundary; state inputs are capped at 16 KiB/256 operations
  and 24 MiB cumulative generated payload. Invocation corpora are cleaned; latest failure
  artifacts are bounded by input size. CI has a separate Linux smoke job and seven-day artifact
  retention. Normal native CI stays on Rust 1.98.1; no live/account/audio operation is involved.
- Windows MSVC/ASAN runs passed: decoder 158,424 executions, coverage counters 2,469 to 5,486,
  reported RSS 297 MiB; state 8,842 executions, coverage 2,436 to 3,339, reported RSS 382 MiB.
  Each reported 31 seconds for its 30-second budget. Seeds were replayed during initialization,
  corpora were removed and lockfiles stayed unchanged. These are tool-process observations,
  not client memory measurements, coverage percentages or proof of absent bugs.
- `cargo xtask check` passed 340 offline Rust tests, doctests, formatting, strict all-feature
  Clippy, text-only compilation and policy checks on the original task baseline. The separate
  fuzz formatter, both license graphs and all six license fixtures passed. Independent review
  found no blockers. Native application builds were not repeated for this tooling-only change.
  Linux/macOS fuzz execution and remote CI remain unverified until their checks finish.
- Integration with main `4325dd1` preserved the presence and voice UI changes. The combined
  `cargo xtask check` passed 351 offline Rust tests plus all format, Clippy, compile and policy
  checks; both license graphs and the separate fuzz formatter passed again. A second ASAN
  smoke passed on the integrated code: decoder 112,015 executions/281 MiB reported RSS,
  state 2,892/350 MiB, 31 seconds each. No crash, lock changes or retained generated corpus.
- Remaining spec implementation candidates include Linux distribution packaging and bounded
  unknown-event diagnostics. Native accessibility/resource/storage tracing and owner-controlled
  live text/voice interoperability remain separate, incomplete gates.

## Rich presence in members, DMs and profiles (September 10, 2026)

Implemented from clean main `5f11cb92644d06e2302678211ac21d9445fad692` on
`feat/rich-presence`. The original checkout was clean and safely fast-forwarded after fetching
origin; default branch remains main. Member rows and DM sidebar/header display a received
activity summary. Profiles keep custom status and show activity names, details and states even
if profile metadata is loading/unavailable. All activity content is synthetic in default tests
and `--demo --demo-profile`. No dependency, live account action or persistence was added.

The shared parser bounds rich activity count and field sizes. Guild snapshot/delta propagation
uses the existing member subscription and 128 KiB pane. Known DM recipients receive a bounded
RAM cache and coalesced global updates plus unofficial READY/SUPPLEMENTAL friend snapshots.
Offline/null/empty changes clear correctly; omitted fields remain unchanged. Disconnect hides
DM data; successful resume applies replayed updates while preserving unchanged activity. Fresh
READY/resync/logout discard it. Unknown/removed recipients and stale account generations do not
populate the cache. Profile changes do not refetch metadata or churn timeline revisions.

Verification: `cargo xtask check` passed (formatting, strict all-feature Clippy, all-feature
workspace tests: 327 passed, text-only compilation, policy checks). The first full attempt exhausted C:
space; this task's temporary cache was moved to E: and the check passed using
`CARGO_TARGET_DIR=E:/codex-builds/rustcord-rich-target`. A policy block prevented deleting that
temporary cache; the safe move preserved it instead. Existing unrelated files/caches were kept.
Focused UI tests include render-and-clear on member/profile and all four DM surfaces, separate
server/global presence, and no external platform commands. Core/Gateway regressions include
bounded batching, omission/clearing, rejected unknown users, stable no-op allocation, cache
budgets, recipient removal and presence replay before RESUMED.

Baseline native dark screenshots were captured and inspected using the unchanged release
`--demo --demo-profile` process. Computer Use then reported owner physical Escape and was
stopped. After screenshots, native light/narrow/long-content/keyboard/scroll inspection, and
comparable after native memory/CPU measurement are blocked by that owner stop. No further
Computer Use or after executable launch was attempted. Keep the PR draft until this evidence is
completed. See `docs/pr-evidence/rich-presence/README.md` and the dated performance entry.

Text-only scope: artwork, elapsed/progress timers, party counters and activity actions are not
implemented. No Windows after-render, macOS/Linux native or live Discord interoperability claim.

Both release package variants and `cargo replay` passed. Text executable +86,016 bytes; voice
+84,992 bytes. Paired replay medians37.6737 ->37.5833ms with unchanged retained range; no speedup
claim. Exact installed/ZIP sizes, raw samples and method are recorded in performance.md.


### Owner-authorized merge of rich presence

The owner explicitly requested merging PR #48 after the draft handoff. Reconciled with main
`dc49d640302c5244c84953dfc8345396e82ce971`, preserving new role metadata/grouping, computed member-list
subscription IDs, notification changes and guild deletion authorization. Rich activity and role
fields coexist with both admission limits. Boxed member payloads in the wire member variant and
voice state event keep the combined enums compact without suppressing Clippy.

The integrated tree passed `cargo xtask check` (351 tests), `node tests/license-policy.cjs`
(six offline fixtures), and `cargo xtask licenses` with pinned cargo-deny 0.20.2. Missing platform
crate sources were fetched with `cargo fetch --locked` before the offline license rerun passed.
Both release packages passed: text 50,987,520 bytes and voice 54,337,024 bytes. `cargo replay`
passed: 100,000 events in 39.4606 ms, 500 rows, 236,992..237,477 estimated bytes. This is a single
integration smoke run, not a performance comparison with the original PR baseline; main changed
message metadata in the meantime. Native after evidence remains unavailable following owner
Escape. The owner approved merging with this known limitation and pending remote CI; no branch
protection bypass or renewed Computer Use was requested.

### Already-running game presence at startup

Follow-up to the missing Genshin Impact report, baseline clean main `ebad184` on branch
`fix/ready-game-presence`, Rust 1.98.1. Identify requests no capabilities, but the previous
bootstrap only consumed merged presence records. Accept legacy `READY.presences` / `user.id`
alongside merged friend records through the same bounded decoder and existing DM recipient filter.
No subscription or rendering change. Guild-scoped presence remains scoped to its guild.

The synthetic regression failed before the fix and passed afterward. A loopback Gateway test
sends only legacy READY with an already-running Genshin Impact activity and verifies that the
core exposes `Playing Genshin Impact` for the known DM recipient without any PRESENCE_UPDATE.
The pre-existing coalescing, recipient filtering and bounded multi-batch tests also pass.
Workspace check passed 352 tests; the subsequently added loopback test passed separately.
No owner account session was used, so this establishes a startup parsing defect rather than
confirming the exact cause of every missing live activity. Native capture remains paused by
the owner's earlier Escape stop; the initial follow-up PR was delivered as draft without it.
Both release packages and replay passed; each executable grows 512 bytes. Five-pair replay
medians were 40.5364 -> 39.6790 ms (noise), with unchanged retained bounds. Final formatting
and strict all-feature Gateway Clippy passed after adding the loopback regression. Package and
replay measurements are in performance.md. Unrelated `target-relocation-remainder/` was preserved.

The owner subsequently authorized merging PR #50. Integrated main `4325dd1`, retaining the
voice UI/icon atlas changes and both sets of progress/performance notes. Only the appended
documentation sections conflicted. The combined tree passed `cargo xtask check` (353 tests,
strict Clippy and policy); owner-paused native evidence and unverified live behavior remain
explicit limitations of the authorized merge.
Both integrated release packages passed: text 51,126,272 bytes and voice 54,479,360 bytes.
These include main's new voice UI and atlas; earlier paired presence measurements remain
historical evidence of the isolated fix, not the combined UI change. Remote CI is pending.
Main's subsequently landed fuzz tooling (`68526e8`) merged cleanly without application runtime
changes; the final combined `cargo xtask check` passed again (353 tests, Clippy and policy).

## Discord-style voice UI and Phosphor icon atlas (September 10, 2026)

- Baseline: main `c4ae54d`, clean tree; branch `t3code/improve-voice-chat-ui`.
- Owner rejected the primitive-painted glyphs. `crates/ui/src/icons.rs` now draws Phosphor
  Icons 2.1.1 (MIT) from one bundled 512×320 atlas (`assets/icons/`), tinted at draw time and
  uploaded once at startup; the `Icon` API is unchanged and gains slashed mic/headphones, camera,
  screen share, activities, soundboard, hang-up, in-call, add-people and profile glyphs.
  `tools/generate-icons.py` pins every upstream SVG hash and rasterizes with `resvg` 0.45.1.
  License staged as `licenses/Phosphor-Icons-MIT.txt`; notices, asset README and design doc updated.
- Voice views follow Discord: DM calls show a black stage above the conversation with 80px
  participant avatars and mute/deafen badges; guild voice channels show 16:9 participant tiles
  with name badges in a virtualized grid and a green Join Voice button when not connected. The
  bottom control bar is Discord's pill layout (mute + settings chevron, camera, screen share,
  activities, soundboard, more, red hang-up). Unsupported controls are disabled with hints.
  Header shows a green "In a call" badge; the account card gains mute/deafen toggles; a
  "Voice Connected" panel with disconnect sits above it while a call is active; incoming calls
  use a compact banner with round answer/decline actions.
- New offline fixture `--demo --demo-call` (synthetic DM call, one muted peer) for screenshots.
- `cargo xtask check` passed: 304 offline tests, formatting, strict all-feature Clippy, text-only
  compile and policy. Both macOS packages built; sizes and single-run demo memory/CPU samples are
  in docs/performance.md. Native `--demo-call`/`--demo-voice` captures (dark and light) inspected;
  evidence in docs/pr-evidence/voice-call-ui. No live call, microphone or account action occurred;
  Windows/Linux rendering and real call behaviour with the new controls remain unverified.

### Activity artwork in profile cards

The owner confirmed rich-presence text works after launching main3307396, then requested the
missing image. Baseline3307396 was preserved in a separate E: worktree/package directory;
implementation branch feat/activity-artwork does not replace the running main executable.

Profile activity cards now display a 64px static image to the left of name/details/state, for
both guild and DM profiles. Prefer supplied large artwork, small artwork when large is absent,
or a public application-icon lookup for application-only activities. Asset IDs and bounded
Discord media-proxy paths pass through the same presence snapshot/update flow and memory
budgets. Reuse the existing credential-free worker, cache, placeholders and texture lifetime;
no dependency, separate cache or background activity-directory loading is added.

Focused model/protocol/core, local HTTP/cache, and headless egui checks pass. The egui check
verifies actual tessellated artwork appears and disappears with activities in guild/DM profiles;
an initial assertion checked untessellated shapes and was corrected to inspect rendered meshes.
Full cargo xtask check passes358 tests, formatting, strict Clippy and policy. The synthetic demo
uses an original generated emblem, never live game artwork. Native before/after capture and
process sampling remain unavailable following the owner's earlier Escape stop; a subsequent
request authorized launching the live build for the owner, not renewed screenshot automation.
No live account artwork test was performed. Required native evidence therefore remains a draft
PR blocker under the delivery skill; no completion claim about live artwork or visual QA.
Both release packages and cargo replay passed. Each executable grows17,408 bytes. Five paired
replay medians40.3052 ->40.4948 ms are noise with unchanged timeline bounds; see performance.md.
Independent cross-layer review found no additional blocking defect. Package measurement does not
substitute for native screenshot/process or live-account artwork evidence.


## Debian/Ubuntu distribution packaging (September 10, 2026)

- Baseline main `68526e822461ff8134c3b14e786b17f4bf5920ce`; isolated branch
  `feat/linux-distribution-package`. The original checkout's ongoing work remains untouched.
- SPEC14.5 packaging now stages an unsigned native `.deb` for each text/voice variant through
  the existing package commands. Runtime shared-library dependencies come from dpkg-shlibdeps;
  desktop libraries loaded dynamically and session services are declared separately. The
  desktop entry installs with the binary, documentation, asset notices and relevant voice source.
- Packaging uses an explicit input list and a fresh private temporary directory. Root ownership,
  executable/data modes, metadata, extracted file contents, desktop syntax and linked-library
  availability are checked before the archive is copied to dist. No maintainer scripts, autostart,
  account-data changes, root installation or application launch occur in this smoke test.
- Windows `cargo xtask check` passed 351 Rust tests, doctests, formatting, strict all-feature
  Clippy, text-only compilation and policy checks. The Linux synthetic package regression passed
  both variant payloads, stale nested-file exclusion and invalid ELF/payload rejection. Independent
  review found no remaining blockers after the source-manifest fix.
- Integrated main `3307396` (startup game-presence fix); Windows full checks passed again with
  353 Rust tests. Both real Linux release variants and `.deb` package smoke passed on Ubuntu 26.04
  x86_64 under WSL2, Rust 1.98.1. Text: 52 payload files/28,056,260 compressed bytes; voice:
  98 files/29,256,868 bytes. Full installed/binary sizes and measurement limits are in performance.md.
  The locally provisioned compiler and build cache are isolated under the E: build directory.
  X11/Wayland rendering, Secret Service, portal dialogs, IME, accessibility and actual login/audio
  remain unverified; WSL compilation and archive inspection do not prove those desktop paths.
- Previous PR #51's Linux fuzz, license and security jobs passed on GitHub; native jobs remained
  queued or running at this inspection. This is separate from the new packaging validation.

- Packaging host emitted a dpkg-shlibdeps warning for the libc6 `/lib64` loader diversion.
  Readlink and dpkg ownership checks resolve both paths to the installed libc6 loader;
  generated Depends includes libc6 >=2.43 and both host library-closure checks passed. No
  missing-library/dependency-metadata checks were suppressed. These builds target this host
  distribution, not older Debian/Ubuntu versions. The `.deb` files remain unsigned.
- No application source/dependency changes were made by this slice. Native install/launch,
  Wayland/X11, desktop services, IME, accessibility and physical/live audio remain unverified.
  Remaining implementation work includes bounded unknown-event compatibility diagnostics;
  source-license assembly and the documented runtime/live evidence gates remain incomplete.

### Owner-authorized merge of activity artwork

The owner explicitly requested merging PR #54. Integrated main31bf546, preserving Debian
packaging and both sets of progress/performance notes; conflicts were limited to appended docs.
The combined cargo xtask check passed358 tests, strict Clippy and policy. Application source,
assets and lockfile match the previously verified artwork release builds exactly, so those
Windows text/voice package results remain applicable. Linux packaging is covered by its own
main/CI checks, not claimed as locally executed on Windows. Before integration, macOS, Ubuntu,
security, licenses and fuzz CI passed; Windows CI was still pending. Native screenshots remain
owner-paused and live artwork remains unverified; this explicit merge request accepts those
reported limitations without changing repository protection or enabling account automation.


## Bounded Gateway compatibility diagnostics (September 10, 2026)

- Baseline main `31bf546a0754f157e131007003b6df70db57f353`; isolated branch
  `feat/bounded-gateway-diagnostics`. SPEC7.2 unsupported dispatches now have opt-in fixed-label
  diagnostics, including missing names and unsupported opcodes. Received event names, payloads,
  credentials and account/message metadata are never logged. Existing ignore/protocol-error
  behavior is preserved; no new capabilities are inferred from unknown inputs.
- Reuses member diagnostics with independent 64-record/8-KiB attempted-output budgets per
  enabled scope, across reconnects. No raw archive, retained strings, queue or new dependency.
  Broken stderr and partial writes are charged but cannot panic these diagnostic paths.
  The desktop's existing terminal diagnostic also uses fallible output, for either enabled scope.
- Focused synthetic tests pass disabled output, exact UTF-8/line bounds, oversized labels,
  failed writes, redaction and continued message/heartbeat flow after unsupported events.
  A separate opt-in loopback run emitted exactly the three expected fixed labels and no
  synthetic private markers. No owner account, desktop, microphone or speaker was accessed.
- Validation exposed cached xtask selecting its compilation checkout instead of the caller's
  workspace. `cargo locate-project --workspace` now resolves it at runtime. The new Node
  regression failed on the old binary and passed after the fix, from a nested synthetic workspace.
  CI runs that regression. Shared Cargo output also reused an old test binary; those initial
  full-suite results are discarded. The private-target full suite passed 353 tests, including the new test names, plus Clippy,
  formatting, policy and the workspace regression. CONTRIBUTING documents separate worktree
  target directories. Main `34c4a8f` (activity artwork) is integrated with both sets of
  documentation preserved. The combined private-target check passed 358 Rust tests, doctests,
  formatting, strict all-feature Clippy, text-only compilation and policy; the cached-workspace
  regression passed again. Both Windows release packages and replay passed from the private
  target. Text executable size is unchanged; voice adds 1,536 bytes. Paired reducer medians
  were 41.1287 ms before and 40.1348 ms after, with identical retained byte/row bounds; this
  small variation is noise, not a speed claim. See performance.md for package measurements.
- No visible UI change, so screenshots are not applicable. On the initial PR head, macOS, Linux,
  security, licenses and fuzz CI passed; Windows was pending. Final integrated-head checks are
  reported separately. Native desktop, storage tracing and owner-controlled live text/voice
  evidence gates remain incomplete.

## September 10, 2026 — egui main and native emoji experiment

Starting from clean `main` at `3307396005f72f2e2b26946204b27991881d4ad1`
(same as fetched origin/main), branch `chore/egui-main-test` pins egui and eframe
and their ecosystem to upstream main `65e7db3c06d779c60ac56647bdd3011ed8ba1cbd`.
The exact revision came from upstream `git ls-remote`, not a cached version page.
`Id::new` calls were migrated to `Id::unique`; panel state reset now uses the
parent-scoped ID required upstream. The existing sidebar test caught the latter
regression and now passes. The font coverage test uses `FontData::bytes()`.

The owner's follow-up reported striped placeholders for Unicode emoji. Enabling
eframe `system_fonts` also enables epaint color font decoding and OS font fallback
for normal labels/editors, including channel names. Bundled text fonts and existing
Twemoji rendering remain. No system emoji font is copied into the distribution.

Validation: `cargo xtask check` passed with native fallback enabled: strict Clippy,
353 tests passed, one existing opt-in voice test ignored, text-only check and policy
checks passed. `cargo deny --locked --all-features check licenses advisories` passed
with cargo-deny 0.20.2. Existing vendored Wry deprecation/unsafe warnings remain;
no egui ID deprecation is suppressed. macOS 27.0, Apple M1 Pro, 16 GiB, Rust 1.98.1.
Windows/Linux native rendering and live Discord interoperability remain unverified.

Both final release package commands passed. Text executable: 47,910,256 bytes
(+205,968); voice executable: 50,730,160 bytes (+189,584). Complete installed/ZIP
comparisons and measurement limits are in docs/performance.md. Native screenshots
in `docs/pr-evidence/egui-main` use the unchanged synthetic `--demo --demo-profile`
fixture at 1120×760 points / 2× scale: before is the intermediate main-pin build
without system fonts, after is the final text build with system fonts. The same
profile status visibly changes from the striped missing-glyph marker to a yellow
moon. This validates native label rendering on this Mac, not every emoji sequence
or live Discord behavior. Temporary preview copies used distinct bundle IDs to
keep automation separate from the owner's running app.

## September 11, 2026 - message-history scroll stability

Baseline `fd20dc90c5bf25bce1cfc313944661f973f3c9e1`, branch `fix/scroll-jitter`,
isolated Windows worktree. The original main checkout and its untracked
`target-relocation-remainder/` were preserved. Rust 1.98.1; unchanged pinned egui,
text-only default and optional voice features. Baseline release packages were built
from a separate detached worktree before application edits.

The timeline saved its anchor before egui consumed wheel input, then restored that
outdated position after measuring new rows. Leading row measurements also shifted
already visible messages, while estimated trailing heights could leave gaps.
The fix saves the post-input anchor, measures leading rows in a clipped child whose
bounds do not move the visible content, and fills the viewport using actual row heights.
Row widget IDs remain stable across those two layouts. Existing 500-row/content-byte
bounds and caches are unchanged; no additional rendering passes or dependencies.

Synthetic egui input regression: 500 messages with compact and long wrapped rows,
900x600 and 360x600 point viewports, explicit 60 Hz timestamps, 120 upward wheel
frames, 240 downward frames and four idle frames. Maximum visible-message displacement
error was 80/76 points on the baseline and 0/0 after the fix. These are CPU layout
coordinates, not native GPU frame-time or live Discord evidence. Existing focused
timeline checks cover zoom, deletion, spoilers, keyboard actions and jumping to present.

Native Computer Use remains owner-paused from the earlier Escape interruption.
Before/after native screenshots and native process CPU/RSS sampling are therefore
unavailable; no desktop automation, live conversation or running owner build was touched.
The delivery skill requires a draft PR while this evidence is unavailable. Release
package measurements and their limits are recorded in `docs/performance.md`.

Validation: `cargo test --locked -p ui timeline::tests -- --nocapture` passed;
`cargo xtask check` passed all 360 tests (one existing opt-in voice test ignored),
formatting, strict workspace Clippy, text-only check and policy. The tall-leading-row
regression also verifies first-frame visibility and absence of phantom scroll extent.
`cargo xtask package` and `cargo xtask package-voice` passed; each executable grew by
1,024 bytes. Native capture/process metrics and live scrolling remain unverified.
The shared target initially reused an xtask binary containing the baseline worktree
path; rebuilding only the xtask cache corrected that before the successful checks
and final packages. Baseline packages and the owner's running build were preserved.

Owner-requested main integration: preserved the newer DM-ordering fix `990d1c3`;
the only conflict joined both appended progress sections. The combined
`cargo xtask check` passed 361 tests, strict Clippy, formatting, text-only check
and policy (one existing opt-in voice test ignored). Release/size evidence above
describes `d6909bd` before this integration. The owner explicitly requested pushing
to main with the previously reported pending CI and paused native evidence.

## Combined DM/group activity ordering - September 11, 2026

DMs and group DMs now share newest-message-first sidebar ordering. Incoming messages
and confirmed sends update placement; composing or pending sends do not. Existing
bounded activity cursors retain ordering across deleted latest messages and stale/null
metadata replacements. Empty conversations fall back to their channel IDs; equal
activity uses channel ID as a deterministic tie-breaker. Guild/category ordering and
selection by channel ID remain unchanged. No new storage, dependencies or network calls.

Baseline: clean task worktree from origin/main `fd20dc9`; original local main was
`2d17054` with unrelated `target-relocation-remainder/`, preserved. Rust 1.98.1.
The new regression failed against baseline ordering, then passed. Final
`cargo xtask check` passed all 359 tests, formatting, strict Clippy and policy checks;
`cargo xtask package` and `cargo xtask package-voice` passed on Windows. Independent
read-only review found no remaining blockers. Release package/replay comparisons are
recorded in `docs/performance.md` (text and voice executables each +48,640 bytes).

The existing native `--demo` baseline was inspected; Computer Use was stopped with
physical Escape before the matched before/after pair, so no visual-pair claim is made.
The temporary screenshot-only fixture was removed from the delivered change.
No live Discord session, microphone, macOS or Linux validation was performed.
The owner explicitly requested rebase and direct push to main instead of a PR.

Protocol basis checked September 11: Discord's [channel fields](https://docs.discord.com/developers/resources/channel)
provide last_message_id and [snowflake IDs](https://docs.discord.com/developers/reference#snowflakes)
encode creation time. Ordering is a local interpretation of available metadata;
synthetic tests do not prove exact official-client ordering or live interoperability.

September 11 owner-requested merge: integrated main 487069f (including scrolling/DM order); retained diagnostics and runtime xtask workspace resolution through formatting conflicts. Full `cargo xtask check` passed 0 tests, strict Clippy, text-only compilation and policy; `node tests/xtask-workspace.cjs` passed. Existing-head cross-platform CI was green. No native/live interaction performed.
