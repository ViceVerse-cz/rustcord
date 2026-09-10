# Initial performance evidence

## Incoming typing indicators - September 10, 2026

Baseline main dff09752cb501aa5243334cd27f87a2c8c514db9 has the exact tested source tree of
reply-target PR #34. Its text/voice executables were separately copied and hash-verified before
root integration edits; package directories and the baseline replay executable were preserved.
Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical CPUs), 31.1 GiB visible RAM,
Rust 1.98.1, release thin LTO / one codegen unit / wgpu; text and optional voice built separately.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,956,352 | 49,982,976 | +26,624 (+0.053%) |
| text installed bytes | 50,502,356 | 50,534,234 | +31,878 (+0.063%) |
| text ZIP bytes | 31,255,214 | 31,263,624 | +8,410 (+0.027%) |
| voice executable bytes | 53,314,560 | 53,336,576 | +22,016 (+0.041%) |
| voice installed bytes | 54,083,337 | 54,111,170 | +27,833 (+0.051%) |
| voice ZIP bytes | 32,627,919 | 32,640,183 | +12,264 (+0.038%) |
| initial ordinary replay median, ms / 100,000 events | 36.6831 | 38.9649 | +2.2818 (+6.22%) |
| follow-up interleaved replay median, ms / 100,000 events | 38.4538 | 37.1408 | -1.3130 (-3.41%) |
| retained timeline bytes / 500 rows | 228,992..229,477 | 228,992..229,477 | 0 |

Both unsigned Windows packages passed. One package measurement each, ZIP DEFLATE level 9,
text excluding nested voice; file counts remain 50/96 and no dependencies/notices changed.
Installed totals describe staged docs before this measurement addendum, not an extra repack.
Measured executable SHA256: text B22910243E7A9A34FE06F6887E9888284CF19998147FBCF5730ADC127AE53CC1;
voice 37409800846E9AC55A9B8DB7D07EDFB29A122C9505BC6FBB0F95F1F5D8121DFE.

Replay uses one warmup and five direct executable runs per revision. The initial +6.22%
prompted one interleaved baseline/after comparison after both package builds had finished,
also with one warmup each and five measured pairs. Its direction reversed; this short noisy
workload does not establish a speed improvement or stable regression. Follow-up baseline
times: 36.8714, 39.9938, 38.4538, 37.2671, 38.7337 ms; after: 37.1408, 36.4049, 37.3242,
38.0731, 36.9023 ms. This measures the synthetic ordinary reducer, not typing throughput,
process RSS, UI latency or Discord compatibility.

Typing adds eight fixed identity/deadline slots to State, an eight-entry coalescing gate and
a separate eight-slot fixed-payload inbox. No disk cache, worker, dependency or polling timer
was added. The UI uses at most three retained names of 40 characters each plus a remainder,
and schedules only the next displayed expiry. Native before/after screenshots, idle CPU,
RSS/GPU, p95 frame/startup timing and accessibility remain unmeasured while desktop automation
is owner-paused. Headless rendering checks are not native or live-account evidence.

## Saved reading and layout preferences - September 10, 2026

