# Implementation progress — 2026-09-10

## Current scope and gates

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
include session voice gain controls and explicit external-open actions for unsupported content;
native/live acceptance and release evidence remain separate gates. Historical entries follow.

Read the original SPEC.md completely before implementation. Repository initially contained only the tracked two-line README and an untracked SPEC.md; no existing source or agent instructions were removed. The owner explicitly revised authentication and storage during implementation. The final spec now requires the official Discord login in a temporary webview, secure remembered login, and allows bounded local SQLite caches, saved drafts/settings and files. Those changes were applied throughout SPEC.md and AGENTS.md.

| Milestone | Actual status |
|---|---|
| 0 — native shell / feasibility | Native egui/eframe/wgpu app, Cargo workspace, pinned Rust, lockfile, synthetic fixture, real composition/variable-height timeline, compatibility evidence and initial tests implemented. macOS native launch verified. OS credential-store and login-method round trips remain unverified |
| 1 — real normal-user message exchange | **BLOCKED: no owner-controlled authenticated session/private live conversation was supplied or exercised.** Direct REST/Gateway adapters and own-webview credential handoff are implemented, but normal-user acceptance is not established. No real message/reply exchange with an official client is claimed |
| 2 — reliable text | Partial: bounded cache/queues, partial patches, timestamps, deletes/tombstones, late-history reconciliation, session generations, ambiguous-send state, back-pagination, cancellation, heartbeat/finite reconnect/resume, SQLite history/drafts. Scoped history failures, page validation/exhaustion, authoritative refresh and a local WebSocket lifecycle test added September 10. Full failure matrix, long process soak and live freshness recovery remain open |
| 3 — everyday messaging | Partial native text UI, server categories/icons, loaded thread/forum-post navigation and archived-thread browsing, bundled Unicode emoji and server emoji picker, grouped timeline and hover actions, native embeds/static images, bounded CommonMark formatting, spoiler concealment, explicit link confirmation, CJK/Arabic fallback fonts, copy/reply/edit/delete controls, clickable user/channel mentions and autocomplete, service profiles, reaction counts/add/remove controls, image viewing and general attachment downloads, single-file picker/drop uploads, conversation search, explicit remote read markers, paginated pinned-message browsing, history clear/logout and saved theme. Discord Markdown parity, multiple-file uploads, animation, pin mutations, notifications, complete active-thread discovery/create/join controls and actual IME/screen-reader tests remain open |
| 4 — voice | **Partial; live gate blocked.** Optional one-to-one DM and guild voice UI/signaling, bounded participant rosters, native CPAL/Opus mixed playback and DAVE group encryption implemented. Synthetic crypto/transport/mixer tests pass. No real Discord call, physical microphone/speaker, device-permission or cross-platform audio validation |
| 5 — release | Partial: docs, dual licenses, dependency inventory, xtask, CI matrix and locally ad-hoc-signed macOS package. Strict audit warnings, Windows/Linux execution, signing/installer work, complete transitive license-text packaging and performance/platform gates remain open |

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
