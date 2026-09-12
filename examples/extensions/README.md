# Making a Serein extension

These are complete offline examples and a small Rust SDK. Copy a plugin directory and the
SDK into your own repository, edit its manifest and action, and publish the resulting
`.serein-extension` file. Any language producing compatible WebAssembly can use this ABI;
Rust authors can call `serein_extension_sdk::export!(handler)`.

Build and package from this directory (Python 3 is used only by the author):

```powershell
rustup target add wasm32-unknown-unknown
cargo build --locked --release --target wasm32-unknown-unknown
python pack.py composer-uppercase/manifest.json target/wasm32-unknown-unknown/release/composer_uppercase.wasm packages/composer-uppercase.serein-extension
python pack.py message-word-count/manifest.json target/wasm32-unknown-unknown/release/message_word_count.wasm packages/message-word-count.serein-extension
```

Import the package in Settings > Extensions, review the capabilities, and enable it.
Uppercase composer proposes a replacement that the user must Apply; word count appears in
the selected message's extension menu and returns a native panel. Neither can send messages.
The author packages compiled bytes; Serein never runs a repository's build scripts.

## ABI version 1

Export a 32-bit linear `memory`, `serein_alloc(i32 length) -> i32 pointer`, and
`serein_invoke(i32 pointer, i32 length) -> i64 output`. The host writes UTF-8 JSON into the
allocation and invokes the action. Pack the response pointer in the high 32 bits and its
byte length in the low 32 bits of the returned i64. A fresh instance is used each time;
memory and leaked ABI buffers are destroyed afterward. Do not import WASI or any functions.

Input fields are `action`, optional `selected_message`, optional `composer`, optional
`storage`, and `values` (input IDs mapped to strings; checkbox values are `true`/`false`).
Only the explicitly selected action's context is included and only after capability consent.
Output fields are optional `replacement`, optional `storage`, and `panel` (array).
Storage is one opaque UTF-8 value, replacing the previous value when present.

Panel elements use the `type` tag: `text` (`text`), `row` (`children`), `button` (`id`, `label`),
`text_input` (`id`, `label`, `value`), and `checkbox` (`id`, `label`, `checked`). A button's ID
must name a manifest action with `surface: "panel"`. Panel actions receive current input
values and granted storage; they do not receive a previous message or draft context.
IDs must be lowercase ASCII letters, digits or hyphens, start with a letter/digit, and be
at most 64 bytes; reserved Windows device names are rejected.

Limits: 4 MiB Wasm, 16 MiB JSON package, 16 MiB linear memory, 5 million execution fuel,
128 calls, 256 KiB interpreter stack, 256 KiB serialized input/output, 64 panel elements,
8 row nesting levels, 4 KiB text/input values, and 16 manifest actions. Storage has a 1 MiB
disk ceiling; because it travels in the invocation, it must also fit the 256 KiB I/O budget
alongside other fields. Exceeding any limit is an error, never silent truncation.
Wasmi's strict compilation limits also apply. Plugin panics and exhausted fuel produce a
visible error. Disabled plugins lose their package and stored data; re-enable starts fresh.

For catalog inclusion, submit a manifest, source commit (40/64 hex), immutable HTTPS release
URL, exact `download_bytes`, and SHA-256. Maintainers must review the source and built
artifact together before listing that version. These examples are source templates, not
an automatic trust designation. All versions and updates require explicit user consent.
