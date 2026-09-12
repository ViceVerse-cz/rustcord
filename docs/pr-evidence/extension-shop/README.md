# Native extension shop evidence

Baseline d00bd04, after ae697f4. These are actual native wgpu framebuffer captures
of offline synthetic data, not browser renders or generated mockups.

```sh
cargo run --locked --release -p serein --features demo --example profile_preview -- --demo --page=extensions --width=1120 --height=760 --output=after.png
```

Add `--light` for light mode, `--themes` for the theme tab, or `--width=600`
for the narrow window. The baseline example harness was extended only to seed
the existing starter catalog and expose the Extensions page; application UI
was unchanged. The after harness preloads the three checked-in images before
rendering; host unit tests separately exercise offline catalog/image loading.
The capture callback reads this application's GPU framebuffer only. Native
Computer Use was unavailable (OS error 2); no desktop input automation is claimed.

`metrics.json` records one warmup and five alternating capture-workload runs
per revision. This includes startup and PNG output; it is not idle CPU, p95
frame timing, GPU memory, or live-service evidence. Package snapshots precede
the final measurement note. See ../../performance.md for the compact comparison.
