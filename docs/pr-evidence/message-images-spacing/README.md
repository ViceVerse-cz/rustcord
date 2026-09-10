# Synthetic native evidence

Baseline `c4ae54d`; updated task branch. macOS 27.0, Apple M1 Pro, 16 GiB,
Rust 1.98.1, release text-only build, wgpu/Metal, built-in 3024×1964 Retina display.
Screenshots are application-only 1120×760 captures; the exact renderer scale was not
separately instrumented. Local copies use unique bundle IDs only to avoid stale native
computer-use handles after relaunch; production bundle IDs are unchanged.

- `before.png` / `after.png`: identical `--demo --demo-chat` fixture at latest messages.
  The updated conversation fits in the viewport because terminal blank text lines are gone.
- `before-images.png` / `after-images.png`: `--demo`, at latest message. The after fixture
  adds a second original synthetic landscape to demonstrate multiple images. The baseline
  fixture has one attachment image. The larger image above these attachments is an embed.
- `light.png`: same gallery in `--demo --demo-light`.
- `viewer.png`: second attachment opened through its accessible image action; Escape closed
  the modal successfully. Filenames remain useful in this explicit viewer.

Headless egui tests cover 420/240 pt galleries and 900/360 pt dark/light timelines,
including image click targets, aspect ratios, no filename captions, preserved internal
newlines and continuation height. Native window resizing through the available control
surface did not take effect; narrow geometry is verified by these tests, not the light screenshot.
No live account, external download, Discord message, microphone or call was used.
