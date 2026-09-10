Build `cargo xtask package` using Rust 1.98.1 MSVC and Visual Studio C++ build tools. The text-only executable is `dist/serein.exe`; distribute it with the adjacent docs and licenses. The login flow requires WebView2. `cargo xtask package-voice` additionally needs CMake and stages the optional DM voice build in `dist/voice`.

Run `dist/serein.exe --demo` for an offline synthetic preview with no saved-login lookup or account storage. Run without `--demo` only when the owner is ready to operate their account. See [platform support](../../docs/platform-support.md) for the live-test boundary and limitations.

September 10, 2026: Windows x64 workspace checks pass, including all 70 offline Rust tests. The text release created a responsive native window in a process smoke check; visual interaction could not be inspected because the Computer Use helper was unavailable. Installer creation, signing, native Save As, authentication, physical audio and accessibility remain unverified.
