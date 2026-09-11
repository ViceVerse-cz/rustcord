# Own profile editor evidence

All images contain synthetic offline data, rendered by native egui/wgpu on Windows.
`before.png` is the read-only My Account page from baseline `22e2283`; the baseline
has no Profile editor. `after.png` is the new Profile page at the same 1120 x 760
logical viewport, 125% native scale. `light.png` changes only the theme;
`narrow.png` uses 760 x 900 logical pixels and places actions before the preview.

Reproduce the final captures with:

```powershell
cargo run --locked -p serein --example profile_preview -- --demo --output=after.png
cargo run --locked -p serein --example profile_preview -- --demo --light --output=light.png
cargo run --locked -p serein --example profile_preview -- --demo --width=760 --height=900 --output=narrow.png
```

The baseline used the same capture example, omitting the new `prime_profile`
function and invocation, with `--page=account`. The example uses the renderer's
actual screenshot callback because the desktop automation pipe was unavailable
(`os error 2`). It renders the application's widgets without network adapters;
its commands are ignored. Use the main binary's `--demo --demo-settings=profile`
to exercise synthetic saving. Keyboard entry, Save/Cancel, duplicate suppression
and failed-save draft preservation are covered by headless egui tests. Native
input automation, screen readers and live Discord writes were not tested.

Raw measurements are in [metrics.json](metrics.json); methodology, comparisons
and limits are in [performance.md](../../performance.md). Package snapshots omit
this final evidence addendum. Both release variants compile, but complete voice
packaging fails on baseline and changed sources because exact-version OpenH264
license texts are missing. The unchanged pending-message color test also fails on
the clean baseline. The PR remains draft for these blockers.
