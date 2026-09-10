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

## September 10: reaction refresh and pinned-message browsing (Windows)

Baseline 74c0d79 versus this reaction/pins change; both text and voice packages rebuilt with pinned Rust 1.98.1, the existing release profile and lockfile on Windows 11 Home 10.0.26200, AMD Ryzen 7 7800X3D (16 logical processors), approximately 31 GiB visible RAM. No dependencies changed. These are unsigned local packages, excluding the separate OS WebView2 runtime and drivers. Full package and ZIP sizes below were measured immediately after packaging, before this measurement note was added to repository documentation. Text packaging excludes the sibling voice directory; both exclude docs/pr-evidence. ZIP uses Python zipfile DEFLATE level 9 over the entire respective package.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Text executable, bytes | 41,813,504 | 41,835,008 | +21,504 (+0.051%) |
| Voice executable, bytes | 45,160,448 | 45,182,976 | +22,528 (+0.050%) |
| Text installed package, bytes | 42,033,731 | 42,060,801 | +27,070 (+0.064%) |
| Voice installed package, bytes | 45,604,233 | 45,632,327 | +28,094 (+0.062%) |
| Text ZIP, bytes | 24,370,088 | 24,380,566 | +10,478 (+0.043%) |
| Voice ZIP, bytes | 25,737,983 | 25,746,892 | +8,909 (+0.035%) |
| Reducer replay median, milliseconds | 26.2139 | 26.3168 | +0.1029 (+0.39%) |
| Retained timeline estimated bytes | 220,992-221,477 | 220,992-221,477 | Unchanged, 500 records |

Replay method: `cargo build --release --locked -p replay-bench`, then run the produced executable once for warmup and five measured times on each revision. Baseline runs: 26.4021, 26.2139, 25.7050, 26.6599, 25.9837 ms; after: 26.1046, 28.5302, 26.8548, 26.3168, 26.2875 ms. Each run reduces 100,000 synthetic events and checks bounded state/logout. The small median difference is noise, not an optimization claim. This workload does not measure the pins endpoint, actual network latency, UI frame time, process RSS or voice audio.

Native baseline only: rebuilt text --demo, default dark 1122x792 captured window, wgpu renderer; selected adapter and display scale were not independently measured. After more than 30 seconds settling and one synthetic reaction toggle, 16 Get-Process samples at one-second intervals covered 15.247 seconds. Working set stayed 198,946,816 bytes, private bytes 412,827,648; CPU delta was 0.046875 seconds (0.307% of one core). This baseline exceeds the initial 150 MiB active-text working-set target. Whole-system contention, GPU allocations, helper-process usage, startup/frame p95 and voice memory were not measured. The owner stopped Computer Use before after-build interaction; no comparable after native-memory/CPU sample or performance improvement is claimed.

## September 10: single-file uploads (Windows)

Baseline e99bc8817454335789b84140c2998394bca899cf versus this attachment change. Same Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical processors), approximately 31 GiB visible RAM, pinned Rust 1.98.1 and release profile. The baseline text/voice packages were copied separately after their executable hashes matched the reviewed baseline builds. Both changed variants were packaged successfully. New native tokio-util 0.7.19 comes from enabling reqwest streaming; its MIT license is included. No fonts/codecs changed. These unsigned development-package measurements exclude the OS WebView2 runtime and drivers. Installed totals and ZIPs reflect staging before this final progress/performance note was added; text excludes its sibling voice directory and both exclude PR evidence. ZIP: all respective package files, Python zipfile DEFLATE level 9.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Text executable, bytes | 41,835,008 | 42,109,440 | +274,432 (+0.656%) |
| Voice executable, bytes | 45,182,976 | 45,423,616 | +240,640 (+0.533%) |
| Text installed package, bytes | 42,060,801 | 42,345,271 | +284,470 (+0.676%) |
| Voice installed package, bytes | 45,632,327 | 45,884,089 | +251,762 (+0.552%) |
| Text ZIP, bytes | 24,380,566 | 24,476,080 | +95,514 (+0.392%) |
| Voice ZIP, bytes | 25,746,892 | 25,829,810 | +82,918 (+0.322%) |
| Reducer replay median, milliseconds | 26.8048 | 26.5324 | -0.2724 (-1.02%, noise) |
| Retained timeline estimated bytes | 220,992-221,477 | 220,992-221,477 | Unchanged, 500 records |

