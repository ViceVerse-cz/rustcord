# Inline video on macOS and Linux, overlay player design

Captured on macOS from `cargo build -p serein --features demo` with the offline fixture
flags (`--demo --demo-video`, `--demo-video-paused`, `--demo-video-playing`), downscaled 50%.
The synthetic MOV fixture decodes through VideoToolbox; no account media is involved.

- `after-idle.png`: stage with the centered play button and duration badge, no card chrome.
- `after-paused.png`: filename pill, centered play button and the translucent bottom bar
  with seek, elapsed/total time and volume drawn over the picture.
- `after-ended.png`: replay affordance after the clip finished.
