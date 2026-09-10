# Initial performance evidence

Measured 2026-09-09 on an Apple M1 Pro (8 CPU cores), 16 GiB RAM, macOS 27.0 beta build 26A5425a, arm64, Rust 1.98.1. These are initial samples, not a completed acceptance benchmark.

| Workload / metric | Actual result | Limits |
|---|---|---|
| Synthetic reducer replay | 100,000 events in 21.99 ms, release | Not transport, parsing, UI frame time or live messaging |
| Retained replay timeline | 500 messages; 156,992–157,477 estimated entity bytes over the last 100 cycles | Demonstrates the application timeline bound, not process RSS/GPU stability |
| Native synthetic window | Debug build launched and was interactive; keyboard-composed synthetic message appeared in timeline | No p95 startup measurement; no live account |
| Settled synthetic process | `ps` RSS 118,896 KiB (~116.1 MiB), sampled CPU 0.0% | Debug, 50-message synthetic page plus one send; wgpu/Metal; 1120×760 requested logical window, observed screenshot 1088×768. Display scale not instrumented |
| Release/package size | Recorded in progress.md after packaging | System webview/OS frameworks are external footprint; no audio engine |

The sampled debug RSS exceeds the 80 MiB settled text-idle target and is below the 150 MiB active-channel starting target, but these workloads are not the authenticated acceptance workload. No pass is claimed for either. Shader/GPU allocations, driver components, login webview helper processes, cold/warm login bandwidth, 60 Hz scroll frame p95, cached-navigation p95, and long-running process-memory soak are unmeasured. The fixture path does not open WebKit, keyring or SQLite; those costs need separate controlled-session measurement.

The release profile uses thin LTO, one codegen unit and stripped executable debug information with separate line-table symbols. The first optimized build took 2m39s in this environment while a developer audit tool also compiled; build time is not startup time.

Run `cargo replay` for the reproducible synthetic reducer workload. Use `cargo xtask package` for the actual staged package, then measure the native process with platform tools. Targets in SPEC.md remain targets. No voice or image-preview resource claims are made.


September 10 continuation: reducer replay measured 19.791708 ms for the same 100,000 synthetic events, retaining 500 records / 156,992–157,477 estimated bytes. The package now embeds 16.51 MiB of licensed font data; its local ad-hoc-signed macOS executable is 36,933,088 bytes (~35.2 MiB). Formatting has an 8192-byte/128-line per-message input ceiling and a 64-entry/1 MiB estimated source/parsed-span cache. Spoiler consent stores original text bounded by the active timeline. These additions have no new process RSS, GPU or p95 frame measurements yet; the September 9 memory sample must not be reused as their measured footprint.

September 10 avatar/member addition: active member data is capped at 100 list positions / 128 KiB, separate from navigation and messages. Avatar textures are capped at 64 × 128 × 128 × 4 = 4 MiB; eight queued decoded outputs add at most 512 KiB. One image worker holds a compressed body capped at 512 KiB and decodes with dimensions at most 256×256 and a 1 MiB allocation limit before resizing to 128×128. Disk metadata is streamed with 32 eviction candidates, rather than retaining a full RAM index. Avatar disk allowance is deliberately generous (1 GiB / 4096 files per account) to reduce repeat network work. These are tested component ceilings, not whole-process RSS, GPU-driver totals or long-running measurements. No new codec dependency or bitmap asset was added.

macOS arm64 release packaging passed with strict ad-hoc signature verification. Executable: **37,443,712 bytes** (35.71 MiB), up **359,296 bytes** (0.97%) from the preceding UI build. No Developer ID/notarization or Windows/Linux packaging validation is claimed.

## September 10: optional one-to-one DM voice

Same macOS arm64 host/toolchain, release thin-LTO build. Text executable: **37,634,384 bytes** (35.89 MiB). Voice executable: **40,491,296 bytes** (38.62 MiB), **+2,856,912 bytes / 7.59%** over the current text build. Both local packages passed strict ad-hoc signature verification; neither is notarized. `otool -L` selects system frameworks and libraries only, with no Homebrew/libopus dynamic-library dependency. The codec is statically bundled. Voice packages also include codec/crypto notices and the modified MPL HPKE component source.

The voice-enabled release launched natively with `--demo`; the DM header exposed Call/Audio, Call was disabled, and the Audio menu's accessibility text reported that microphone/speakers are unavailable in preview. This is an offline startup/UI check, not physical audio or Discord validation. The preview was closed after inspection.

Call media budgets include four eight-frame PCM rings/queues (20 ms, 960 mono f32 samples each: 120 KiB sample storage total), eight encoded jitter packets (at most 10,200 bytes), 64 KiB voice WebSocket messages and 4 KiB UDP input packets. Codec, MLS, device/driver, task and operating-system allocations are additional. Call audio threads/devices stop on leave; the next call waits for native teardown completion. These are component limits and ownership checks, not measured call RSS or a hardware teardown result. No active-call CPU, RSS, latency, audio-quality, device-change or long-duration measurement is claimed.

## September 10: categories, embeds and server icons

