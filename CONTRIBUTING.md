# Contributing

Read SPEC.md and AGENTS.md first, including the final owner-approved webview and local-storage changes. Build with the pinned toolchain and committed Cargo.lock.

Run `cargo xtask check`, `node tests/login-handoff.cjs`, and `cargo replay`. Default tests must be synthetic/offline; SQLite tests use temporary disposable data. Never put actual Discord credentials into tests, CI, screenshots or reports. Keep the normal-user live gate manual and owner-controlled.

Keep UI, model, protocol, storage and transport boundaries clear. Bound item counts and bytes, propagate explicit errors, preserve unrelated work, and document untested behavior. Changes to authentication, local storage or native dependencies must update the corresponding docs and risk notes. Original contributions use MIT OR Apache-2.0.

For dependency changes, install `cargo-deny` with
`cargo install cargo-deny --version 0.20.2 --locked`, fetch sources with `cargo fetch --locked`,
also run `cargo fetch --locked --manifest-path fuzz/Cargo.toml`,
then run `cargo xtask licenses` and `node tests/license-policy.cjs`. CI runs these separately
from native builds. The offline check covers all features and platforms in the locked graph,
including development and vendored dependencies. License findings fail the dedicated license CI job, independently of packaging; version-specific
exceptions in `deny.toml` require source/notice review when upgraded. This checks declared license
policy, not complete per-artifact license-text assembly or external system-library obligations.

Run `cargo xtask fuzz` for bounded, offline, coverage-guided decoder/state smoke tests after
installing the pinned development tools described in [fuzz/README.md](fuzz/README.md).
Fuzzing uses its own lockfile and nightly toolchain; normal application builds remain stable.

Agent implementation requests follow the [idea-to-PR contract](AGENTS.md) and the
[delivery skill](.agents/skills/serein-delivery/SKILL.md). Native UI changes include synthetic
before/after screenshots; runtime changes include comparable release performance evidence.
Use the PR template and leave blocked checks/evidence visible in a draft. Do not merge automatically.


Use a separate Cargo target directory for each worktree (the default `target/` already does
this). If `CARGO_TARGET_DIR` moves builds to another drive, give each worktree its own path;
sharing one directory can reuse stale application/test artifacts across simultaneous edits.
`xtask` resolves the invocation's Cargo workspace at runtime, including nested working
directories, instead of embedding the checkout where its cached executable was compiled.
After `cargo xtask check`, `node tests/xtask-workspace.cjs` checks that behavior without
building or accessing an owner session.


Run `cargo replay --soak 120` for sustained offline lifecycle pressure (default 60 seconds,
accepted range 1..3600). The workload generates one bounded history page/live event at a time
across 32 synthetic DMs, exercises row/byte eviction, edit/delete races, stale requests,
reconnect/resync and logout, and asserts the shipping reducer budgets. It keeps fixed-size
counters and ranges instead of retaining a trace. CI runs a five-second smoke; longer local
runs are needed for sustained evidence. Output reports actual duration, visits and logout
cycles. This is a core-only workload: no Discord traffic, storage, GUI, image or audio devices.
