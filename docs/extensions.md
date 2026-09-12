# Community extensions

Serein extensions are local, opt-in tools for the native client. The Extensions
page in Settings contains plugins and themes, links to their source, their
requested capabilities and their review status. A plugin cannot call Discord,
send a message, read credentials, open files or make network requests.

## Install and remove

Open Extensions to browse the free catalog; Refresh explicitly checks it again.
Enable downloads the selected, hash-pinned package after its capabilities have
been accepted. Updates are manual and require renewed capability consent.
Import selects a local `.serein-extension` JSON package. An import is unreviewed;
importing alone does not grant it capabilities or execute it.

Disable stops accepting results immediately, then removes Serein's downloaded
package, temporary files and extension data. A failure to remove files is shown
and cleanup is retried on the next load. Re-enabling requires downloading or
importing the package again and starts with fresh extension settings. Serein
never deletes the creator's Git repository or the user's imported original.

Plugin grants and data belong to the signed-in account. Logout invalidates
plugin results, drains bounded in-flight work and clears that account's extension data. Theme selection is a device
preference. There are no background catalog refreshes or automatic updates.

## Creator workflow

1. Keep source and license in a public Git repository. Use the standalone Rust
   starters under `examples/extensions` for a composer tool and message tool.
2. Build a Wasm module implementing the version 1 ABI documented by the starter.
   No native binary, installer, Git hook or build script runs on an end user's
   computer. Other languages can implement the same Wasm buffer/JSON contract.
3. Package the manifest and Wasm bytes (or declarative theme) as a single JSON
   file. Test through Import with an offline `--demo` build first.
4. Publish the package as a versioned release artifact. Submit a pull request
   changing `extensions/catalog.json`, with the source commit, build procedure,
   license, artifact URL, byte size and SHA-256 digest.
5. Maintainers review each listed version, its capabilities and the source to
   artifact relationship. A catalog checksum identifies reviewed bytes; it is
   not a signature or a guarantee that code is harmless. Updates need review too.

The in-app catalog reads the default branch. A new catalog entry is not publicly
available through that endpoint until its pull request is merged. Empty catalogs
are valid; imports allow development before a release is listed.

## Host contract

The `extensions` crate defines the versioned manifest, capability, action,
invocation, result and theme types. These serialized types are the compatibility
boundary; internal `client-core` structures and egui objects are not an SDK.
Unknown API versions and invalid packages are rejected before installation.

Actions are invoked by a message context-menu item, composer tool or panel
button. Input is restricted to the granted context and bounded form values.
Results can propose a composer replacement or return native text, rows,
buttons, text inputs and checkboxes. Composer proposals require Apply, retain
the ordinary Send action and are discarded when their originating context is
stale. Account/session changes invalidate outstanding results.

Themes override existing named palette colors for light/dark appearance, with
the existing two-color backdrop supported. They contain no code, CSS, fonts,
images or URLs to fetch. Built-in colors fill omitted tokens and the existing
user accent setting takes precedence. Reset returns to a built-in appearance.

## Resource and privacy limits

Wasm executes on an on-demand background worker with fuel, stack and memory
limits and no WASI or host imports. Each invocation gets a new runtime; it is
dropped when the call completes. Plugin execution never happens in a render or
audio callback. The application bounds package sizes, installed plugin count,
invocation input/output, panel complexity, queues and plugin storage.

Disabled plugins have no retained instance, worker, package or plugin data once
cleanup succeeds. Shared host code and bounded catalog metadata still cost some
application space. Freed allocations may remain in the process allocator; an
unchanged RSS reading alone does not mean an instance remains active.

Plugin storage is ordinary local data, not encrypted by Serein. Do not use it
for credentials. No extension diagnostics or private invocation data are
uploaded. Sandboxing and review reduce exposure but cannot prove absence of
bugs in the runtime or host; keep Serein updated.

The complete ABI and numeric limits are documented in
[`examples/extensions/README.md`](../examples/extensions/README.md). The demo uses
a separate bounded temporary `serein-extension-demo` profile; it can import local
fixtures but cannot download a catalog or package. `Ctrl+Shift+F12` resets a
community theme if its colors make controls difficult to read.
