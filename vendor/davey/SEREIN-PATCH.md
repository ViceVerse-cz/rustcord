# Native OpenMLS dependency selection

Source: crates.io `davey` 0.1.4, upstream git `a1e2e741bea06bc3b7167a5c3792844b8975993c`, crate directory `davey` in <https://github.com/Snazzah/davey>. Original crate archive SHA-256: `25028cc2ec8cd43138ca0d2c71d7f6019196238ebd6edea11dd35276fa85b860` (checked against the local archive and crates.io metadata).

The only upstream manifest change removes `features = ["js"]` from the `openmls` dependency. Versions, other features, and Rust source remain unchanged. Serein targets native desktops; this patch does not provide WebAssembly support. OpenMLS 0.8.1 already selects `std::time` on native targets. Its `js` feature adds WebAssembly dependencies, including `fluvio-wasm-timer`, which brings the unmaintained `instant` dependency into the lockfile. No DAVE protocol, encryption, key lifecycle, or selected RustCrypto provider code is changed. No advisory is suppressed.

On September 10, 2026, crates.io reported 0.1.4 as the latest non-yanked Davey release, and upstream still requested the `js` feature unconditionally. Remove this patch when a compatible upstream release permits native dependency selection without that feature. Merely moving the dependency to a WebAssembly target section would retain it in the all-target lockfile used by the security audit.

Upstream provenance is retained in `Cargo.toml.orig` and `.cargo_vcs_info.json`. All `src/` files and `README.md` are copied unchanged. The upstream crate's lockfile and registry marker are omitted; the workspace lockfile controls resolution. The MIT license is copied from the exact upstream revision because the published crate archive omits it:

- Crate metadata: <https://crates.io/api/v1/crates/davey/0.1.4>
- Original manifest: <https://github.com/Snazzah/davey/blob/a1e2e741bea06bc3b7167a5c3792844b8975993c/davey/Cargo.toml>
- License: <https://github.com/Snazzah/davey/blob/a1e2e741bea06bc3b7167a5c3792844b8975993c/LICENSE>

Validation belongs to the consuming workspace: run the existing two-party DAVE/MLS encryption, tampering, and transition tests, the voice suite, and the strict dependency audit after lockfile resolution. This manifest change is not evidence of live Discord compatibility or an independent cryptographic audit.
