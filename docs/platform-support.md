# Platform support and packaging

Target platforms are Windows, macOS and Linux. **macOS arm64, Windows x64 and Linux x64 have local build evidence.** macOS has native visual checks; Windows has offline tests and a process/window startup smoke check only. Minimum OS versions, other architectures, real screen-reader support and native login-method support are not certified.

| Platform | Build/runtime requirements | Status |
|---|---|---|
| macOS | Rust 1.98.1, Xcode command-line tools; Metal/wgpu, system WKWebView, Keychain | Local arm64 build and native synthetic window tested on macOS 27.0 beta, Apple M1 Pro / 16 GiB |
| Windows | Rust MSVC toolchain, Visual Studio C++ build tools, system graphics drivers, WebView2 Runtime 101+ (current supported runtime recommended), Credential Manager | Local x64 checks and unsigned release packaging on Windows 11 build 26200; synthetic process/window startup passed. Visual interaction, InPrivate behavior, IME and accessibility unverified |
| Linux | Rust, C compiler, pkg-config, GTK >=4.10, WebKitGTK 6.0, fontconfig, libxkbcommon, X11/Wayland development packages, Vulkan-compatible GPU/driver, Secret Service session bus/keyring | Ubuntu 26.04 x64 / WSL2 text and voice releases and Debian package smoke passed; X11/Wayland rendering and login window unverified |

Debian/Ubuntu development packages typically include `build-essential pkg-config libgtk-4-dev libwebkitgtk-6.0-dev libfontconfig1-dev libxkbcommon-dev libwayland-dev libx11-dev libxi-dev libxrandr-dev libxcursor-dev libvulkan-dev`. Package names vary by distribution. SQLite is bundled through rusqlite; it is an embedded client cache, with no database service.

Linux packaging also requires `python3 dpkg-dev desktop-file-utils`.

`cargo xtask package` builds the locked default release configuration. macOS gets `dist/Serein.app`; Windows gets an executable plus license files; Debian/Ubuntu Linux additionally produces a `.deb` with desktop integration and dependency metadata. On macOS, packaging replaces the executable through a fresh sibling file and rename, then seals the completed bundle with `codesign --force --sign -` and runs `codesign --verify --strict`. This is a **local ad-hoc signature**, with no signing identity, Developer ID certificate, or notarization. It verifies the staged bundle's integrity and does not certify Gatekeeper acceptance or a trusted publisher. The distinction between signature validity and trust is described in [Apple's code-signing guidance](https://developer.apple.com/library/archive/technotes/tn2206/_index.html).

Windows/Linux staging artifacts remain unsigned. These are not certified installers. Use `ditto -c -k --keepParent dist/Serein.app dist/Serein-macos.zip` on macOS; normal archive tools may package Windows/Linux staging output. Do not modify bundle resources after sealing; rerun packaging when source documentation changes. Debian/Ubuntu package instructions are in [packaging/linux](../packaging/linux/README.md). Windows installer/signing, other Linux distribution formats, macOS Developer ID signing/notarization and release reproducibility remain open work.

The webview lives only during login: WKWebView on macOS, WebView2 on Windows, GTK/WebKitGTK on Linux. Linux uses a separate GTK authentication window and pumps it only while login is active. The text-only build initializes no audio runtime or microphone. The optional voice build opens audio devices only after an explicit call reaches required encrypted readiness; camera access is not implemented. Popup-dependent authentication and third-party embedded challenges may not work; do not claim all Discord login methods without live tests.

## Optional DM voice build

`cargo run --locked --features voice` enables native DM audio. Its build adds CMake and a C/C++ toolchain for statically bundled libopus; Linux also needs ALSA development headers (`libasound2-dev` on Debian/Ubuntu). CPAL uses native system audio. See [the voice adapter](../crates/discord-voice/README.md) for codec/protocol dependencies and limitations.

`cargo xtask package-voice` stages a separate voice-enabled release under `dist/voice` (`dist/voice/Serein.app` on macOS). The default `cargo xtask package` stays text-only. The macOS bundle includes its microphone-use description; actual microphone permission, capture/playback, device switching and sleep/resume have not been exercised. Windows x64 voice release packaging and synthetic protocol/audio tests pass; physical audio and live calls remain unverified on Windows. Linux x64 text/voice release builds and Debian package smoke passed on Ubuntu 26.04 under WSL2; native Linux desktop/audio runtime remains unverified. CMake is a source-build dependency, not a runtime voice service.

Both packages use the same application identity and account cache; they are build variants, not isolated accounts. Device choices and push-to-talk settings last only for the current session. Focused V push-to-talk has no global-key guarantee; use headphones because there is no acoustic echo cancellation. No signing, desktop integration, physical audio or live-compatibility claim follows from compilation alone.

Native emoji use eframe system-font fallback and installed OS color fonts. macOS
Apple Color Emoji was visually checked with a synthetic moon status on September
10, 2026. Windows/Linux emoji coverage is unverified and depends on installed fonts
(e.g. Segoe UI Emoji / Noto Color Emoji); no OS font is redistributed.


## Opt-in minimize to tray (September 11, 2026)

Windows Appearance settings now offer Minimize to tray, off by default. Minimizing
hides the window only after successful Shell icon registration. The icon supports
keyboard/mouse restore and a Show Serein / Quit menu. Quit uses the normal unsaved
work/download exit checks; the window Close button retains normal exit behavior.
Disabling restores a hidden window, and Shell recovery failure leaves it accessible.
The adapter uses existing user32/Shell APIs and dependencies, with no background
polling or autostart. A synthetic native Windows test verifies registration,
hide/restore, own-window taskbar recovery, Quit event and cleanup. Linux/macOS have
an explicitly disabled control; their tray integration is not implemented.
