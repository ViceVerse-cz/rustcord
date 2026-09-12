# Server settings layout evidence

Synthetic/offline data only. The existing `profile_preview` example captures the
actual eframe/WGPU framebuffer; it does not create a service connection. These
images are renderer evidence, not native input/accessibility or live Discord proof.

- Baseline UI: `2f9b770`, with only the server fixture hook added to the preview example.
- Wide before/after: 1600 x 1000 logical viewport, dark theme, Windows display scale 125%.
- Narrow after: 760 x 900 logical viewport, light theme.
- Engagement after: the same wide dark viewport.
- The preview uses the existing synthetic server settings fixture. Icons/counts/text
  are intentionally synthetic, rather than copied from the owner's live server.

```powershell
cargo run --locked -p serein --example profile_preview --features demo -- --demo --page=server --width=1600 --height=1000 --output=docs/pr-evidence/server-settings-layout/after.png
```

Use `--page=server-engagement` for Engagement and `--width=760 --height=900 --light`
for the narrow/light case. Images were inspected for clipped columns, row spacing,
input padding, preview placement, and long-label containment.

Windows Computer Use initialization succeeded, but `sky.list_windows()` failed:
`Computer Use native pipe is unavailable ... (os error 2)`. Native mouse/keyboard
and accessibility verification remain outstanding.
