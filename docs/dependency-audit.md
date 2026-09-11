# Dependency audit — September 10, 2026

Voice is now part of every desktop build. Historical optional-voice selection reports below must not be used as the current default dependency graph. License checks remain enforced in their dedicated CI job, independently of packaging.

The Linux GTK4/WebKit6 migration passes the unchanged `cargo audit --deny warnings` gate:
**zero vulnerability-class findings and zero warnings**, exit 0, with 726 locked packages.
The check used cargo-audit 0.22.2 and 1,243 advisories at
`b50980aad8b8f14f77e25a97b32dd94bf008b0af`. No ignores or vulnerable-version relabeling were added.
Historical results below describe earlier revisions. This audit does not prove native login,
runtime storage isolation, live service compatibility or a completed security review.

The migration replaces Linux's old GLib/GTK3/WebKit2GTK chain with GTK4 0.11.4, WebKit6 0.6.1
and GLib 0.22.9. Wry's upstream GTK4 migration is not released; its old Linux dependencies
would remain in Cargo.lock even if the consumer only selected Wry on Windows/macOS. The local
Wry 0.57.0 manifest/build patch removes those unused declarations and rejects unsupported
target builds; all backend sources remain unchanged. Provenance is under vendor/wry.
Native Linux/Wayland/X11 execution and live login validation remain open.

## Earlier text-client result

On the macOS development host, ran:

```sh
target/audit-tool/bin/cargo-audit audit --deny warnings --json > target/audit-review-2026-09-10.json
cargo tree --locked --target x86_64-unknown-linux-gnu -i glib@0.18.5
cargo tree --locked --target x86_64-unknown-linux-gnu -i proc-macro-error@1.0.4
cargo tree --locked --target aarch64-apple-darwin -i glib@0.18.5
```

`cargo-audit 0.22.2` exited **1**. Its JSON contained zero vulnerability-class entries, one `unsound` warning and one `unmaintained` warning. This is a failed security check, not a clean audit. The advisory database contained 1,243 advisories at commit `b50980aad8b8f14f77e25a97b32dd94bf008b0af` (updated September 9, 2026).

The Linux dependency trees confirmed these paths:

```text
serein → platform → wry 0.57.0 → webkit2gtk 2.0.2 → gtk 0.18.2 → glib 0.18.5
serein → platform → gtk 0.18.2 → gtk3-macros 0.18.2 → proc-macro-error 1.0.4
glib 0.18.5 → glib-macros 0.18.5 → proc-macro-error 1.0.4
```

Wry also reaches GLib through other GTK/WebKit bindings. The macOS-target inverse tree printed nothing: this chain is not selected for that target. The workspace lockfile and Linux release still include it. Neither target selection nor an optional feature removes the requirement to fix the supported Linux configuration.

