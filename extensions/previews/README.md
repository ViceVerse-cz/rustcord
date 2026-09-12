# Starter catalog previews

These PNGs are native wgpu framebuffer captures of Serein using offline synthetic
chat data. Ocean applies the committed theme; both plugin images execute the
committed WebAssembly examples and show their actual result panels. No real
account, service message, microphone or camera was used.

Reproduce with the existing native fixture (from the repository root):

```sh
cargo build --locked --release -p serein --features demo --example profile_preview
# Add --extension=serein-ocean, composer-uppercase, or message-word-count.
# --thumbnail limits the saved image to 640x360 on the screenshot worker.
```

The creator owns the preview: use original or appropriately licensed PNG/JPEG
images, then add its immutable URL, SHA-256 and exact byte count to the catalog.
See [the extension guide](../../docs/extensions.md#shop-previews) for limits.
These images describe the listed examples; previews do not verify compatibility.
Original Serein content follows the repository MIT OR Apache-2.0 license; bundled
font/icon attribution remains in THIRD_PARTY_NOTICES.md.