Replay: build release replay-bench once per revision; one warmup plus five direct executable runs, 100,000 synthetic events each. Baseline: 26.5979, 28.1066, 26.8048, 26.5815, 27.4200 ms. After: 26.2354, 26.3576, 27.3465, 28.1847, 26.5324 ms. This reducer workload does not measure file transfers, network throughput, process memory, UI latency or voice.

Native baseline only: text --demo, dark 1122x792 window, wgpu; selected adapter/display scale unmeasured. After more than 30 seconds idle settling, 16 Get-Process samples at one-second intervals covered 15.273 seconds. Working set peaked at 158,564,352 bytes and settled at 158,547,968; private bytes peaked at 394,444,800 and settled at 394,412,032. CPU delta was 0.046875 seconds (0.307% of one core). The baseline was idle, with no picker/upload interaction. The owner stopped Computer Use before changed-build inspection, so no comparable after sample, upload-load memory, frame/startup p95, GPU/helper-process or voice-memory measurement is available. No native performance improvement is claimed.

Resource ceilings: one selected file/job, file <=20,000,000 bytes, 64 KiB application chunks, latest-value progress, staging/storage responses <=64 KiB, local path <=4096 encoded bytes and filename <=256 UTF-8 bytes. Library, TLS, OS and driver buffers are additional. This avoids a deliberate whole-file allocation but is not a measured process-memory ceiling. Pending filename metadata shares existing draft/pending admission budgets; no upload disk snapshot or new database table exists.

### Updated attachment packages including drag-and-drop

Same reference host, Rust profile and original e99bc88 baseline above; both Windows package variants rebuilt after adding drop selection. No dependency or reducer source changed in this continuation. Incremental executable growth over picker-only b83b41a is 28,160 bytes text (0.067%) and 26,624 bytes voice (0.059%). The complete PR comparison is:

| Metric | Original baseline | Picker and drop | Delta |
| --- | ---: | ---: | ---: |
| Text executable, bytes | 41,835,008 | 42,137,600 | +302,592 (+0.723%) |
| Voice executable, bytes | 45,182,976 | 45,450,240 | +267,264 (+0.592%) |
| Text installed package, bytes | 42,060,801 | 42,382,555 | +321,754 (+0.765%) |
| Voice installed package, bytes | 45,632,327 | 45,918,753 | +286,426 (+0.628%) |
| Text ZIP, bytes | 24,380,566 | 24,486,146 | +105,580 (+0.433%) |
| Voice ZIP, bytes | 25,746,892 | 25,838,616 | +91,724 (+0.356%) |
| Reducer replay median, milliseconds | 26.8048 | 27.9813 | +1.1765 (+4.39%, noisy) |

Package totals use the same full-file/DEFLATE-9 method and exclusions, measured before this final progress/performance addendum was staged. After release builds finished, the existing release replay executable (reducer source unchanged) was run once for warmup plus five samples: 26.2423, 28.5555, 32.6936, 27.9813, 26.8632 ms. Retained state remains 500 messages and 220,992-221,477 estimated bytes. The spread and unchanged reducer make this a noisy observation, not an algorithmic regression or optimization claim; drop throughput and UI latency were not measured.

The native picker-only parent composer was inspected, then Computer Use stopped with physical Escape before chooser interaction. No native drop-build process sample, final screenshot or actual OS drag/drop measurement is available. Admission moves egui handles once, retains at most one bounded local path, and reuses asynchronous metadata validation; it never calls the handle's whole-file bytes method. All original upload resource ceilings remain unchanged.
