# Guild voice evidence

All images are native macOS screenshots of explicitly offline `--demo` builds; no account, microphone or Discord server was used. Baseline `619071c` was built before edits with Rust 1.98.1. The before/after pair uses the unchanged standard fixture at the same approximately 1088×768 viewport, dark appearance and 2× display scale: the formerly disabled hangout voice row becomes selectable.

The `roster.png` image uses the additional `--demo --demo-voice` fixture. Its names, avatars, three participants, mute/deafen flags and elapsed time are synthetic. This demonstrates the new roster presentation, not a live connection. The text-only build remains explicitly selectable; the voice build supplies real media separately.

For native automation only, temporary copied app bundles use distinct bundle IDs/names and local ad-hoc signatures so the computer-use tool can distinguish baseline and changed processes. Their application code and fixtures are unchanged. These copies are ignored build artifacts and are not distributed.