Baseline dfe9e3f8a657665009ac89bfadb5e1be7cb3f064 (PR #31) unsigned Windows text/voice
executables were copied and hash-verified before edits; their installed/ZIP size reports
were retained. Same Windows 11 Home 10.0.26200 / Ryzen 7 7800X3D (16 logical CPUs) /
about 31 GiB RAM / Rust 1.98.1 / release thin LTO, one codegen unit, wgpu.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,840,128 | 49,855,488 | +15,360 (+0.031%) |
| text installed bytes | 50,349,976 | 50,370,977 | +21,001 (+0.042%) |
| text ZIP bytes | 31,194,530 | 31,205,377 | +10,847 (+0.035%) |
| voice executable bytes | 53,196,800 | 53,213,184 | +16,384 (+0.031%) |
| voice installed bytes | 53,929,436 | 53,951,446 | +22,010 (+0.041%) |
| voice ZIP bytes | 32,570,631 | 32,576,903 | +6,272 (+0.019%) |

Both unsigned Windows packages passed. One size measurement each; ZIP DEFLATE level 9;
text excludes the voice folder. File counts remain 50/96; no dependencies or notices added.
Installed totals describe staged docs before this measurement addendum; no PR evidence is bundled.

SHA256 of measured executables:

- text: 4BC461415BC3814CD206194BBAC3D5C40E60DB7B0D64540FAF74C861A2FD3D72
- voice: 357F7BED34136B24174BC91EB0DCE675DC975C4225BB734E5F96461AF1DAED57

Settings add one fixed-size model record, one latest pending snapshot, and at most one
queued write on the existing 16-command / 16-result worker. The 300 ms debounce runs only
while a user change is pending; a failed/full queue stops retrying until another edit or
explicit Retry saving. No new worker, dependency, service request or periodic idle repaint.
SQLite schema 8 adds one scalar singleton within the existing 64 MiB database ceiling.

This changes settings persistence and native layout, not the message reducer workload.
No reducer speed claim is made. Native before/after screenshots, CPU/RSS/GPU use, frame
timing and scale-change latency remain unmeasured because desktop automation is owner-paused.
Headless geometry/control/anchor tests are synthetic and cannot establish native accessibility
or runtime performance. No native resource-performance improvement is claimed.

## Recently visited conversation windows - September 10, 2026

Baseline b4c66ac0c8f226bdfdc22d904fbde8a239975dcb (PR #29) packages were copied and hash-verified.
The baseline reducer executable and navigation harness were built and copied before core edits.
Same Windows 11 Home 10.0.26200 / Ryzen 7 7800X3D (16 logical CPUs) / about 31 GiB RAM /
Rust 1.98.1, release thin LTO, one codegen unit, wgpu.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,824,256 | 49,840,128 | +15,872 (+0.032%) |
| text installed bytes | 50,326,094 | 50,349,976 | +23,882 (+0.047%) |
| text ZIP bytes | 31,186,721 | 31,194,530 | +7,809 (+0.025%) |
| voice executable bytes | 53,180,416 | 53,196,800 | +16,384 (+0.031%) |
| voice installed bytes | 53,905,027 | 53,929,436 | +24,409 (+0.045%) |
| voice ZIP bytes | 32,560,106 | 32,570,631 | +10,525 (+0.032%) |

Both unsigned Windows packages passed. One measurement each; ZIP DEFLATE level 9; text
excludes the voice folder. File counts remain 50/96, with no dependency or notices changes.
Installed totals describe staged docs before this measurement addendum; no PR evidence is bundled.

SHA256 of measured executables:

- text: AAAEC68E80A62E89A98129A3BB342BBE2B75B6D2DEDD94B58A212809D2222145
- voice: 0F5403607ADF30286D37783FCAFF6D02BD2D65D4B634C1D7CB196D385735D8C6

One warmup and five measured direct executable runs per workload/revision:

| Metric | Baseline | Resident windows | Delta |
| --- | ---: | ---: | ---: |
| Immediate previews / 10,000 selections | 0 | 10,000 | +10,000 |
| Revalidation requests / 10,000 selections | 10,000 | 10,000 | 0 |
| Core select median, microseconds (five-run median) | 1.3 | 0.2 | -1.1 |
| Core select p95, microseconds (five-run median) | 1.7 | 0.2 | -1.5 |
| Ordinary 100,000-event replay median, ms | 37.7021 | 38.4952 | +0.7931 (+2.10%) |
| Ordinary replay retained live payload bytes / 500 rows | 220,992..221,477 | 220,992..221,477 | 0 |

Navigation: `cargo replay -- --navigation`; three synthetic 50-message DMs, three initial loads,
then 10,000 selections and handcrafted authoritative history responses. Only State::select is
timed; fixtures/responses and rendering/I/O are excluded. Baseline medians: 1.3, 1.2, 1.3, 1.3,
1.3 microseconds; p95: 1.7, 1.6, 1.8, 1.7, 1.6. Changed medians and p95 were 0.2 in all runs.
These tiny measurements are quantized and sensitive to timer overhead; this does not establish
native channel-switch p95. Every changed run retained two dormant windows / 150 total rows /
199,654 estimated allocation bytes. The accounting footer was added after the baseline capture;
the timed workload is unchanged. Baseline had no dormant windows.

Ordinary replay baseline samples: 38.6347, 36.0714, 35.9521, 37.7021, 38.2936 ms; changed:
38.4947, 39.0375, 39.4187, 38.2191, 38.4952 ms. The median is slower, with overlapping ranges;
no ordinary-message speed improvement is claimed. This is not RSS or live compatibility evidence.

Component bounds: at most two dormant windows, with active+dormant rows capped at 1,475 and
estimated allocations at 16 MiB minus 66 KiB; reserve 25 rows and 66 KiB for search/pins.
The estimate now includes row/container storage, pending patch capacity and reconciliation sets,
with a B-tree slack allowance. Individual timelines retain their 500-row / 4 MiB payload limit.
No additional worker, network request, schema or runtime dependency. Native screenshots,
CPU/RSS, GPU allocations and frame/startup/channel-switch measurements remain owner-paused.

## Deleted message state - September 10, 2026

Baseline e0f18d0d2c2d783ad85ec8793f62288aca259d31 (PR #28) packages and replay executable
were separately copied and hash-verified before edits. Same Windows 11 Home 10.0.26200 /
Ryzen 7 7800X3D (16 logical CPUs) / about 31 GiB RAM / Rust 1.98.1, release thin LTO,
one codegen unit, wgpu.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,810,944 | 49,824,256 | +13,312 (+0.027%) |
| text installed bytes | 50,304,421 | 50,326,094 | +21,673 (+0.043%) |
| text ZIP bytes | 31,173,706 | 31,186,721 | +13,015 (+0.042%) |
| voice executable bytes | 53,163,520 | 53,180,416 | +16,896 (+0.032%) |
| voice installed bytes | 53,879,770 | 53,905,027 | +25,257 (+0.047%) |
| voice ZIP bytes | 32,543,824 | 32,560,106 | +16,282 (+0.050%) |

Both unsigned Windows packages passed. One size measurement each; ZIP DEFLATE level 9;
text excludes the voice folder. File counts remain 50/96; no dependency or notices changes.
Installed totals describe staged docs before this measurement addendum; no PR evidence is bundled.

SHA256 of measured executables:

- text: 57AF34D6F35E3576E2D0C3E3F9560265ED3991863BB2D1356CE5FF7225F6804C
- voice: 6608248BC069ADD7CE9520411C9AF7C1DC832FC5139DC166F6641CECE7D38A56

The synthetic 100,000-event reducer replay used one warmup and five measured direct executable
runs per revision. Baseline: 36.4297, 37.0517, 38.0769, 36.5077, 36.9497 ms; changed:
36.0073, 37.0905, 39.5844, 40.1928, 36.6144 ms. Median 36.9497 -> 37.0905 ms,
+0.1408 ms (+0.38%); ranges overlap. Both retain 500 live records and
220,992..221,477 estimated live payload bytes. No speed or acceptance-latency claim is made.
This workload has ordinary message events, not deletion I/O, process RSS or UI frame timing.

Deleted reading rows retain an ID and an empty Option<Message> slot, without message payloads.
Eviction charges live heap plus the Option slots against the existing 500-row / 4 MiB estimate;
the live-only bytes API still returns zero for an all-deleted window. These component bounds
exclude tree allocator overhead and are not whole-process RAM measurements. Tests exercise
deleted rows at both eviction edges, payload release, late arrivals and the 1,024-deletion guard.
SQLite deletion uses one transaction for at most 100 IDs. The existing worker keeps its
16-command / 16-result queues, with at most 16 additional pending cleanup account IDs.
No new worker, dependency, schema or persistent journal is added.

Native screenshots, CPU/RSS and frame timing remain unmeasured while desktop automation is
owner-paused. No account, browser or microphone was used.

## Unsupported-content markers - September 10, 2026

Baseline 8c6976edc85cc0d5b5e86b636cfd331b4063c747 (PR #27) packages were copied and
hash-verified before edits. Same Windows 11 Home 10.0.26200 / Ryzen 7 7800X3D (16 logical CPUs) /
about 31 GiB RAM / Rust 1.98.1, release thin LTO, one codegen unit, wgpu.
Both unsigned Windows packages passed. One size measurement each; ZIP DEFLATE level 9;
text excludes the voice folder.

| Metric | Baseline | Markers | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,798,144 | 49,810,944 | +12,800 (+0.026%) |
| text installed bytes | 50,286,162 | 50,304,421 | +18,259 (+0.036%) |
| text ZIP bytes | 31,169,312 | 31,173,706 | +4,394 (+0.014%) |
| voice executable bytes | 53,150,720 | 53,163,520 | +12,800 (+0.024%) |
| voice installed bytes | 53,861,511 | 53,879,770 | +18,259 (+0.034%) |
| voice ZIP bytes | 32,541,063 | 32,543,824 | +2,761 (+0.008%) |
| replay median ms / 100,000 events | 37.0330 | 38.3804 | +1.3474 (+3.64%) |
| replay retained estimated bytes | 220,992..221,477 | 220,992..221,477 | 0 |

Replay: one warmup and five measured direct executable runs per revision, 500 retained records.
Baseline samples: 35.7539, 37.0330, 37.1835, 36.2793, 38.5828 ms. Changed samples:
37.4959, 38.3804, 36.7489, 39.1534, 38.6181 ms. The measured median is slower, with overlapping
ranges and a small sample; no speed improvement or acceptance-latency claim is made.
This existing workload measures ordinary synthetic reducer events, not poll payload decoding,
SQLite migration, process RSS or UI latency.

RAM retains five booleans; SQLite stores one integer (0..31). Presence-only parsing discards
payloads, capped at 100 array objects / 64 direct fields per object within the 4 MiB wire limit.
Pending patch accounting now includes the fixed MessagePatch struct as well as retained payloads.
No new worker, network request, runtime dependency or notices. Package file counts remain 50/96;
installed totals include staged docs before this measurement addendum, with no PR evidence files.
Native screenshots, CPU/RSS, frame timing and live behavior remain unmeasured while desktop
automation is owner-paused. No browser/account/microphone action was used.

SHA256 of measured executables:
- text: B6B6E6BCA86F29F6ECBE1B5C5AF0FA9EFD4F9ED2C480C23E02EDCB0E7C70D5DA
- voice: 06E843CBAA0ECCB099D405153CB293EC5B09CB75B4936BC2A0AC2FCB555E02DD

## Unsupported-content fallback - Windows packages, September 10, 2026

Baseline 36ab5e729571d7e42650b02ec3feefb9418652c3 (PR #26) packages were separately
copied and hash-verified before edits. The changed branch also integrates the later Gateway
fixture-only CI fix. Same Windows 11 Home 10.0.26200 / Ryzen 7 7800X3D (16 logical CPUs) /
about 31 GiB RAM / Rust 1.98.1, release thin LTO, one codegen unit, wgpu.
Both unsigned package commands passed. One size measurement per variant, ZIP DEFLATE level 9;
text excludes the voice subdirectory.

| Metric | Baseline | Fallback | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,776,640 | 49,798,144 | +21,504 (+0.043%) |
| text installed bytes | 50,255,386 | 50,286,162 | +30,776 (+0.061%) |
| text ZIP bytes | 31,159,877 | 31,169,312 | +9,435 (+0.030%) |
| voice executable bytes | 53,128,704 | 53,150,720 | +22,016 (+0.041%) |
| voice installed bytes | 53,833,119 | 53,861,511 | +28,392 (+0.053%) |
| voice ZIP bytes | 32,536,920 | 32,541,063 | +4,143 (+0.013%) |

File counts remain 50/96; no dependency or notice change. Installed totals describe actual
staged docs before this measurement addendum. Packages contain no PR evidence. Generated
routes are bounded by three u64 IDs; the existing single pending external-link slot retains
the 2,048-byte URL limit. Only visible rows create fallback controls. No new worker, queue,
network request or persistent cache is added. No reducer/cache algorithm changed, so replay
was not rerun. Native CPU/RSS, frame timing, browser-helper footprint and destination resolution
remain unmeasured because desktop automation is paused. No runtime improvement is claimed.

SHA256 of measured executables:
- text: 6D1B66CA08D75A3910633388B43B3B9B41EFCEF54DE5AA44BC7CC4C2B84E3A9C
- voice: 1A1FD426CED5D23386B6B8417A47940A8998D04364122542ECCF82D99B64A97E

## Session voice gain - Windows package comparison, September 10, 2026

Baseline 3f9aa0edcd35f82eb83d5576a63d453fc384725c (PR #24) packages were copied and
hash-verified before edits. The changed branch also integrates the subsequent Linux-only
GTK v4_10 feature/docs fix. Same Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical
CPUs), about 31 GiB RAM, Rust 1.98.1, release thin LTO, one codegen unit and wgpu.
Both unsigned package commands passed. One size measurement per variant; ZIP DEFLATE
level 9; text excludes the voice subdirectory.

| Metric | Baseline | Gain controls | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,657,344 | 49,776,640 | +119,296 (+0.240%) |
| text installed bytes | 50,127,753 | 50,255,386 | +127,633 (+0.255%) |
| text ZIP bytes | 31,117,348 | 31,159,877 | +42,529 (+0.137%) |
| voice executable bytes | 53,016,064 | 53,128,704 | +112,640 (+0.212%) |
| voice installed bytes | 53,714,247 | 53,833,119 | +118,872 (+0.221%) |
| voice ZIP bytes | 32,493,543 | 32,536,920 | +43,377 (+0.133%) |

Packages retain 50/96 files and unchanged dependency notices. Installed totals describe the
actual staged docs before this measurement addendum; neither package includes PR evidence.
Shared UI includes the new sliders in both variants; the default build still excludes the
voice transport. No runtime speed or memory improvement is claimed.

Gain adds two bounded u16 atomics and one snapshot per audio callback, with finite/clipped
PCM multiplication and no new callback allocation, lock, queue or device restart. Existing
capture buffering/resampler latency remains. Device-free tests exercise the actual callback
helpers; no reducer/cache change warrants a replay rerun. Native screenshots, CPU/RSS, frame
timing, callback latency and physical audio remain unmeasured because native desktop automation
is paused after owner Escape stops. No account or microphone was used.

SHA256 of measured executables:
- text: 77FFE0271B1EAFFC7C9431DEB85D8EBE162D89377638C2259779E6E49EF12476
- voice: 91E61133C2D7FCF0F0BDDFBC2A379102A28F790C3F4FDEA1D293079E8E5830B2

## Linux login migration - Windows package comparison, September 10, 2026

Baseline 512b5e73fa4abac4a8bbe2233ceb74f56b74cdf8 (PR #23) was copied and hash-verified before
edits. Same Windows 11 Home 10.0.26200 / Ryzen 7 7800X3D / about 31 GiB RAM / Rust 1.98.1,
release thin LTO, one codegen unit, wgpu. Both unsigned Windows package commands passed.
One size measurement per variant; ZIP DEFLATE level 9; text excludes the voice subdirectory.

| Metric | Baseline | Migration | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,658,368 | 49,657,344 | -1,024 (-0.002%) |
| text installed bytes | 50,105,951 | 50,127,753 | +21,802 (+0.044%) |
| text ZIP bytes | 31,106,640 | 31,117,348 | +10,708 (+0.034%) |
| voice executable bytes | 53,016,064 | 53,016,064 | 0 |
| voice installed bytes | 53,687,168 | 53,714,247 | +27,079 (+0.050%) |
| voice ZIP bytes | 32,480,689 | 32,493,543 | +12,854 (+0.040%) |

File counts rise from 42/88 to 50/96 because eight login notice files are now included.
Notice bytes were verified in both packages. Totals describe actual staged docs before this
measurement addendum; neither package contains PR screenshots. Small executable differences
are not a runtime speed or memory improvement claim. Windows/macOS Wry backend sources remain
unchanged. No reducer/cache/audio algorithm changed, so reducer replay was not rerun.

Linux uses one ephemeral WebKit6 session, one bounded token slot, one in-flight main-frame query
and coalesced boolean wake state. Queries are at least 100 ms apart; the event pump checks a
2-ms deadline between at most 16 callbacks. A single native callback may exceed that budget.
Linux executable/system-library footprint, web helper CPU/RSS, startup, input latency, storage
writes and teardown remain unmeasured. Native desktop automation is paused; no window was run.
The existing WSL lacks GTK4/WebKit6 development libraries; Linux CI compilation is separate.

SHA256 of measured executables:
- text: 214CB9A68E1C58DC8CAFFF7B21C07D01EF150E92434F95036516CA7865483A63
- voice: 99A34EC7A3F647B119106E22E35DE9E2FD1847631E6E8C16AFC4ADE4536ECA72

Historical measurements follow.

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

## September 10: channel references (Windows)

Baseline 14dd066abf2c15b5dc07fca9fb2d14a2a651a99c versus this channel-reference change. Baseline packages were copied separately from the verified parent builds before edits. Both changed text and voice packages passed packaging on Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical processors), approximately 31 GiB visible RAM, pinned Rust 1.98.1 and the existing release profile. No dependency/font/codec change. Unsigned development packages exclude WebView2 and drivers. Installed/ZIP totals were measured after packaging, before this final progress/performance note was staged; text excludes the sibling voice directory, both exclude PR evidence. ZIP includes all respective package files using Python zipfile DEFLATE level 9.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Text executable, bytes | 42,137,600 | 42,149,888 | +12,288 (+0.029%) |
| Voice executable, bytes | 45,450,240 | 45,462,528 | +12,288 (+0.027%) |
| Text installed package, bytes | 42,382,555 | 42,403,234 | +20,679 (+0.049%) |
| Voice installed package, bytes | 45,918,753 | 45,939,432 | +20,679 (+0.045%) |
| Text ZIP, bytes | 24,486,146 | 24,491,835 | +5,689 (+0.023%) |
| Voice ZIP, bytes | 25,838,616 | 25,845,493 | +6,877 (+0.027%) |
| Reducer replay median, milliseconds | 26.7122 | 27.6904 | +0.9782 (+3.66%, noisy) |
| Retained timeline estimated bytes | 220,992-221,477 | 220,992-221,477 | Unchanged, 500 records |

Replay: release replay-bench built per revision, one warmup plus five direct executable runs after compilation finished, 100,000 synthetic events per run. Baseline: 25.8563, 26.7122, 25.7093, 27.1198, 27.2082 ms; after: 26.3900, 27.3277, 27.6904, 27.8245, 35.7749 ms. The demo's generated message 500 gained a channel token; reducer logic is unchanged. The spread makes the timing difference inconclusive, with no optimization claim. This workload does not measure reference parsing/rendering, network, process memory or voice.

Native automation remains paused after the owner's preceding physical Escape stops. No comparable current-task native process samples, idle CPU, peak/settled memory, helper/GPU usage, actual adapter/display scale or startup/frame p95 are available. The application uses wgpu, but no renderer timing claim is made. Required native before/after screenshots and changed-build interaction remain draft blockers.

Resource limits: eight locally loaded same-server channel suggestions, 120-character suggestion labels, 64-character queries; user/channel links share 100 interactive references per formatted part. Existing 8 KiB formatter input, event/depth/link and cache limits remain unchanged. A bounded channel-label hash is calculated on state revision changes to invalidate row heights, never on unchanged idle frames. No new cache or directory lookup was introduced.

## September 10: general attachment downloads (Windows)

Baseline ae360f3f912ff15e99af5d1fe0d4f330b2545414 versus this general-file download change. The verified parent text/voice packages were copied separately before edits. Host: Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical processors), approximately 31 GiB visible RAM; pinned Rust 1.98.1, existing release profile and feature split. No dependencies/fonts/codecs changed. Both unsigned packages passed packaging. Installed/ZIP totals were taken before this final measurement/progress addendum was staged; text excludes its sibling voice directory and both exclude PR evidence. ZIP uses all respective package files with Python zipfile DEFLATE level 9. WebView2 and drivers are external.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Text executable, bytes | 42,149,888 | 42,150,400 | +512 (+0.001%) |
| Voice executable, bytes | 45,462,528 | 45,463,552 | +1,024 (+0.002%) |
| Text installed package, bytes | 42,403,234 | 42,411,458 | +8,224 (+0.019%) |
| Voice installed package, bytes | 45,939,432 | 45,948,168 | +8,736 (+0.019%) |
| Text ZIP, bytes | 24,491,835 | 24,495,907 | +4,072 (+0.017%) |
| Voice ZIP, bytes | 25,845,493 | 25,848,999 | +3,506 (+0.014%) |
| Reducer replay median, milliseconds | 26.7206 | 27.7784 | +1.0578 (+3.96%, noisy) |
| Retained timeline estimated bytes | 220,992-221,477 | 220,992-221,477 | Unchanged, 500 records |

Release replay-bench: one warmup plus five direct executable samples per revision, 100,000 synthetic events each, after compilation finished. Baseline: 26.4996, 26.7206, 26.4924, 26.8281, 28.4031 ms. After: 31.0129, 27.7784, 27.5397, 27.0537, 28.2892 ms. Reducer logic is unchanged; only generated demo message 500 gained file-attachment metadata. The small timing difference is inconclusive; this workload measures neither downloads nor UI latency.

Native automation remains paused after the owner's earlier physical Escape stops. Comparable native process-memory/CPU samples, actual Save dialog/transfer-load measurements, GPU/helpers, display scale/adapter and startup/frame p95 remain unmeasured. No screenshot or runtime performance improvement is claimed; native evidence remains a draft blocker.

The existing one-dialog/transfer slot, 100 MiB original-file cap, at-most-32-KiB disk writes, expected-length checking, latest-value progress and 15/30/300-second connection/read/overall timeouts are unchanged. General files are streamed without decoding or deliberately retaining a complete file buffer. Transport/TLS/OS buffers are additional; this is a component policy, not a measured process-memory ceiling.

## September 10: loaded thread navigation (Windows)

Baseline 985bfc6ded2740a660abeeb496720b6fd0cb8489 versus this thread-navigation change. Baseline packages were copied separately before edits and verified against parent executable hashes. Both changed unsigned Windows packages passed. Host: Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical processors), approximately 31 GiB visible RAM; pinned Rust 1.98.1, existing release profile and text/voice feature split. No dependency/font/codec change. Full installed/ZIP totals reflect documentation staged by each package invocation before this final measurement/progress addendum; the voice package also includes the latest progress text. Text excludes its sibling voice directory, and both exclude PR evidence. ZIP: all respective package files, Python zipfile DEFLATE level 9. External WebView2 and drivers are excluded.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Text executable, bytes | 42,150,400 | 42,330,624 | +180,224 (+0.428%) |
| Voice executable, bytes | 45,463,552 | 45,643,264 | +179,712 (+0.395%) |
| Text installed package, bytes | 42,411,458 | 42,598,237 | +186,779 (+0.440%) |
| Voice installed package, bytes | 45,948,168 | 46,138,097 | +189,929 (+0.413%) |
| Text ZIP, bytes | 24,495,907 | 24,571,118 | +75,211 (+0.307%) |
| Voice ZIP, bytes | 25,848,999 | 25,924,133 | +75,134 (+0.291%) |
| Reducer replay median, milliseconds | 26.5772 | 28.3909 | +1.8137 (+6.82%, noisy) |
| Retained timeline estimated bytes | 220,992-221,477 | 220,992-221,477 | Unchanged, 500 records |

Release replay-bench built per revision, one warmup plus five direct samples after compilation finished, 100,000 synthetic events each. Baseline: 28.8253, 26.1273, 29.2089, 26.4214, 26.5772 ms. After: 26.8821, 29.8107, 26.7173, 31.2080, 28.3909 ms. The overlapping spread makes the median increase inconclusive; it is reported rather than called an improvement. This existing message reducer workload does not measure thread-snapshot throughput, sidebar rendering, network, process RSS or voice. Dedicated thread reconciliation is covered behaviorally by bounded offline tests.

Native automation remains paused after the owner's preceding physical Escape stops. Before/after process samples, idle CPU, peak/settled memory, actual display scale/adapter, GPU/helpers and startup/frame p95 are unavailable. The app uses wgpu; no native performance claim or screenshot is manufactured. Native evidence remains a draft blocker.

Threads share the 4,000-entry navigation ceiling; sync metadata is capped at 2 MiB inside the 4 MiB wire cap. Reconciliation and sidebar grouping use temporary bounded ordered maps/sets without an additional persistent cache. Snapshot omission/removal clears existing account history conservatively; frequent thread churn may cause extra refetches. No background thread directory, polling loop, new database table or subscription expansion is included.

## September 10: older pinned-message pages (Windows)

Baseline 188c5557a024d8ce2e1ca2ca965289a6c7dd5559 versus this pin-pagination change. The verified parent release packages and reducer executable were copied separately before edits. Both changed unsigned packages passed on Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical processors), approximately 31 GiB visible RAM, Rust 1.98.1 and the existing release profile/text-voice split. The existing time 0.3.55 formatting feature serializes pin cursors; Cargo.lock and versions are unchanged. Installed/ZIP totals reflect package-staged documentation before this final measurement/progress addendum; text excludes the sibling voice directory and both exclude PR evidence and external WebView2/drivers. ZIP includes all respective package files with Python zipfile DEFLATE level 9.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Text executable, bytes | 42,330,624 | 42,341,888 | +11,264 (+0.027%) |
| Voice executable, bytes | 45,643,264 | 45,655,040 | +11,776 (+0.026%) |
| Text installed package, bytes | 42,598,237 | 42,620,650 | +22,413 (+0.053%) |
| Voice installed package, bytes | 46,138,097 | 46,157,505 | +19,408 (+0.042%) |
| Text ZIP, bytes | 24,571,118 | 24,580,394 | +9,276 (+0.038%) |
| Voice ZIP, bytes | 25,924,133 | 25,933,749 | +9,616 (+0.037%) |
| Reducer replay median, milliseconds | 27.5110 | 28.2131 | +0.7021 (+2.55%, noisy) |
| Retained timeline estimated bytes | 220,992-221,477 | 220,992-221,477 | Unchanged, 500 records |

Replay: one release build per revision, one warmup plus five direct samples after compilation finished, 100,000 synthetic events each. Baseline: 30.9570, 29.3369, 27.0244, 27.5110, 27.2474 ms. After: 26.7628, 29.5610, 28.2131, 26.8470, 28.5889 ms. Samples overlap; the small median increase is inconclusive. This existing reducer workload does not measure pin HTTP requests, parser throughput, UI latency, process RSS or voice.

Native automation remains paused after the owner's prior physical Escape stops. Comparable process-memory/CPU samples, actual display scale/adapter, GPU/helpers, and startup/frame p95 are unavailable. No native performance or visual-inspection claim is made. Runtime ceilings remain one replaceable 25-item / 64 KiB result page, 512 KiB response body and existing shared request task. Fixed-size timestamp cursors add no growing history or query cache. Pagination only occurs after a deliberate action.

## September 10: archived-thread browsing (Windows)

Baseline 293c72b3ab3570e904cf23b52b479509d8ee75bc versus this archive-browser change. Baseline text/voice packages and reducer executable were copied before edits; parent package hashes were reverified. Both changed unsigned packages passed. Host: Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical processors), approximately 31 GiB visible RAM; Rust 1.98.1, existing release profile and text/voice split. No dependency/font/codec change. Disk pressure moved changed build and test-temporary output to a separate E: directory; packaging now honors CARGO_TARGET_DIR, and the final voice package hash matches that target's executable. Installed/ZIP totals reflect documentation staged by packaging before this final evidence addendum. Text excludes its sibling voice directory; both exclude PR evidence and external WebView2/drivers. ZIP: all respective package files, Python zipfile DEFLATE level 9.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Text executable, bytes | 42,341,888 | 42,437,632 | +95,744 (+0.226%) |
| Text installed package, bytes | 42,620,650 | 42,726,195 | +105,545 (+0.248%) |
| Text ZIP, bytes | 24,580,394 | 24,615,435 | +35,041 (+0.143%) |
| Voice executable, bytes | 45,655,040 | 45,750,784 | +95,744 (+0.210%) |
| Voice installed package, bytes | 46,157,505 | 46,262,905 | +105,400 (+0.228%) |
| Voice ZIP, bytes | 25,933,749 | 25,969,231 | +35,482 (+0.137%) |
| Initial replay median, ms | 27.2874 | 30.2790 | +2.9916 (+10.96%) |
| Interleaved replay median, ms | 27.9277 | 27.2380 | -0.6897 (-2.47%) |
| Retained timeline estimated bytes | 220,992-221,477 | 220,992-221,477 | Unchanged, 500 records |

Replay: one release build per revision and one warmup plus five direct samples after builds finished, 100,000 synthetic events each. The initial median increase warranted a second comparison: both executables were warmed again, then sampled five times each with alternating execution order. The interleaved result did not reproduce the initial increase, and its samples overlap. No speed improvement or established regression is claimed from these short runs. This reducer workload does not measure archive transport/parser throughput, native rendering, process RSS or voice.

Initial replay median: baseline [26.1177, 29.5278, 26.0407, 28.8295, 27.2874]; after [30.279, 29.9318, 33.3689, 30.5462, 29.9439] ms.

Interleaved replay median: baseline [27.9277, 26.1154, 27.0548, 28.2489, 29.8243]; after [29.7797, 27.238, 26.9207, 30.8193, 26.5756] ms.

Native automation remains paused after the owner's prior physical Escape stops. Comparable native CPU/memory samples, actual adapter/display scale, GPU/helpers and startup/frame p95 are unavailable. No native screenshot or performance claim is made. Runtime limits are one replaceable 25-entry / 64 KiB archive page, a 512 KiB response, existing shared read worker and at most one transient channel within the account navigation budget. No growing archive cache or automatic pagination was added.

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


### September 10, 2026 — server emoji, picker and text selection

Baseline `a270437cb6c1e69030420cda4e0fb6efa957bcb0` (`feat/twemoji`); follow-up
`feat/server-emoji-picker`. Same Apple M1 Pro, 16 GiB RAM, macOS 27.0 build 26A428,
arm64 Rust 1.98.1 locked release builds. Both packages passed strict local ad-hoc
signature verification. Baseline runtime binaries were compared byte-for-byte with
the previously verified packages; their docs precede the previous final report.
Current package snapshots likewise precede this final measurement report.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Text exe bytes | 44,792,944 | 45,017,184 | +224,240 (+0.50%) |
| Text installed bytes | 45,053,460 | 45,283,476 | +230,016 (+0.51%) |
| Text compressed bytes | 29,326,284 | 29,391,804 | +65,520 (+0.22%) |
| Voice exe bytes | 47,634,496 | 47,875,680 | +241,184 (+0.51%) |
| Voice installed bytes | 48,125,973 | 48,378,556 | +252,583 (+0.52%) |
| Voice compressed bytes | 30,641,220 | 30,708,940 | +67,720 (+0.22%) |
| Text median/peak sampled RSS, KiB | 114,768 | 136,368 | +21,600 (+18.82%) |
| Text interval idle CPU | 0.0% | 0.0% | 0.0 percentage points |
| Reducer median, ms | 27.093 | 26.771 | -0.323 (-1.19%) |

Installed is the sum of complete .app file bytes; compressed is Python tarfile gzip
level 9 of the complete package, including notices/docs. No new Cargo dependency.
Unicode names add 158,704 source bytes; catalogs have per-guild 1,000-entry/256-KiB
limits within shared 4-MiB navigation storage. Custom images reuse the existing
64-entry/16-MiB decoded-texture cache and bounded credential-free media worker.

Replay: isolated baseline build and current release executable, one discarded warmup
plus five direct runs each. Both retain 500 messages / 220,992–221,477 estimated bytes.
The small elapsed difference is noise, not a throughput improvement claim; this
workload does not exercise picker layout or catalog parsing.

Process: native text `--demo`, same mixed fixture plus the same message containing
Unicode and custom markup, dark theme, 1088×768 captured viewport, default requested
1120×760 logical size. Current fixture additionally supplies two original synthetic
custom images. After interaction, ten-second warmup then eleven RSS/cumulative CPU
samples at one-second intervals; CPU is cumulative-time delta over about 10.16 seconds.
Both medians equal sampled peaks; peaks exclude startup. After had additional picker
activation attempts, so interaction histories are not identical. RSS increased 21.1 MiB;
this is a material observed increase, with allocator/font/media contributions not
isolated. It is not a claim about logged-in workloads or the exact cost of the catalog.
wgpu configured; actual backend/display scale not instrumented. No helper processes
were launched. GPU memory, frame/startup p95, open-picker scroll performance and long
soak remain unmeasured. Native automation intermittently failed to activate controls,
preventing reliable completion of picker/light/narrow interaction evidence.
Raw local samples: `target/custom-{before,after,replay,sizes}.json`;
script: `target/measure-custom-emoji.py`.


## September 10, 2026 — message hover controls

Baseline: clean `d802b2a`, rebuilt before UI edits; after: `feat/message-hover`.
Rust 1.98.1, macOS 27.0 (26A428), Apple M1 Pro, 16 GiB RAM, release text-only
(`--no-default-features`) and separate `--features voice` packages. Native runtime samples
use text-only `--demo --demo-chat`, unchanged nine-message synthetic fixture, default window
size, dark/system appearance, scroll two pages upward to the first message, pointer over
the conversation. wgpu configured; actual backend, GPU allocations and display scale
were not instrumented. Captures share the same 1087×768 window-image size.

| Metric | Baseline | After | Delta |
| --- | --- | --- | --- |
| Text executable, bytes | 45,017,184 | 45,033,392 | +16,208 (+0.036%) |
| Text package, bytes | 45,316,618 | 45,332,937 | +16,319 (+0.036%) |
| Text gzip, bytes | 29,438,513 | 29,444,125 | +5,612 (+0.019%) |
| Voice executable, bytes | 47,875,680 | 47,875,472 | -208 (-0.000%) |
| Voice package, bytes | 48,406,075 | 48,405,978 | -97 (-0.000%) |
| Voice gzip, bytes | 30,757,714 | 30,759,656 | +1,942 (+0.006%) |
| Median sampled RSS, KiB | 124,088.00 | 116,720.00 | -7,368.00 |
| Peak sampled RSS, KiB | 124,112.00 | 116,848.00 | -7,264.00 |
| Mean sampled CPU, % | 0.36 | 4.37 | +4.01 |

Process samples: ten `ps -p PID -o %cpu=,rss=` samples at one-second intervals,
at least ten seconds after interaction. RSS is resident memory, not physical footprint;
peaks cover only this sample window, not startup or a soak. Baseline PID 49815, after
PID 52432; no authentication webview/audio helper was launched by either demo. Other
app instances and system processes are excluded. Sampling occurred on a shared desktop
while voice packaging could still run. The after CPU samples range 0–14%, versus 0–3.6%
baseline: this noisy sample does not establish idle-CPU improvement or steady-state
regression. Lower sampled RSS is not a whole-process memory budget guarantee. Startup,
p95 frames, GPU memory, live account and voice-call performance remain unmeasured.

Package sizes sum all regular files in Serein.app plus the four accompanying root
README/license/notice files. Compressed sizes use `tar -czf` on those same paths, excluding
other distribution variants and pre-existing archives. Snapshots precede this measurement
report; packaging source and runtime assets are unchanged. Raw samples, sizes and archives
remain under ignored `target/message-hover-*`. Text grows by 16,208 executable bytes;
voice changes by −208 bytes. No new dependencies or cache/queue budgets.


## September 10, 2026 — guild voice

Baseline: clean `main` / `619071c`, fetched origin/main, Rust 1.98.1. Changed branch:
`feat/guild-voice`. Both revisions use macOS 27.0 (26A428), Apple M1 Pro (8 cores),
16 GiB RAM, arm64 locked release thin-LTO builds. Both text (`--no-default-features`)
and optional `voice` packages were built and locally ad-hoc signature verified.

| Metric / method | Baseline | After | Delta |
| --- | --- | --- | --- |
| Text executable, bytes | 45,033,392 | 45,176,320 | +142,928 (+0.32%) |
| Text package, bytes | 45,314,977 | 45,462,380 | +147,403 (+0.33%) |
| Text zip, bytes | 29,471,417 | 29,529,234 | +57,817 (+0.20%) |
| Voice executable, bytes | 47,875,472 | 48,060,368 | +184,896 (+0.39%) |
| Voice package, bytes | 48,388,018 | 48,577,389 | +189,371 (+0.39%) |
| Voice zip, bytes | 30,848,320 | 30,920,827 | +72,507 (+0.24%) |
| Reducer median, ms | 26.700 | 27.443 | +0.743 (+2.78%) |
| Idle CPU, % | 0 | 0 | +0 percentage points |
| Settled RSS, KiB | 100,448 | 105,120 | +4,672 (+4.65%) |
| Sampled peak RSS, KiB | 100,448 | 105,280 | +4,832 (+4.81%) |

Package = logical sum of every file in the installed .app, including bundled notices,
docs and modified HPKE source for voice; excludes filesystem allocation overhead.
ZIP = `ditto -c -k --sequesterRsrc --keepParent` of that same .app. Package snapshots
precede this measurement report and the final progress/PR notes. No dependency or
lockfile change. These are development artifacts, not notarized releases.

Reducer: build once per revision (`cargo replay`), invoke each executable directly,
one warmup plus five runs of 100,000 synthetic events. Retained timeline range stays
220,992–221,477 estimated bytes / 500 records. Baseline median 26.700 ms vs 27.443 ms
is a small noisy slowdown, not an improvement claim or a voice-workload measurement.

Native process: voice-enabled `--demo`, WGPU, 2× scale, approximately 1088×768 captured
viewport, dark theme, standard fixture. Navigate from getting-started to long-form,
leave People pane visible, wait 60 seconds after interaction, then 30 `ps -p PID -o
%cpu=,rss=` samples at one-second intervals. Both native processes sampled on the same
host; baseline foreground and changed window background during the quiet interval.
The changed process also visited the empty voice page before returning to text, so
its additional retained UI work is included. RSS is resident process memory, not
physical footprint or GPU allocation. Earlier transient samples were discarded before
this controlled interval. No helper child processes were launched by the demo; shared
OS/WindowServer allocations are not attributed. Foreground placement and shared-machine
noise limit interpretation; the 4,672 KiB settled RSS increase is not a precise live-call
cost. Startup, p95 frames, GPU memory, hardware audio and live voice RSS remain unmeasured.

New component workload: `cargo test --locked -p discord-voice --release
synthetic_mix_workload -- --ignored --nocapture` passed. Five measured 1,000-tick runs
after one warmup, with real Opus decoding and independent jitter state, gave medians:

| Remote speakers | Total for 1,000 ticks | Mean time per 20 ms tick within median run |
| --- | --- | --- |
| 1 | 16.027 ms | 16.03 µs |
| 8 | 125.254 ms | 125.25 µs |
| 63 | 1,002.216 ms | 1,002.22 µs |

The baseline rejects multi-party voice, so there is no working baseline group mixer
comparison. This synthetic computation excludes encryption, sockets, devices, callback
scheduling and real-time latency. The 64-total-person limit retains at most 63 decoder
states, 63 × 10,200 encoded jitter bytes and 63 × 23,040 decoded PCM bytes, plus codec,
MLS and native allocations. Eight-frame device queues remain bounded. See the adapter
README for packet, transition and timeout limits. No acoustic echo cancellation was added.


## Server people subscription repair — September 10, 2026

Clean baseline `dc48391` vs `fix/server-people-list`; Rust 1.98.1, macOS 27.0
(26A428), Apple M1 Pro, 16 GiB RAM. Both release configurations built using
`cargo xtask package` and `cargo xtask package-voice`, including strict ad-hoc
signature verification. Baseline artifacts were preserved in
`target/people-baseline`; changed distribution measurements in `target/people-after`.
Installed bytes sum regular bundle files, archives use `tar -czf` per bundle;
package totals include the documentation snapshot present at packaging time.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Text executable bytes | 45,176,320 | 45,176,288 | -32 (-0.000%) |
| Text installed bytes | 45,470,755 | 45,473,718 | +2,963 (+0.007%) |
| Text tar.gz bytes | 29,505,591 | 29,506,659 | +1,068 (+0.004%) |
| Voice executable bytes | 48,060,368 | 48,060,336 | -32 (-0.000%) |
| Voice installed bytes | 48,585,764 | 48,588,727 | +2,963 (+0.006%) |
| Voice tar.gz bytes | 30,834,259 | 30,835,849 | +1,590 (+0.005%) |
| Reducer median ms | 26.474292 | 27.127792 | +0.653500 (+2.468%) |

`cargo replay` built the reducer workload, followed by one direct warmup and five
measured direct executions per comparison. Both retained 220,992–221,477 estimated
timeline bytes and 500 records. The reducer does not exercise Gateway subscriptions;
its unchanged executable measures host timing noise here, not a fix-related speedup.
The +0.654-ms median difference is not evidence of a Gateway regression. Full installed
and archive deltas also include documentation, signatures and archive metadata.
No dependency changes; both stripped executable sizes decreased by only 32 bytes.
UI latency, process RSS/CPU and live event rates were not measured: this patch changes
wire subscription control only. Receiving the required guild typing subscription can
increase incoming event traffic; Serein discards unhandled typing events and clears the
subscription when the member pane closes. No live performance claim is made.

## Composer, inline editing and notifications — September 10, 2026

Baseline `efa724b` versus `feat/composer-notifications`, locked release builds on
macOS 27.0 arm64, Apple M1 Pro (8 CPU cores), 16 GB RAM, WGPU/Metal. Text and
optional voice packages were built separately with `cargo xtask package` and
`cargo xtask package-voice`. Installed bytes sum regular files inside each `.app`;
compressed bytes use `ditto -c -k --sequesterRsrc --keepParent`. These are local
ad-hoc-signed packages; archive metadata can introduce small variation.

| Metric | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| Text executable bytes | 45,176,288 | 45,521,920 | +345,632 (+0.77%) |
| Text installed bytes | 45,476,218 | 45,842,027 | +365,809 (+0.80%) |
| Text compressed bytes | 29,533,549 | 29,690,224 | +156,675 (+0.53%) |
| Voice executable bytes | 48,060,336 | 48,384,320 | +323,984 (+0.67%) |
| Voice installed bytes | 48,591,227 | 48,937,514 | +346,287 (+0.71%) |
| Voice compressed bytes | 30,925,091 | 31,074,546 | +149,455 (+0.48%) |
| 100,000-event reducer median | 27.854 ms | 30.185 ms | +2.332 ms (+8.37%) |
| Retained timeline estimate | 220,992–221,477 B / 500 records | same | unchanged |

Replay: one warmup and five measured direct binary runs per revision, builds
completed beforehand. Baseline measured runs: 28.682, 27.871, 27.854, 27.097,
27.139 ms; after: 35.392, 33.829, 30.185, 30.032, 30.069 ms. The observed
regression includes the new bounded activity tracking (~23 ns/event at these
medians). It is a synthetic reducer measurement, not UI latency, live throughput
or RSS; concurrent desktop work and short durations add noise.

Native sampling uses the same offline chat, nominal 1120×760 viewport, 2× display
scale, external 4096×2304 display and draft `Hi <@2> 👋 🎉 <:serein_leaf:9001>`.
After at least 60 seconds of warmup, `ps` samples RSS and CPU every second for
10 samples. The final validation copy changes only the default demo argument
selection and bundle identity to prevent accidental authenticated auto-launches;
shipping package size/replay numbers above use the normal builds. System alerts
are disabled for this memory scenario. RSS excludes GPU allocations and other
OS processes; no app helper process is spawned for this scenario. Startup peak,
physical footprint, p95 frame time and OS notification service memory are unmeasured.

Native RSS samples were 113,088 KiB throughout baseline and 135,648 KiB throughout
after: +22,560 KiB (+19.95%). This is the maximum and settled RSS within the
10-second sampling window, not a startup peak. CPU median was 0.0% for both; after
had two samples at 1.7% and 2.1% (mean 0.38%), baseline all 0.0%. This shows higher
retained process memory for the rich composer scenario; no memory improvement is
claimed. The package measurements precede these final evidence paragraphs; bundled
documentation/archive metadata can change the distribution total slightly.
## September 10: integration of all open PRs (Windows)

The owner explicitly requested merging every open PR into main. Baseline is the verified feature-stack tree 0f67e69757983c52e1a45c03030a09e7c16a3b30 (same tree as consolidated 9e069ca), copied before integration. After combines that stack with main efa724b, including the already-merged Twemoji artwork/picker, grouped/hover timeline, guild voice and People subscription repair. These deltas measure the combined feature set against the incoming stack, not an individual feature or a comparison against main alone.

Both unsigned Windows packages passed. Same Windows 11 Home 10.0.26200, Ryzen 7 7800X3D / 16 logical processors, about 31 GiB RAM, Rust 1.98.1 and existing release profile; artifacts and test temporary files used the E: build directory. Text excludes the sibling voice directory; both exclude PR evidence and external WebView2/drivers. Installed/ZIP sizes reflect packaging before this final evidence addendum; ZIP uses all respective files with Python DEFLATE level 9. License files from both branches are preserved and packaged.

| Metric | Feature stack before integration | Integrated | Delta |
| --- | ---: | ---: | ---: |
| Text executable, bytes | 42,437,632 | 49,067,008 | +6,629,376 (+15.621%) |
| Text installed package, bytes | 42,726,195 | 49,431,699 | +6,705,504 (+15.694%) |
| Text ZIP, bytes | 24,615,435 | 30,843,886 | +6,228,451 (+25.303%) |
| Voice executable, bytes | 45,750,784 | 52,400,128 | +6,649,344 (+14.534%) |
| Voice installed package, bytes | 46,262,905 | 52,988,377 | +6,725,472 (+14.538%) |
| Voice ZIP, bytes | 25,969,231 | 32,209,427 | +6,240,196 (+24.029%) |
| Reducer median, ms | 27.5046 | 26.5682 | -0.9364 (-3.40%) |
| Retained timeline estimate | 220,992-221,477 bytes | Same | 500 records |

One release reducer executable per revision, one warmup plus five direct measured runs, 100,000 synthetic events each. Baseline samples: [27.4948, 27.5046, 26.2998, 28.5269, 29.6689] ms. Integrated: [26.4394, 27.3291, 27.4944, 26.3761, 26.5682] ms. This short reducer benchmark is not native frame time, process RSS, network or audio performance; no user-visible speed claim is made. Existing per-feature native evidence is retained from its original PRs. New combined native inspection/process samples remain unverified while this session's desktop automation is paused.


## Channel visibility - September 10, 2026 (Windows)

Baseline main 92e82e7b72c716340a20c2643069c482933acd4e has the identical source tree to
tested integration a2af8d8; its verified text/voice package executables were copied into a
separate baseline before porting the visibility patch. Original dirty checkout work was
preserved. Baseline hashes: text BA52F1BD1E5E3DC6042FFBA7A670E73201095AFED6A33320013109DA267CDC1A;
voice A3668B41A112FEB9DFDF505A7D6A0C36F9C6BE9BAA4070CFD9B0BAC826626EAE.
Same Windows 11 Home 10.0.26200, Ryzen 7 7800X3D / 16 logical CPUs, approximately 31 GiB
RAM, pinned Rust 1.98.1, release profile and E: target/temp paths. Both variants are unsigned.

| Metric | Main baseline | Channel visibility fix | Delta |
| --- | ---: | ---: | ---: |
| Text executable, bytes | 49,067,008 | 49,092,608 | +25,600 (+0.052%) |
| Text installed package, bytes | 49,431,699 | 49,466,993 | +35,294 (+0.071%) |
| Text ZIP, bytes | 30,843,886 | 30,860,898 | +17,012 (+0.055%) |
| Voice executable, bytes | 52,400,128 | 52,425,728 | +25,600 (+0.049%) |
| Voice installed package, bytes | 52,988,377 | 53,023,671 | +35,294 (+0.067%) |
| Voice ZIP, bytes | 32,209,427 | 32,225,829 | +16,402 (+0.051%) |
| Reducer median, ms | 26.2823 | 26.5260 | +0.2437 (+0.93%) |
| Retained timeline estimate | 220,992-221,477 bytes | Same | 500 records |

One release replay executable per revision, one direct warmup and five direct measured
runs of 100,000 synthetic events. Baseline samples: [26.2234, 26.2756, 26.8052, 26.2823, 28.1089] ms; after: [26.526, 26.5737, 26.175, 29.9168, 26.3162] ms.
This small median change is noisy and not evidence of a user-visible improvement or regression.
The replay exercises the reducer, not the complete Gateway obfuscation or native UI flow;
focused offline tests cover those state transitions. No RSS, idle CPU, frame/startup latency,
DPI/adapter capture, network, microphone or live audio measurements were taken while native
automation remains paused under the owner's stop.

Sizes include each complete package, excluding the sibling voice directory for text, PR
evidence, external WebView2 and audio drivers. Python ZIP DEFLATE level 9 was used consistently.
Baseline packages precede the integration evidence addendum; after packages include this task's
behavior docs but precede this final performance/progress addendum. Installed/ZIP deltas include
those documentation differences. No dependencies or license contents changed.
Final executable hashes: text FE540FEEC0781FC54D1CB3C950D53A512A96B770AF26F72D27BC49092F02BBB5;
voice 264B6EBEAF5FECBC08A33385B4F9A2073F318E3EE03D8E845D3E0A4E7191A80B.


## Permission-aware actions - September 10, 2026

Baseline is verified channel-visibility PR #17 revision
6fb81da12942b03e0a51599803d74ea37cbd3503. Its package outputs and replay executable were
copied into this task's separate baseline directory before permission implementation;
no existing unrelated debug build was used. After is feat/permission-aware-actions.
Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical CPUs), about 31 GiB RAM,
Rust 1.98.1 release, wgpu. Text is --no-default-features; voice adds --features voice.
Both unsigned Windows packages were built by cargo xtask package/package-voice.

