# Contributing

Read SPEC.md and AGENTS.md first, including the final owner-approved webview and local-storage changes. Build with the pinned toolchain and committed Cargo.lock.

Run `cargo xtask check`, `node tests/login-handoff.cjs`, and `cargo replay`. Default tests must be synthetic/offline; SQLite tests use temporary disposable data. Never put actual Discord credentials into tests, CI, screenshots or reports. Keep the normal-user live gate manual and owner-controlled.

Keep UI, model, protocol, storage and transport boundaries clear. Bound item counts and bytes, propagate explicit errors, preserve unrelated work, and document untested behavior. Changes to authentication, local storage or native dependencies must update the corresponding docs and risk notes. Original contributions use MIT OR Apache-2.0.

Agent implementation requests follow the [idea-to-PR contract](AGENTS.md) and the
[delivery skill](.agents/skills/serein-delivery/SKILL.md). Native UI changes include synthetic
before/after screenshots; runtime changes include comparable release performance evidence.
Use the PR template and leave blocked checks/evidence visible in a draft. Do not merge automatically.
