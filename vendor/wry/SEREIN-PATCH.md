# Windows/macOS-only Wry dependency selection

Source: crates.io Wry 0.57.0. Original archive SHA-256: `a819957a01b3119af85e638a38d242af76dbc87d130dca67bfd0441072e21ff0`.
The archive records upstream revision `792d0359ba6501a4fc360ece17de2ae42329a47c` with a dirty
working tree, so the published registry archive is authoritative, not that Git tree.
Original Cargo.toml.orig, VCS metadata, all src files and upstream license texts are retained.

Serein now implements Linux authentication directly with GTK4/WebKit6. This fork removes the
old Linux/BSD GTK3, WebKit2GTK, JavaScriptCore and Soup dependency declarations, including their
feature edges, so unsupported backends are absent from the all-target lockfile. The old x11
and linux-body feature names remain empty/compatibility declarations; the build script
explicitly rejects targets other than Windows/macOS. No src file or Windows/macOS backend
implementation is changed. The unused upstream dev dependencies and example declarations are
omitted, along with examples and the upstream lockfile; Serein's workspace lockfile is used.
This fork is not a general replacement for upstream Wry and does not claim mobile/BSD support.

Merely target-gating upstream Wry in Serein does not remove its old target dependencies from
Cargo.lock. This is a real Linux backend replacement, not an advisory ignore or package-version
relabeling. Preserve the MIT/Apache-2.0 notices in binary packages. Remove this fork when a
compatible released Wry eliminates the old bindings or permits their complete exclusion.

Sources: <https://crates.io/crates/wry/0.57.0>,
<https://github.com/tauri-apps/wry/pull/1530> (upstream GTK4/WebKit6 migration).