| Metric / method | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,092,608 | 49,304,064 | +211,456 (+0.431%) |
| text installed bytes | 49,466,993 | 49,690,144 | +223,151 (+0.451%) |
| text zip bytes | 30,860,898 | 30,948,093 | +87,195 (+0.283%) |
| voice executable bytes | 52,425,728 | 52,637,184 | +211,456 (+0.403%) |
| voice installed bytes | 53,023,671 | 53,247,734 | +224,063 (+0.423%) |
| voice zip bytes | 32,225,829 | 32,314,397 | +88,568 (+0.275%) |
| 100,000-event replay median ms | 27.9407 | 29.2795 | +1.3388 (+4.79%) |

Installed totals enumerate package files (32 text, 78 voice), excluding the text package's
voice sibling. ZIP is Python zipfile DEFLATE level 9. Both packages retain dependency notices
and exclude PR screenshots. Baseline packaged docs predate #17's final measurement addendum;
changed packaged docs predate this final measurement addendum. Installed/ZIP deltas therefore
include their actual documentation snapshots and are not executable-only code growth.

cargo replay builds once; each revision receives one direct warmup and five measured direct
replay-bench runs. Baseline samples (ms): 29.9405, 26.4015, 27.9407, 29.2019, 27.8303.
After samples (ms): 29.0536, 29.2975, 29.189, 29.2795, 30.2557.
Both retain exactly 500 records and 220,992-221,477 estimated timeline bytes. This measures
synthetic reducer/message admission, not role-update storms, native latency, RSS or Discord
compatibility. The new fixture explicitly supplies permission metadata. The small increase
is reported as overhead, not an improvement; run-to-run noise is visible in the samples.

