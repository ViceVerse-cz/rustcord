# Authentication dependency notices

Collected September 10, 2026 from the exact locked registry releases. Wry's unmodified
MIT/Apache-2.0 texts are copied from vendor/wry; its patch note records archive provenance.
The GTK4, GLib and Soup3 MIT texts come directly from their registry source archives.
WebKit6 and JavaScriptCore6 archives omit standalone license files; their root MIT licenses
were retrieved from the exact upstream commits recorded in their respective VCS metadata.

| Component | Source | License text SHA256 |
| --- | --- | --- |
| gtk4 0.11.4 | [f03aab95](https://docs.rs/crate/gtk4/0.11.4/source/LICENSE) | `8cf56d10131ce201cf69ab74b111d3ebac1acca3833d7efb39ae357224b70edb` |
| glib 0.22.9 | [dbeadbd6](https://docs.rs/crate/glib/0.22.9/source/LICENSE) | `8cf56d10131ce201cf69ab74b111d3ebac1acca3833d7efb39ae357224b70edb` |
| soup3 0.9.0 | [43121bc2](https://docs.rs/crate/soup3/0.9.0/source/LICENSE) | `7ade43ec5e1a5e43000aa871d290dff4c0e9e2e16b7f6bea2ecf091e3d056d31` |
| webkit6 0.6.1 | [16c6035a](https://gitlab.gnome.org/World/Rust/webkit6-rs/-/raw/16c6035a19f747322b1814b02ca482e12e23f5fd/LICENSE) | `44dad7b2e199b0d355adf4437df5c9bb18633d47804242f24dab5a633faba5f7` |
| javascriptcore6 0.6.0 | [33ddd822](https://gitlab.gnome.org/World/Rust/webkit6-rs/-/raw/33ddd822d2665546ee9d69ae006b5ad7541ef8a1/LICENSE) | `44dad7b2e199b0d355adf4437df5c9bb18633d47804242f24dab5a633faba5f7` |

These are Rust binding/component notices, not relicensing of WebKitGTK/GTK or system libraries.
The binding licenses are distinct from the system engines' LGPL/BSD and other vendor terms.
The system libraries are prerequisites, not bundled by the current Linux package command.
Both text/voice packages retain this small notice collection; full per-artifact transitive
license-text assembly remains a release gate.