Same macOS arm64 host/toolchain. Text executable: **37,972,480 bytes** (36.21 MiB), +338,096 bytes over the preceding text build. Voice executable: **40,844,544 bytes** (38.95 MiB), +353,248 bytes over the preceding voice build. Both packages passed strict local ad-hoc signature verification. The lockfile has no new package entries, only direct edges to existing SHA-256/URL/serde dependencies. No new image codec, font or bitmap asset was bundled.

The shared avatar/icon/embed texture allowance is now 64 entries / 16 MiB RGBA, replacing the preceding avatar-only 4 MiB bound. Two decoded-result slots add at most 2 MiB. One worker holds at most a 2 MiB encoded response and uses an 8 MiB PNG decoder allocation limit for embeds (512 KiB / 1 MiB for avatars/icons); dimensions and resized outputs have separate limits. Disk remains 1 GiB / 4096 files / 90 days per account. These are component ceilings, not whole-process RSS or GPU measurements.

Embed metadata is limited to 10 cards, 25 fields/card and 64 KiB/message, counted within the existing 4 MiB timeline and 1 MiB pending-patch limits. Parsed message/embed sections share the existing 64-entry / 1 MiB formatting cache. Spoiler consent retains a bounded copy of active revealed message content and embed metadata. SQLite eviction includes serialized embed bytes and preserves its 64 MiB database ceiling. Navigation remains limited to 4000 items / 4 MiB metadata; collapsed category keys are pruned with navigation.

A native voice-build offline preview verified complete card/image layout, category collapse/selection visibility, generated server icons and external-link confirmation at approximately 1088×768. No production CDN request, authenticated workload, process-memory/frame-time benchmark, hardware call or Windows/Linux run was measured. Earlier RSS samples do not represent these additions.

September 10 inline-link correction: locally ad-hoc-signed text executable **37,988,976 bytes**; voice **40,844,624 bytes**. The existing parser and native egui link widgets are reused, with no new dependencies/assets. Message and embed text keep their shared bounded formatting cache. Native synthetic layout and pointer checks passed; no new RSS/frame-time or live-service measurement was taken.

September 10 chat-image attachments: text executable **38,030,064 bytes** (+41,088); voice **40,902,080 bytes** (+57,456) over the preceding inline-link builds. Both passed strict local ad-hoc signature verification. No new dependency/codec/bitmap asset was added. Attachment metadata is capped at 10 / 64 KiB per message within existing timeline/event/pending-patch budgets; typed cache JSON is capped at 256 KiB and counted in SQL eviction. The shared 16 MiB/64-texture media allowance, two decoded-result slots and disk cache limits are unchanged. Inline images fit 420×280 and the enlarged viewer uses the existing at-most-512-pixel output, reusing the same texture. Native synthetic layout/viewer checks passed; no actual image-download or whole-process RSS/frame-time benchmark was measured.

## Mentions, profile cards and native image saving

September 10 follow-up, same macOS arm64 host/toolchain. The lockfile adds rfd 0.17.2 and pollster 0.4.0 for native Save As (423 non-workspace packages in the all-feature macOS metadata inventory). Both package variants include their license texts. No new decoder, font or bundled bitmap was added.

One profile retains at most 64 KiB metadata after a 256 KiB response cap. Banners reuse static 512-pixel textures; enlarged attachment viewing reuses/upscales the same 512-pixel preview without allocating a full-resolution texture. Mention suggestions inspect at most 256 locally known users and show eight rows. Existing 16 MiB/64-texture, timeline and disk budgets remain in force. REST admits at most four concurrent requests. Explicit original image saves allow one transfer, stream transport chunks through at-most-32-KiB file writes with coalesced progress, and cap the original at 100 MiB; they do not decode or buffer an entire image.

Native macOS offline checks exercised mention insertion/profile activation, banner card layout and scrolling, and enlarged image controls. Native Save As interaction and live CDN requests were not exercised; localhost tests establish transfer/cancellation/file replacement behavior only. No whole-process RSS, idle CPU or frame-time benchmark was performed for these additions.

Final staged executable sizes: text **38,374,272 bytes** (36.60 MiB), +344,208 bytes over the preceding attachment build; voice **41,248,800 bytes** (39.34 MiB), +346,720 bytes. Both variants passed strict local ad-hoc signature verification; these are development packages, not notarized releases.


## September 10 — chat timeline comparison

Baseline is origin/main **74c0d79**, rebuilt in a detached worktree with a separate target
and package directory. After is the final chat-timeline source including the edited-label
correction. The earlier e9fb3e4 binaries and measurements were superseded after integration.
macOS 27.0 (26A428), Apple M1 Pro, 16 GiB RAM, Rust 1.98.1, arm64 release/thin LTO.
Both text and optional voice variants were built with the lockfile, packaged and verified
with strict local ad-hoc codesign. Archives contain the complete .app, including notices/docs;
logical file-byte sums exclude filesystem allocation overhead. Package snapshots precede
this final measurement report; the voice snapshot includes the new scope/progress docs.

