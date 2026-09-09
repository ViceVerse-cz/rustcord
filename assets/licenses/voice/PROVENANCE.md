# Voice dependency license provenance

Collected September 10, 2026. Except for the separately identified canonical MPL text below, files are unmodified source license/notices, copied from the exact resolved crates.io releases or fetched from the commit recorded in the release's `.cargo_vcs_info.json`. Registry source links identify the shipped source archive; SHA-256 values below verify the copied text. All files are flat for distribution staging.

Davey 0.1.4 and OpenMLS 0.8.1 omit their root license files from their registry archives. Their MIT texts were retrieved from the pinned upstream commits linked below (Davey package path `davey`, OpenMLS package path `openmls`). `libopus_sys` 0.3.3 bundles the codec under `opus/`; the registry archive is authoritative because its VCS metadata records a dirty working tree. Both its binding licenses and the bundled codec's COPYING and LICENSE_PLEASE_READ.txt are retained, including the upstream IETF patent-statement references.

This directory covers newly added direct voice libraries and the bundled codec/binding notices. It is **not** a complete transitive dependency license bundle, legal opinion, patent review or platform redistribution sign-off. The full per-artifact release review remains outstanding as described in THIRD_PARTY_NOTICES.md.

| File | Exact source | SHA-256 |
|---|---|---|
| cpal-LICENSE.txt | [source](https://docs.rs/crate/cpal/0.18.2/source/LICENSE) | `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4` |
| opus2-LICENSE-MIT.txt | [source](https://docs.rs/crate/opus2/0.4.0/source/LICENSE-MIT) | `6b3b465fa69075348ee5ffc9ba6afa93402743a4a942a463404fac25257bd3ed` |
| opus2-LICENSE-APACHE.txt | [source](https://docs.rs/crate/opus2/0.4.0/source/LICENSE-APACHE) | `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2` |
| rtrb-LICENSE-MIT.txt | [source](https://docs.rs/crate/rtrb/0.4.0/source/LICENSE-MIT) | `23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3` |
| rtrb-LICENSE-APACHE.txt | [source](https://docs.rs/crate/rtrb/0.4.0/source/LICENSE-APACHE) | `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2` |
| chacha20poly1305-LICENSE-MIT.txt | [source](https://docs.rs/crate/chacha20poly1305/0.10.1/source/LICENSE-MIT) | `3c0dfa33fd2e6976038555b52095699452653b1fcabe113074f14e0848a6b11e` |
| chacha20poly1305-LICENSE-APACHE.txt | [source](https://docs.rs/crate/chacha20poly1305/0.10.1/source/LICENSE-APACHE) | `a9040321c3712d8fd0b09cf52b17445de04a23a10165049ae187cd39e5c86be5` |
| libopus_sys-LICENSE.txt | [source](https://docs.rs/crate/libopus_sys/0.3.3/source/LICENSE) | `c5512e4899a9b06c3a286bf69444ebc5314d506e437c334bfd66a2eac7bca3ef` |
| libopus_sys-LICENSE-old.txt | [source](https://docs.rs/crate/libopus_sys/0.3.3/source/LICENSE-old) | `a6c8bb45880edc99bb13feb04d69fb2f0f73b565dfe209affa60414d2aa4c1a7` |
| libopus_sys-opus-COPYING.txt | [source](https://docs.rs/crate/libopus_sys/0.3.3/source/opus/COPYING) | `01e1167d54a096d123cf6dfbbeb19587278845c6481d2d66d545669846079551` |
| libopus_sys-opus-LICENSE_PLEASE_READ.txt | [source](https://docs.rs/crate/libopus_sys/0.3.3/source/opus/LICENSE_PLEASE_READ.txt) | `7efb4989e0cd1b256229bdf2f09300c5d14e35db0e7476bfb87fac243498273d` |
| davey-LICENSE.txt | [source](https://raw.githubusercontent.com/Snazzah/davey/a1e2e741bea06bc3b7167a5c3792844b8975993c/LICENSE) | `90760006b6e6c76a67476de39bee25e42e584679eac5d608765355d5d54e8afa` |
| openmls-LICENSE.txt | [source](https://raw.githubusercontent.com/openmls/openmls/47dbedecad0c1fd8eb5368d582250ebfcc1e1ce6/LICENSE) | `43e5e3c4b5cca67f9ea912f7e1929702a848aa765d3b3e14f25d3838a5a5565d` |
| hpke-rs-LICENSE-MPL-2.0.txt | [canonical Mozilla text](https://www.mozilla.org/media/MPL/2.0/index.txt) | `3f3d9e0024b1921b067d6f7f88deb4a60cbe7a78e76c64e3f1d7fc3b779b9d04` |

Registry package archive checksums from Cargo.lock:

- chacha20poly1305 0.10.1: `10cd79432192d1c0f4e1a0fef9527696cc039165d729fb41b3f4f4f354c2dc35`
- cpal 0.18.2: `6f02e8d0327b42d3e2e4ab2119af397344eb9fc54a34bf0ddeaa1277af8681f1`
- davey 0.1.4: `25028cc2ec8cd43138ca0d2c71d7f6019196238ebd6edea11dd35276fa85b860`
- libopus_sys 0.3.3: `b81c32f233fb2507347a93f97b9919493be9db7ec2249ce2c3ed0c89ad8edf60`
- openmls 0.8.1: `dcb512bfe6a55777518853ea535c6241f069cb0e8984678c117151d2a1e7e903`
- opus2 0.4.0: `49521e33fbf825d2abc8d696506c278cb8469d0c6cc05d3785bef5c5f7ee957b`
- rtrb 0.4.0: `9278fb35b3e730abe136e9b395b5b81b96d06b9f5478a50f0c8430a2237b22de`

Vendored hpke-rs 0.6.1 followup: the [release-pinned manifest](https://raw.githubusercontent.com/cryspen/hpke-rs/f3463e7530771d7f7116635335c25e7d2d11e861/Cargo.toml) declares **MPL-2.0**, not MIT/Apache. Neither its registry archive nor the complete pinned Git tree includes a LICENSE/COPYING/NOTICE file. The unmodified canonical Mozilla MPL-2.0 text linked above is therefore supplied in this directory and `vendor/hpke-rs/LICENSE-MPL-2.0.txt`; it is not represented as an upstream repository file. The original source and manifest notices are retained. `vendor/hpke-rs/SEREIN-PATCH.md` identifies the local SHAKE dependency replacement and adapter. Distributors of the modified component must make its corresponding source, including modifications, available as required by MPL-2.0 and tell recipients how to obtain it. This addition does not complete the remaining transitive license review.

## RustCrypto SHAKE backport dependencies
- `keccak-LICENSE-APACHE`: unmodified from crates.io keccak 0.1.6 / `LICENSE-APACHE`; SHA-256 `a9040321c3712d8fd0b09cf52b17445de04a23a10165049ae187cd39e5c86be5`.
- `keccak-LICENSE-MIT`: unmodified from crates.io keccak 0.1.6 / `LICENSE-MIT`; SHA-256 `bdebaf9156a298f8fdab56dd26cb5144673de522d80f4c0d88e0039145f147f9`.
- `sha3-LICENSE-APACHE`: unmodified from crates.io sha3 0.10.9 / `LICENSE-APACHE`; SHA-256 `a9040321c3712d8fd0b09cf52b17445de04a23a10165049ae187cd39e5c86be5`.
- `sha3-LICENSE-MIT`: unmodified from crates.io sha3 0.10.9 / `LICENSE-MIT`; SHA-256 `f18f6229547ab07f0b7b3e1f83acad8bb436f5f4c95a8a98b44f876caa00f04e`.
