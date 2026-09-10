# Agent orchestration

This repository uses one capable primary session and up to three independent workers.
Start Codex or Claude Code **from this repository**, accept its normal project trust prompt,
and start a fresh session after configuration changes. Describe the feature normally;
`AGENTS.md` and `CLAUDE.md` load the orchestration guidance for implementation work.
No personal configuration, plugin installation, background service or API bridge is required.

```sh
codex                  # project default: gpt-6-astra, high
claude                 # project default: fable, high
```

Existing app model selection, explicit CLI options, user/local settings and organization policy
may override these defaults. Select Astra or Fable in the host if needed. Model availability is
account-dependent. The skill cannot change the current parent model. These are two native
workflows sharing one policy, not a mixed-provider agent pool within a single session.

| Responsibility | Codex | Claude Code |
| --- | --- | --- |
| Lead: scope, architecture, difficult decisions, acceptance | Astra high | Fable high |
| Implementation | Sol medium | Sonnet high |
| Research and independent review | Terra high | Sonnet high |
| Checks, native screenshots, performance, mechanical conflicts | Luna max | Sonnet medium |

The routing is a starting policy, not an empirical quality or cost ranking. Claude uses its
supported native model families rather than trying to invoke Luna/Terra. Haiku is not the
verification default: these tasks can require Rust failure diagnosis and evidence interpretation.
A simple edit stays with the lead. Semantic conflicts and security/concurrency decisions go
back to the lead. No worker recursively delegates. The lead does useful independent work
while workers run, reviews their output and owns the final PR.

Codex reads `.codex/config.toml` and `.codex/agents/` in compatible trusted local hosts.
Claude reads `.claude/settings.json`, `.claude/agents/` and root `CLAUDE.md`, which imports
`AGENTS.md`. The shared policy is `.agents/skills/serein-orchestration/SKILL.md`; Claude reads
it by path, so it does not need a duplicate skill or slash-command installation.
If your Codex host exposes generic model-selectable spawning rather than named project agents,
the shared skill supplies explicit model/effort pairs and fresh task packets.

## Operating rules

Assign each worker exact files, a baseline, acceptance criteria and evidence to return.
One owner per file; one owner for the native UI; serialize builds sharing a Cargo target.
Benchmarks run without competing builds. Preserve the before state before editing visual or
runtime behavior. Follow the delivery skill for synthetic screenshots and actual measurements.
Only the lead integrates, commits, pushes and opens the PR. An unresolved check is not success.

The maximum of three workers is configured in Codex; Claude follows it as an instruction.
Claude workers have a 24-turn limit; a partial result returns to the lead for deliberate
continuation, not automatic completion. Permission settings are inherited, not relaxed.
The researcher has restricted read tools in Claude and a read-only sandbox request in Codex;
host overrides can affect enforcement, so the read-only task constraint still applies.

Model selection is requested until runtime evidence confirms it. If unavailable, the lead
reports the limitation and performs the bounded task itself. Do not add nested CLI sessions
or silently replace models to conceal failure. No cost savings are claimed without observed
usage and an appropriate comparison. Configure lower effort only after actual task experience.

## Verification and limits

Configuration was authored against Codex CLI 0.153.4 and Claude Code 2.1.267 on macOS.
Validation results and any outstanding checks are recorded in `docs/progress.md`.
Static parsing and CLI diagnostics do not prove account access or live model routing. The
current Codex session exercised explicit Terra/high delegation. Claude foreground smoke probes
resolved the project parent to `claude-fable-5-1` and all three named agents to
`claude-sonnet-5`. In-session Claude delegation, tool restrictions and actual effort remain
unverified. Existing sessions may not reload these project files.

To inspect routing in a fresh session, ask: “Use the Serein orchestration skill. Have one
researcher trace the timeline caller path read-only while you inspect the relevant tests;
report the requested and observable model/effort.” Check the host's agent details. Then use a
real bounded feature to evaluate quality, elapsed time and available usage; don't manufacture
a savings percentage from this configuration alone.

## Sources and design

Reviewed September 10, 2026:

- [Astra Advisor](https://github.com/DannyMac180/astra-advisor): inspiration for bounded
  delegation and lead-owned acceptance. This repository contains original instructions, not
  a vendored plugin or its cost calculator.
- [Codex subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents): native
  project agent TOMLs, model/effort overrides and concurrency configuration.
- [Claude Code subagents](https://code.claude.com/docs/en/sub-agents): project agent Markdown,
  tool selection, effort, turn limits and worktree baseline behavior.
- [Claude Code model configuration](https://code.claude.com/docs/en/model-config): Fable alias,
  model availability, precedence and effort levels.