| Metric | Main 74c0d79 | After | Delta |
| --- | ---: | ---: | ---: |
| Text executable bytes | 38,604,512 | 38,622,688 | +18,176 (+0.047%) |
| Text full .app bytes | 38,832,679 | 38,850,855 | +18,176 (+0.047%) |
| Text tar.gz bytes | 23,271,482 | 23,277,295 | +5,813 (+0.025%) |
| Voice executable bytes | 41,445,968 | 41,464,160 | +18,192 (+0.044%) |
| Voice full .app bytes | 41,905,096 | 41,929,046 | +23,950 (+0.057%) |
| Voice tar.gz bytes | 24,587,808 | 24,600,845 | +13,037 (+0.053%) |
| Text settled RSS | 113,568 KiB | 113,712 KiB | +144 KiB (+0.127%) |
| Text peak sampled RSS | 113,760 KiB | 113,808 KiB | +48 KiB (+0.042%) |
| Reducer median, 100,000 events | 27.005 ms | 27.224 ms | +0.219 ms (+0.81%) |
| Text median idle CPU | 0.0% | 0.0% | 0.0 percentage points |

Reducer: one discarded warmup plus five direct executable runs per revision, after builds
finished. Both retained **500 records / 220,992–221,477 estimated timeline bytes**. The 0.81%
elapsed difference is a small noisy change, not evidence of a throughput improvement or
meaningful regression. This workload does not exercise GUI layout.

Process: each text .app launched explicitly with `--demo`, same unchanged mixed-content
fixture and default launch options; 10-second warmup, ten `ps -p PID -o rss=,%cpu=` samples
at one-second intervals, then terminated. RSS is process resident memory; peak is only the
sampled interval, not startup peak. Both samples exceed the 80 MiB aspirational logged-in-idle
target, but these are offline mixed-media previews, not logged-in idle. No interaction was
scripted: native automation failed with `Sky Computer Use native pipe startup failed`.
wgpu is configured; actual backend, display scale, viewport, GPU/OS-helper memory, interactive
scroll peaks and p95 frame/startup times could not be verified. No native visual or performance
acceptance is claimed from the launch-only memory samples. Raw local measurements and the
measurement script are in ignored `target/chat-measurements.json` and `target/measure-chat.py`.

### September 10, 2026 — Twemoji artwork

Baseline `f708cb21fcefa741ce294afb49f1d34211a8160e`; changed branch `feat/twemoji`.
Apple M1 Pro / MacBookPro18,3, 16 GiB RAM, macOS 27.0 build 26A428, arm64,
Rust 1.98.1, locked release builds. Both text and voice packages passed local
ad-hoc signature verification; neither is notarized.

| Metric (bytes) | Baseline | Twemoji | Delta |
| --- | ---: | ---: | ---: |
| Text executable | 38,622,688 | 44,792,944 | +6,170,256 (+15.98%) |
| Text installed | 38,860,167 | 45,053,460 | +6,193,293 (+15.94%) |
| Text compressed | 23,250,661 | 29,326,446 | +6,075,785 (+26.13%) |
| Voice executable | 41,464,160 | 47,634,496 | +6,170,336 (+14.88%) |
| Voice installed | 41,932,600 | 48,125,973 | +6,193,373 (+14.77%) |
| Voice compressed | 24,564,378 | 30,641,440 | +6,077,062 (+24.74%) |

Installed = sum of every file in the complete .app, including docs, licenses
and required voice source; excludes filesystem allocation overhead. Compressed =
Python tarfile gzip level 9 of that complete app. These package snapshots precede
this final measurement report. The PNG alone adds 6,002,931 executable bytes; the
fixed 2,048×2,016 RGBA atlas has 15.75 MiB pixel payload. CPU conversion, upload
copies and GPU driver overhead are additional; no new resolved Cargo package.

| Process sample | Baseline | Twemoji | Delta |
| --- | ---: | ---: | ---: |
| Median RSS (KiB) | 108,784 | 125,040 | +16,256 (+14.94%) |
| Peak sampled RSS (KiB) | 108,784 | 125,168 | +16,384 (+15.06%) |

Process samples: text-only native `--demo`, unchanged fixture plus the same local
Unicode message shown in the screenshots, default 1120×760 requested logical size
(1088×768 exported capture), dark theme; wgpu configured (Metal expected on macOS,
backend/display scale not instrumented). At least ten seconds after interaction,
ten `ps -p PID -o rss=,%cpu=` samples one second apart. RSS is resident process
memory, not GPU memory or total system cost; sampled peak is not startup peak.
The baseline's smoothed `ps %cpu` was 0.0%, after 0.8–2.3% (median 1.55%); different
process ages and recent startup/input make those CPU values unsuitable for an idle
regression conclusion. No precise interval CPU, startup/frame p95, GPU/driver/helper
memory or long-duration soak claim. Another offline preview remained open during
the final sample; only the measured PID is included. No login webview/audio helpers
were started. Synthetic results do not establish logged-in performance targets.
Raw measurements and the sampling/package-size script are in ignored
`target/twemoji-*-samples.json`, `target/twemoji-sizes.json`, and
`target/measure-twemoji.py`.