| Advisory | Actual issue | Published remediation |
|---|---|---|
| [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) | `glib::VariantStrIter` passes an incorrectly mutable C output argument, leading to undefined behavior and possible optimized-build crashes. | GLib Rust bindings ≥0.20.0; the [upstream fix](https://github.com/gtk-rs/gtk-rs-core/pull/1343) corrects the output pointer mutability. |
| [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html) | `proc-macro-error` is unmaintained. Here it is a build-time macro dependency. | No patched version of that package is listed. Both macro consumers must migrate; changing just one leaves the other path. |

A source search found no direct `VariantStrIter` or `array_iter_str` use in our `apps/` or `crates/`. That limited search is not a transitive reachability or exploitability proof.

## Why a version bump is insufficient

The downloaded, locked Wry 0.57.0 manifest requires `gtk = "0.18"`, `webkit2gtk = "=2.0.2"`, and related bindings from that generation. GTK 0.18.2 requires `glib = "0.18"`. Both `glib-macros 0.18.5` and `gtk3-macros 0.18.2` require `proc-macro-error = "1.0"`. These requirements were read directly from Cargo's registry sources. Adding a newer direct GLib dependency cannot replace their incompatible requirement.

Upstream work is evolving: [GTK's current documentation](https://docs.rs/gtk/latest/gtk/) reports 0.19.0, while its [development manifest](https://github.com/gtk-rs/gtk3-rs/blob/master/Cargo.toml) reports 0.20.0-alpha with GLib 0.23.0-alpha. The old locked release describes itself as unmaintained; that description must not be generalized to all present GTK3 development. Neither newer version satisfies Wry's existing GTK requirement.

Wry's [GTK4/WebKit6 migration issue](https://github.com/tauri-apps/wry/issues/1474) remains open and its [migration PR](https://github.com/tauri-apps/wry/pull/1530) remains a draft in the sources checked today. These are implementation leads, not a tested release combination for Serein.

## Remaining implementation options

1. Adopt a released Wry/GTK/WebKit combination that removes both paths when available. Recheck the whole Linux tree and audit, not just direct versions.
2. If release must precede upstream support, replace only the Linux authentication window with a reviewed GTK4/WebKit6 integration. Preserve ephemeral storage, main-frame origin/capability checks, navigation and download restrictions, event-loop integration, and deterministic teardown. This requires Linux build and runtime testing on X11 and Wayland; it was not implemented or tested in this audit task.
3. A maintained backport would require patching the GLib defect and replacing both obsolete macro dependencies, with ownership, provenance, tests, and a correctly recognized advisory resolution. Vendoring the existing code or changing version labels solely to silence the audit is not remediation.

The earlier Linux-only investigation changed no dependency manifest, lockfile, platform integration, or audit suppression. No Linux runtime or live Discord session was exercised. `cargo xtask check` remains the integration check for implementation changes; it is separate from the failing `cargo audit --deny warnings` security gate.


## Voice dependency investigation

The captured `target/audit-voice-2026-09-10.json` is the **initial, pre-backport** voice lockfile result: `cargo-audit 0.22.2 audit --deny warnings --json` failed with **six vulnerability advisories and five warnings** across 753 locked dependencies, using the same advisory database commit as above. A lockfile audit includes optional dependencies; it does not by itself identify which code a given build selects.

Read the downloaded manifests and ran inverse dependency trees for each reported package with:

```sh
cargo tree --locked --workspace --all-features --target all -i PACKAGE
cargo tree --locked --workspace --all-features --target aarch64-apple-darwin -i PACKAGE
```

Additional platform checks used `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc` for `proc-macro-error2`. The default `serein` macOS tree does not select voice or its crypto dependencies. Enabling the desktop `voice` feature selects this path, including on the macOS host:

```text
serein → discord-voice → davey 0.1.4 → openmls_rust_crypto 0.5.1
  → hpke-rs 0.6.1 → libcrux-sha3 0.0.8
    → libcrux-traits 0.0.6 → libcrux-secrets 0.0.5
```

Davey's RustCrypto provider therefore did **not** eliminate every libcrux dependency. Three vulnerability advisories were on selected packages before remediation. This is package selection evidence, not a claim that all affected functions are reachable through a DAVE call.

| Initial package / advisory | Observed selection | Published remediation |
|---|---|---|
| `libcrux-sha3 0.0.8`, [RUSTSEC-2026-0207](https://rustsec.org/advisories/RUSTSEC-2026-0207.html) | Selected in voice; incremental SHAKE could drop output between squeeze calls. | `libcrux-sha3 >=0.0.10`. |
| `libcrux-sha3 0.0.8`, [RUSTSEC-2026-0208](https://rustsec.org/advisories/RUSTSEC-2026-0208.html) | Selected in voice; AVX2 SHAKE can panic for certain output lengths. | `libcrux-sha3 >=0.0.10`. |
| `libcrux-secrets 0.0.5`, [RUSTSEC-2026-0212](https://rustsec.org/advisories/RUSTSEC-2026-0212.html) | Selected in voice through SHA3's traits dependency; affected swap/select assembly concerns AArch64. | `libcrux-secrets >=0.0.6`. |
| `libcrux-aesgcm 0.0.7`, [RUSTSEC-2026-0209](https://rustsec.org/advisories/RUSTSEC-2026-0209.html) and [RUSTSEC-2026-0211](https://rustsec.org/advisories/RUSTSEC-2026-0211.html) | Locked through the unselected optional `hpke-rs-libcrux` backend; no inverse tree even with workspace all-features/all-targets. | Migrate to the actual replacement `libcrux-aes >=0.0.9`; old package name has no patched release. |
| `libcrux-chacha20poly1305 0.0.7`, [RUSTSEC-2026-0124](https://rustsec.org/advisories/RUSTSEC-2026-0124.html) | Same unselected optional backend; no selected inverse tree. This is distinct from the RustCrypto `chacha20poly1305` used by our transport. | `libcrux-chacha20poly1305 >=0.0.8`. |

The optional locked chain is `hpke-rs → hpke-rs-libcrux → libcrux-aead → libcrux-aesgcm / libcrux-chacha20poly1305`. `--all-features` enables features of workspace packages; it does not enable every feature of every external dependency. No `openmls_libcrux_crypto` package is selected or locked here. Optional-package findings remain failures of the strict lockfile audit and must not be hidden with ignore rules.

The additional warning paths are:

- [RUSTSEC-2024-0384](https://rustsec.org/advisories/RUSTSEC-2024-0384.html), `instant 0.1.13`: selected on the host through `davey → openmls["js"] → fluvio-wasm-timer 0.2.5 → parking_lot 0.11.2 / parking_lot_core 0.8.6`. The upstream recommendation is migration to `web-time`; `instant` has no patched release. The native client itself does not need JavaScript timers, but Davey enables OpenMLS's `js` feature in its manifest, so our direct `default-features = false` cannot remove that feature.
- [RUSTSEC-2026-0210](https://rustsec.org/advisories/RUSTSEC-2026-0210.html), `libcrux-aesgcm`: the same unselected optional backend, renamed upstream to `libcrux-aes`.
- [RUSTSEC-2026-0173](https://raw.githubusercontent.com/RustSec/advisory-db/main/crates/proc-macro-error2/RUSTSEC-2026-0173.md), `proc-macro-error2 2.0.1`: `libcrux → hax-lib → hax-lib-macros` selects this macro dependency only under the custom `cfg(hax)` formal-verification configuration. `--target all` displays it, but host/Linux/Windows inverse trees are empty; Serein does not enable `cfg(hax)`. It has no patched release; upstream recommends migrating away. Replacing old `proc-macro-error` with this other unmaintained package would not fix the existing Linux maintenance warning.

### Substantive dependency backport

Published package metadata was checked on September 10, 2026 through the [crates.io HPKE dependency API](https://crates.io/api/v1/crates/hpke-rs/0.7.0/dependencies), [OpenMLS provider metadata](https://crates.io/api/v1/crates/openmls_rust_crypto/0.6.0/dependencies), and the downloaded release source archives. `hpke-rs 0.6.1` is the newest published 0.6 release and requires `libcrux-sha3 ^0.0.8`; Cargo's compatibility range excludes 0.0.10. Its old traits dependency pins secrets to `=0.0.5`. A lock-only update cannot replace this chain with the fixed versions.

HPKE 0.7.0 selects SHA3 0.0.10, but requires a coordinated provider upgrade: `openmls_rust_crypto 0.6.0` uses HPKE 0.7, OpenMLS traits/storage 0.6, and TLS codec 0.5. Those versions do not satisfy Davey 0.1.4's existing OpenMLS 0.8.1/provider 0.5 requirements. Adding a newer direct dependency would leave the old chain in place.

The applied repair is the [documented HPKE 0.6.1 backport](../vendor/hpke-rs/SEREIN-PATCH.md): replace its only two `libcrux_sha3::shake256` calls with the maintained [RustCrypto SHAKE-256 implementation](https://docs.rs/sha3/0.10.8/sha3/) using `Update`, `ExtendableOutput`, and `XofReader`. Preserve the exact 32-/64-byte output sizes, HPKE interfaces, upstream package identity, and MPL-2.0 licensing/provenance. This replaces the actual selected primitive rather than renaming vulnerable code or suppressing advisories. The package includes a known-answer SHAKE check for three inputs at both output sizes. The vendored MPL-2.0 source and canonical license are included with packaged distribution notices.

A narrower SHA3 dependency bump was investigated first. The fixed SHA3 0.0.10 source preserves the required function signature and selects traits 0.0.8/secrets 0.0.6, but Cargo resolution conflicts with the old optional libcrux backend's pinned SHA3/Hax generation. Updating the whole old backend is a larger compatibility change. Switching the two one-shot calls to the existing RustCrypto family removes the active libcrux chain while retaining the old backend as an explicitly unused optional dependency. Its locked advisories remain visible in the strict audit.

HPKE's two original SHAKE calls are fixed-size key derivation for XWing/ML-KEM. They do not use the incremental interface or problematic AVX2 output shape described by the SHA3 advisories. That limited source inspection is not a general cryptographic audit or a reason to retain the selected vulnerable packages.

### Final post-backport verification

Ran the final strict audit against the patched lockfile:

```sh
target/audit-tool/bin/cargo-audit audit --deny warnings --json > target/audit-voice-patched-2026-09-10.json
```

Exit **1**: **six vulnerability advisories, four unmaintained warnings and one unsound warning**, across **755 locked dependencies**, with advisory database commit `b50980aad8b8f14f77e25a97b32dd94bf008b0af`. These totals remain unchanged because the optional libcrux backend still resolves the old packages into Cargo.lock. The repaired selected dependency graph is:

```text
serein → discord-voice → davey 0.1.4 → openmls_rust_crypto 0.5.1
  → hpke-rs 0.6.1 (vendor/hpke-rs) → sha3 0.10.9 (RustCrypto)
```

Repeated `cargo tree --locked --workspace --all-features --target all -i PACKAGE` for `libcrux-sha3`, `libcrux-secrets`, `libcrux-aesgcm`, `libcrux-chacha20poly1305`, and `proc-macro-error2`: every inverse tree was empty. macOS-target checks for SHA3/secrets were also empty. The inverse `sha3` tree selects only the RustCrypto replacement through the patched HPKE package. The `instant` inverse tree remains the active OpenMLS JavaScript-timer path shown above.

Consequently all six vulnerability-class findings are on **unselected optional packages** in this final configuration. The old `proc-macro-error2` warning is also unselected; its `cfg(hax)` parent chain is no longer selected after removal of active libcrux. `instant` remains selected for voice, and the pre-existing GLib/proc-macro-error findings remain relevant to the supported Linux authentication build. This is dependency-selection evidence, not a promise of exploit-free software or a reason to mark the strict audit passed.

Further remediation needs a released or reviewed coordinated OpenMLS/Davey dependency update, removal of the unnecessary native `js` timer feature upstream, and the Linux authentication migration described above. Until then the six/five lockfile audit failure remains explicit in CI and release reporting. No audit ignore list, altered vulnerability version label, or hidden backend switch was introduced.

## Mentions, profiles and native save follow-up

The September 10 follow-up reran `target/audit-tool/bin/cargo-audit audit --json` after adding rfd 0.17.2 and pollster 0.4.0. It still reports six vulnerability-class findings and five warnings (four unmaintained, one unsound); none is attributed to the two added packages. The strict audit remains failing; the existing findings above are not waived. Output: `target/mentions-profiles-audit.json`.


## Guild voice implementation check — September 10, 2026

Re-ran `target/audit-tool/bin/cargo-audit audit --deny warnings --json` for the guild voice branch: exit 1, six vulnerability advisories and five warnings (four unmaintained, one unsound), matching the previously documented blocked audit. This task adds no dependencies and does not change Cargo.lock or the vendored code. Existing dependency/source notices remain in both packages; the new mixer and group handling are original repository code. This is not a clean security audit or release approval.


## Remove unused native dependency paths - September 10, 2026

Starting from e4ef4a8 (conversation switcher PR #22), the existing hpke-rs 0.6.1 fork no longer
declares the unused optional hpke-rs-libcrux backend or its dev dependency. Its backend feature
and re-export are removed; examples, KDF tests and benchmarks use RustCrypto, and duplicate
libcrux AEAD test cases are omitted. OpenMLS continues selecting HpkeRustCrypto. The existing
SHAKE256 backport is unchanged. This is removal of unused dependency declarations, not a fix
to libcrux cryptographic implementations.

A local Davey 0.1.4 manifest patch removes OpenMLS's unconditional `js` feature. Davey Rust
sources, dependency versions and native DAVE behavior are unchanged; OpenMLS uses std::time
on native platforms. Serein does not target WebAssembly. The crate checksum, original manifest,
VCS revision and upstream MIT text are retained under vendor/davey. A newer compatible release
without the unconditional feature was not available when checked. Both fork removal conditions
are documented in their SEREIN-PATCH.md files.

Cargo.lock drops 37 packages net (775 to 738); Davey changes from registry to the documented
local patch. No remaining package version is changed. Removed paths include hpke-rs-libcrux,
libcrux/hax packages, proc-macro-error2, fluvio-wasm-timer, parking_lot 0.11 and instant.
The same cargo-audit 0.22.2 and 1,243-advisory database at
`b50980aad8b8f14f77e25a97b32dd94bf008b0af` were used before and after: six vulnerability-class findings and
five denied warnings before; zero and two after. Logs are target/audit-before.log and
target/audit-after.log in the isolated development checkout. The CI command is unchanged.

The remaining supported Linux webview paths and remediation constraints above still apply.
No algorithm rewrite, advisory ignore, vulnerable-package relabeling, native window, account,
microphone or live Discord action is part of this change.