Native process memory, idle CPU, startup/frame timing, display scale and GPU adapter were
not measured: owner Escape stops paused native automation. No physical UI/audio/account test
was attempted. Permission metadata plus reserved bounded decision storage has a 2 MiB
estimated budget; an update's temporary cloned candidate is additional peak allocation.
No external runtime dependency or codec was added. UI tests now depend on existing test-support.

Executable SHA256: text 991592D31AB2C330F55F97B81755ACF8DA761ACE31B7D3E32B5D4205ADA49109;
voice BCFA2FB8440261A0DA12B2995C18CAA4A44AA48EFB01C5AABFFD62844801D9A5.


## Rich editor / notification integration - September 10, 2026

Actual starting revision is 67bff236e6186a306023c156f8027712049681ba (permission PR #19).
Its text/voice executable SHA256 values were checked against the preceding recorded builds,
then only the distribution files were copied into a separate integration baseline directory.
After integrates main 2879fbc7de8fb7de30664fb64666f6d93482b827 (PR #18) plus permission-aware
notifications/inline edits and the Windows notification compile fix. This comparison includes
the newly integrated rich editor and notification dependencies; it does not isolate the cost
of those fixes or compare against a separately measured #18 build.

Same Windows 11 Home 10.0.26200 / Ryzen 7 7800X3D / 16 logical CPUs / about 31 GiB RAM /
Rust 1.98.1 release/wgpu host. Text uses no-default-features; voice adds voice. Both packaging
commands passed and produced unsigned Windows binaries. ZIP uses Python DEFLATE level 9.

| Metric / method | Before integration (67bff23) | Integrated | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,304,064 | 49,610,752 | +306,688 (+0.622%) |
| text installed bytes | 49,690,144 | 50,033,083 | +342,939 (+0.690%) |
| text zip bytes | 30,948,093 | 31,080,883 | +132,790 (+0.429%) |
| voice executable bytes | 52,637,184 | 52,943,872 | +306,688 (+0.583%) |
| voice installed bytes | 53,247,734 | 53,589,761 | +342,027 (+0.642%) |
| voice zip bytes | 32,314,397 | 32,444,614 | +130,217 (+0.403%) |
| Reducer median ms / 100,000 events | 29.2795 | 37.5618 | +8.2823 (+28.29%) |

Installed file counts are now 42 text and 88 voice (32/78 before): notification license texts,
a documentation page and the explicitly operated shortcut-registration script are included.
The script is packaged but was not run. Text excludes the voice sibling; both exclude PR
screenshots. Totals describe actual packaged documentation snapshots before this final
measurement addendum. No package was launched or claimed signed.

Reducer workload: one direct warmup plus five measured runs per revision; prior baseline
samples were recorded at the start of this continuation. Before ms: 29.0536, 29.2975, 29.189, 29.2795, 30.2557.
After ms: 37.5618, 36.5306, 37.2953, 38.116, 37.8277.
Both retain 500 records / 220,992-221,477 estimated timeline bytes. The measured increase
includes new notification observation/count work; no speed improvement is claimed. This is
synthetic reducer timing, not process RSS, role-update storms or UI frame latency. Native
RSS/idle CPU, display/GPU parameters and physical notifications remain unmeasured because
owner Escape stops paused native automation. Imported #18 macOS evidence is not evidence for
this integrated Windows executable.

Integrated executable SHA256:
- text: 0020AF6225691A06B5C45E47B4F85F7061FFBD046C92B779EFB5F8D1EE15B4E4
- voice: EAD41DAC6D98F404881987517E83FCC8E3CADBD9BCFC9514985EAD3D853DF145


## Loaded People presence - September 10, 2026

Baseline f20042a73ca101c377a4c96734aa9f0d833d6fa4 is the integrated permission/editor/notification
PR #19. Its preceding measured executable SHA256 values were verified before copying separate
text and voice distribution baselines. This branch is stacked on #19; main's later image
aspect-ratio PR #20 is outside this comparison. Same Windows 11 Home 10.0.26200 / Ryzen 7
7800X3D / 16 logical CPUs / about 31 GiB RAM / Rust 1.98.1 release/wgpu host. Both unsigned
Windows packaging commands passed; text uses no-default-features, voice adds voice.
ZIP uses Python DEFLATE level 9; text excludes the voice sibling and both exclude PR evidence.

| Metric / method | Baseline f20042a | Presence | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,610,752 | 49,624,576 | +13,824 (+0.028%) |
| text installed bytes | 50,033,083 | 50,055,923 | +22,840 (+0.046%) |
| text zip bytes | 31,080,883 | 31,088,897 | +8,014 (+0.026%) |
| voice executable bytes | 52,943,872 | 52,957,184 | +13,312 (+0.025%) |
| voice installed bytes | 53,589,761 | 53,612,089 | +22,328 (+0.042%) |
| voice zip bytes | 32,444,614 | 32,455,653 | +11,039 (+0.034%) |
| Reducer median ms / 100,000 events | 37.5618 | 37.4956 | -0.0662 (-0.176%) |

Installed file counts remain 42 text / 88 voice. Totals include the actual packaged documentation
snapshots before this measurement addendum. Executable hashes below identify the measured builds;
no package was launched, signed or published. No new external dependency or media codec was added.

Reducer workload: one direct release warmup plus five measured runs per revision, using the
existing cargo replay executable. Baseline samples: 37.5618, 36.5306, 37.2953, 38.116, 37.8277 ms.
Presence samples: 37.4956, 38.6994, 36.2119, 36.4057, 37.8734 ms. Both retain 500 records and
220,992-221,477 estimated timeline bytes. The 0.0662-ms median difference is noise, not a speed
improvement. This message-reducer workload checks for unrelated timeline regression; it does
not measure presence-event throughput, process RSS or UI frame latency. Synthetic Gateway tests
separately verify 1,000 incoming users retain at most 100 pending loaded users and an 8-KiB event,
with no deadline extension. Those assertions are component bounds, not measured load performance.

Native screenshots, display/GPU parameters, idle CPU and process RSS remain unmeasured because
native automation is paused after owner Escape stops. Presence-only events leave timeline revision
unchanged, but no native repaint-rate or physical account delivery claim is made.

Measured executable SHA256:
- text: BB8FA32318B2FC16CBBA41FD46124C499363DDF8EEFA70BF7DB24D0C11DF8AE0
- voice: E403D2AFA448FDC8B78327FACFC3B87F4AEE99D26AC53A25FC10951B658E6AC9


## Keyboard conversation switcher - September 10, 2026

Baseline f8baa2df1f705b771b5a2cd99a10bb13b4d8a4c3 is draft presence PR #21. Its recorded
text/voice executable hashes were verified before separate package baselines were copied.
This comparison excludes main's independent image-aspect-ratio PR #20. Same Windows 11 Home
10.0.26200 / Ryzen 7 7800X3D / 16 logical CPUs / about 31 GiB RAM / Rust 1.98.1 release/wgpu
host. Both unsigned Windows packaging commands passed; text uses no-default-features and
voice adds voice. ZIP uses Python DEFLATE level 9, excluding the voice sibling from text.

| Metric / method | Baseline f8baa2d | Switcher | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,624,576 | 49,658,368 | +33,792 (+0.068%) |
| text installed bytes | 50,055,923 | 50,098,393 | +42,470 (+0.085%) |
| text zip bytes | 31,088,897 | 31,104,011 | +15,114 (+0.049%) |
| voice executable bytes | 52,957,184 | 52,990,464 | +33,280 (+0.063%) |
| voice installed bytes | 53,612,089 | 53,654,047 | +41,958 (+0.078%) |
| voice zip bytes | 32,455,653 | 32,466,873 | +11,220 (+0.035%) |

Installed file counts remain 42 text / 88 voice. Neither package includes PR screenshots.
Totals describe actual packaged documentation before this measurement addendum, not a
subsequently reconstructed archive. No package was launched, signed or published. No runtime
crate, core reducer, protocol, worker, storage schema or codec was added.

The picker limits its query to 128 characters / 512 bytes and results to 20 bounded labels.
It normalizes one eligible channel's bounded metadata at a time and retains no background
index. Matching and candidate allocations occur only while open. Headless checks assert query
length/capacity after an oversized Unicode paste and verify bounded results. These are component
bounds, not measured picker latency or RSS. The existing message-reducer replay does not exercise
this UI feature, so it was not rerun as a substitute for picker measurements.

Native screenshots, input/display/GPU parameters, process RSS/idle CPU and interactive latency
remain unmeasured because desktop automation is paused after owner Escape stops. Focus cleanup
requests repaint only while this modal's closing layer remains; no continuously active timer
or search worker was added. No improvement in native latency or memory is claimed.

Measured executable SHA256:
- text: F127A6ACB7F1857A39C174BBF348603DAF623A3BE4D3BEB74FC4424103D6EF39
- voice: 8BDE5C39E0E383BA323BEB7E9021D51D640684861D2F4104F8A84B567732B8E3


## Native dependency hardening - September 10, 2026

Baseline e4ef4a852415e5061735f9183ca65dd202406ecd (PR #22), separately copied and hash-verified
before edits. Same Windows 11 Home 10.0.26200 host, Ryzen 7 7800X3D (16 logical CPUs), about
31 GiB RAM, Rust 1.98.1, release thin LTO / one codegen unit / wgpu; text default and optional
voice packages built separately. No native process was launched. One package measurement per
variant; ZIP uses Python zipfile DEFLATE level 9, excluding the voice subdirectory from text.
Both unsigned cargo xtask package and package-voice commands passed.

| Metric / method | Baseline e4ef4a8 | Dependency patch | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 49,658,368 | 49,658,368 | +0 (+0.000%) |
| text installed bytes | 50,098,393 | 50,105,951 | +7,558 (+0.015%) |
| text zip bytes | 31,104,011 | 31,106,640 | +2,629 (+0.008%) |
| voice executable bytes | 52,990,464 | 53,016,064 | +25,600 (+0.048%) |
| voice installed bytes | 53,654,047 | 53,687,168 | +33,121 (+0.062%) |
| voice zip bytes | 32,466,873 | 32,480,689 | +13,816 (+0.043%) |

Text/voice installed file counts remain 42/88. Text executable size is unchanged; the voice
executable grows 25,600 bytes (0.048%). Removing unused lockfile paths is not an executable-size
or runtime-speed optimization claim. Installed/ZIP totals include the actual staged docs and
modified HPKE source before this measurement addendum. Packages contain no PR screenshots.
Packaged HPKE source and the existing Davey MIT notice were byte-checked against the workspace.
Davey Rust source is unchanged; its manifest loses browser timer features, and the HPKE fork
loses its unused libcrux provider declaration. DAVE/MLS, SHAKE vectors and encrypted local Opus
transport pass in the 190-test workspace suite. No reducer, cache, UI or audio algorithm changes
were made, so the unrelated reducer workload was not rerun. Native CPU/RSS and real voice
latency remain unmeasured under the owner's paused desktop/live gate.

Cargo.lock package count falls from 775 to 738; strict cargo-audit 0.22.2 falls from six
vulnerability-class findings/five denied warnings to zero/two. It still fails on Linux GLib
unsoundness and proc-macro-error maintenance; no suppression or release approval is implied.

Measured executable SHA256:
- text: 16F944D856C896C9D5EC2A388E6434E20BEC5F972302B5884B56922FE770826F
- voice: 161A9A1ADD3F4A90A5B7448E4B46633ED4A7515CF993C0EA0BDC5F95F55ED8F2

## Image aspect ratios — September 10, 2026

| Metric / method | Baseline | After | Delta |
| --- | --- | --- | --- |
| text executable, bytes | 46,098,496 | 46,098,512 | +16 (+0.0000%) |
| text package, bytes | 69,734,675 | 69,736,433 | +1,758 (+0.0025%) |
| text compressed, bytes | 53,074,332 | 53,075,144 | +812 (+0.0015%) |
| voice executable, bytes | 48,917,808 | 48,917,808 | +0 (+0.0000%) |
| voice package, bytes | 74,162,600 | 74,164,342 | +1,742 (+0.0023%) |
| voice compressed, bytes | 55,695,929 | 55,697,942 | +2,013 (+0.0036%) |
| Settled RSS, KiB (`ps`, 10 × 1 s) | 152,592 | 99,328 | −53,264; noisy, no improvement claim |
| Settled CPU, median / maximum | 0 / 0% | 0 / 2.4% | median unchanged |

macOS 27.0 arm64, Apple M1 Pro (8 CPU / 14 GPU cores), 16 GB RAM, wgpu/Metal, built-in 3024×1964 Retina display at unchanged system scale; default ~1087×768 captured window. Rust 1.98.1 locked release text-only/voice packages, baseline clean `2879fbc`; native runs explicitly `--demo`, default synthetic timeline followed by opening its image viewer. Separate baseline/final app copies. Full package totals count files, excluding the nested voice distribution from text; gzip tarballs include package files. Package measurements precede appending these measurements to docs.

Ten OS RSS/CPU samples at one-second intervals after interaction settled (baseline at least 10 s; final resampled after at least 30 s). Initial final capture/packaging-period samples were 108,400–108,928 KiB and 1.7–41.1% CPU; the settled retry above is reported separately, not hidden. Different warmup/background capture/packaging activity and OS memory accounting prevent interpreting the RSS drop as an improvement. These are sampled process RSS, not allocation limits or physical footprint; startup/interaction peak, GPU allocations and p95 frame/startup latency are unmeasured. Baseline had no child processes; no voice call or audio measurement. No performance improvement claim.

## Profile popout — September 10, 2026

Same macOS arm64 host, Rust 1.98.1, release profile (thin LTO), lockfile unchanged. Baseline is `main` at `74709e2` built with `cargo build --release --locked -p serein` before editing; the after build is the same command on the task branch. Package outputs come from `cargo xtask package` / `package-voice` on the task branch only; no voice baseline was built for this task.

| Metric / method | Baseline | After | Delta |
| --- | --- | --- | --- |
| Text executable, unsigned `cargo build --release` | 46,367,952 B | 46,408,896 B | +40,944 B (+0.09%) |
| Text package executable, ad-hoc signed (`dist/Serein.app`) | not built | 46,139,168 B | — |
| Voice package executable, ad-hoc signed (`dist/voice/Serein.app`) | not built | 48,958,768 B | — |
| Full package directory (`du -sk`) text / voice | not built | 45,556 KiB / 48,652 KiB | — |
| Idle `ps` RSS, release `--demo`, 10 s warmup, 5 samples at 2 s | 153,280–153,328 KiB | 155,072–155,216 KiB | ≈ +1.2% (noise-level) |
| Idle `ps` RSS, release `--demo --demo-profile` (popout + People open) | n/a | 153,424 KiB (stable) | — |
| Sampled CPU after warmup | 0.0% | 0.0–1.0% (one 7.1% first sample) | no sustained change |

Method: wgpu/Metal renderer, 1120×792 logical window at 2× scale, no interaction after launch beyond activating the window, no network, no helper processes. RSS is whole-process resident memory of the single executable; GPU and driver allocations are not included. The two-sample difference is inside run-to-run noise and is not called a regression or improvement. Frame time and p95 latency were not instrumented. A new RAM-only profile cache holds at most 32 entries / 1 MiB estimated model bytes for 15 minutes (cleared on session changes), so reopening a card costs one clone instead of a REST round-trip. Other new retained data is bounded inside existing caps: profile ≤64 KiB (theme colors 8 B, tag ≤8 chars + 32-hex badge hash, ≤16 badge icon hashes), member custom status ≤128 chars inside the 128 KiB member budget; badge/tag artwork uses the existing 16 MiB / 64-texture allowance with 64-pixel requests.

## September 10: welcome and system messages

Baseline `6053299` (clean main, origin/main fetched and unchanged), task branch `feat/system-messages`.
macOS 27.0 (26A428), MacBookPro18,3 / Apple M1 Pro, 16 GiB, arm64, pinned Rust 1.98.1, release thin LTO / one codegen unit, wgpu. No dependency or lockfile changes.

| Metric / method | Baseline | After | Delta |
| --- | ---: | ---: | ---: |
| text executable bytes | 46,141,408 | 46,159,872 | +18,464 (+0.040%) |
| text installed bytes | 46,581,502 | 46,599,966 | +18,464 (+0.040%) |
| text zip bytes | 29,911,653 | 29,918,046 | +6,393 (+0.021%) |
| voice executable bytes | 48,977,264 | 48,979,328 | +2,064 (+0.004%) |
| voice installed bytes | 49,648,319 | 49,652,424 | +4,105 (+0.008%) |
| voice zip bytes | 31,253,839 | 31,258,631 | +4,792 (+0.015%) |
| Native RSS median KiB | 153888 | 155200 | +1312 |
| Native RSS sample peak KiB | 153952 | 155584 | +1632 |
| Native CPU median % | 0 | 0 | +0 |
| Reducer replay median ms | 29.331958 | 28.687209 | -0.644749 (-2.20%) |

Both `cargo xtask package` and `cargo xtask package-voice` passed with strict ad-hoc signature verification. Package file counts are 47 text / 93 voice. Sizes are one measurement per variant: executable, sum of staged installed files, Python ZIP DEFLATE level 9. Excludes prior distribution ZIPs left in dist and the separate voice subdirectory from text. Outputs were copied to separate before/after directories; baseline voice was built from an isolated baseline checkout. Staged docs precede this measurement addendum; changed voice includes the compatibility/storage notes written between package builds. No PR evidence or debug symbols are bundled.

Native samples use the same existing `--demo --demo-chat` fixture, default viewport (observed 1087×768 screenshot), system dark appearance, no extra navigation, 10-second warmup and ten `ps -p PID -o %cpu=,rss=` samples at one-second intervals for each fresh process. Median and sample peak RSS are not launch peak, GPU memory or macOS physical footprint. CPU ranges were 0..0.2% before and 0..3.0% after; medians 0%. No login/voice helpers were started by these demo processes. Display scale was not instrumented; both runs used the same display without changing its settings. RSS rose about 1.3 MiB; this short synthetic sample does not establish long-run memory or live-channel acceptance.

Replay: `cargo replay`, then one warmup and five direct `replay-bench` runs per revision. Baseline measured runs: 29.622583, 29.331958, 29.198583, 29.183917, 29.527458 ms. After: 29.256916, 29.038542, 28.407292, 28.603875, 28.687209 ms. Both retain 500 records / 220,992–221,477 estimated bytes. These timings overlap; no speed improvement is claimed. Replay measures the existing 100,000 ordinary synthetic reducer events, not system-message rendering or SQLite migration. A stale cross-worktree Cargo artifact initially failed the changed replay compile; rebuilding the model resolved it before these measured runs.

The screenshot pair uses an identical new eight-event synthetic fixture (`--demo --demo-system-messages`) and scroll-to-top at the same viewport/appearance. Baseline screenshot was rebuilt at `6053299` with only the fixture and demo selector backported; it still discards kinds and renders the old placeholder. Package size and CPU/RSS baseline use the unmodified baseline. Dark native rendering/scrolling inspected; light/narrow rendering checked with headless egui at 280px, wide/dark at 900px. Native light-theme selection did not visibly change via automation, so native light/keyboard, display scale, p95 startup/frame latency, GPU allocation and other platforms remain unverified. No live Discord compatibility claim.

## Integration of PRs #17 through #32 with main (2026-09-10)

Baseline is published PR #32 at 57927c7, not a separately built main. Its text/voice
executables and package reports were copied and hash-verified before integration.
After includes main 85fde15 (image aspect ratio, profile popout/cache, system messages)
plus schema-union and permission/profile/presence integration repairs. Both release
package commands passed; text uses no default features, voice explicitly enables voice.

| Metric | Published PR #32 baseline | Integrated main + stack | Delta |
| --- | ---: | ---: | ---: |
| text executable, bytes | 49,855,488 | 49,938,944 | +83,456 (+0.17%) |
| text installed, bytes | 50,370,977 | 50,478,583 | +107,606 (+0.21%) |
| text zip, bytes | 31,205,377 | 31,244,819 | +39,442 (+0.13%) |
| voice executable, bytes | 53,213,184 | 53,297,152 | +83,968 (+0.16%) |
| voice installed, bytes | 53,951,446 | 54,059,969 | +108,523 (+0.20%) |
| voice zip, bytes | 32,576,903 | 32,615,326 | +38,423 (+0.12%) |
| Replay median, ms | 38.5417 | 38.4845 | -0.0572 (-0.15%; noise) |
| Retained 500-message timeline, estimated bytes | 220,992..221,477 | 228,992..229,477 | +8,000 |

Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical CPUs), approximately 31 GiB RAM,
Rust 1.98.1, release thin LTO/one codegen unit and wgpu. Reducer: one warmup plus five
direct executable runs, 100,000 synthetic events each. Before: 36.6465, 39.2736, 37.5642,
38.5417, 38.7553 ms; after: 42.3323, 36.6533, 36.7709, 38.4845, 38.8944 ms. The small
timing difference is noise; the new message-kind field accounts for the retained-size
increase. This workload measures neither process RSS nor native frame or startup latency.
Native automation remains owner-paused, so those metrics and new screenshots are unmeasured.
ZIPs use Python DEFLATE level 9; text excludes nested voice, both exclude PR screenshots.
Installed packages contain the documentation snapshot staged during packaging, before
this final measurement addendum. No live Discord, microphone or account test was run.


## Reply-target navigation (2026-09-10)

| Metric / method | Main 33181a0 baseline | Reply targets | Delta |
| --- | ---: | ---: | ---: |
| text executable, bytes | 49,938,944 | 49,956,352 | +17,408 (+0.03%) |
| text installed, bytes | 50,481,170 | 50,502,356 | +21,186 (+0.04%) |
| text zip, bytes | 31,245,568 | 31,255,214 | +9,646 (+0.03%) |
| voice executable, bytes | 53,297,152 | 53,314,560 | +17,408 (+0.03%) |
| voice installed, bytes | 54,059,969 | 54,083,337 | +23,368 (+0.04%) |
| voice zip, bytes | 32,615,326 | 32,627,919 | +12,593 (+0.04%) |
| 100,000-event replay median, ms | 36.6917 | 36.9486 | +0.2569 (+0.70%; noise) |
| Retained 500-message timeline, estimated bytes | 228,992..229,477 | 228,992..229,477 | 0 |

Baseline executables copied and hash-verified from the merged integration packages;
source tree equals main 33181a0. Text uses no default features; voice enables voice explicitly.
Windows 11 Home 10.0.26200, Ryzen 7 7800X3D (16 logical CPUs), approximately 31 GiB RAM,
Rust 1.98.1, release thin LTO/one codegen unit, wgpu. Replay: one warmup and five direct
executable runs on each build, 100,000 synthetic events per run. Before samples:
36.4011, 36.769, 36.6917, 37.293, 36.3099 ms; after: 36.2521, 36.2958, 36.9971, 36.9486, 36.9554 ms.
This is reducer time, not native frame/startup latency or process RSS; the small timing
difference is noise. Native desktop automation remains paused, so screenshots, idle CPU,
process/GPU memory and screen-reader/IME checks were not measured for this change.
Package ZIPs use Python DEFLATE 9. Text excludes nested voice; all packages exclude PR
screenshots. Packaged docs are the snapshot copied during packaging, before this final
measurement addendum. No live Discord, microphone or account action was performed.
