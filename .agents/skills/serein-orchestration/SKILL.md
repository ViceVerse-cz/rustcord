---
name: serein-orchestration
description: Coordinate Serein implementation, research, conflict resolution and verification across native Codex or Claude Code agents. Use for repository feature/fix delivery and multi-agent review; handle trivial edits directly.
---

# Serein orchestration

The primary session is the lead: preferably Astra in Codex or Fable in Claude Code.
It owns scope, architecture, task assignment, corrections, integration and final acceptance.
Follow root `AGENTS.md`, `SPEC.md`, subtree instructions and the Serein delivery skill.
This workflow authorizes useful delegation, not extra product scope or external actions.

## Route by difficulty

| Work | Codex model / effort | Claude agent / model / effort |
| --- | --- | --- |
| Planning, ambiguous bugs, auth, concurrency, storage invariants, final acceptance | Primary Astra / high | Primary Fable / high |
| Bounded implementation with clear acceptance criteria | serein-builder: gpt-5.6-sol / medium | serein-builder / sonnet / high |
| Trace callers, protocol/dependency research, independent diff review | serein-researcher: gpt-5.6-terra / high | serein-researcher / sonnet / high |
| Checks, performance runs, native screenshots, understood mechanical conflicts | serein-verifier: gpt-5.6-luna / max | serein-verifier / sonnet / medium |

These are starting assignments, not measured model benchmarks. Luna max is the owner's
preferred verification/conflict setting; a long build itself needs no model reasoning.
Use the lead for semantic conflicts, competing invariants, security-sensitive changes or
uncertain performance conclusions. Do not launch a second Astra/Fable just to supervise the lead.
For a difficult unresolved decision, raise effort only if supported and useful; no default ultra.

## Capability preflight

Use only the current host's exposed native delegation tools and supported model/effort values.
Project agent files supply defaults in compatible CLIs. In a Codex host exposing generic
`collaboration.spawn_agent`, pass the table's model and reasoning_effort explicitly with
`fork_turns: "none"`, plus a self-contained task packet; task_name is only a label, not a role.
For generic spawning, include the selected role file's restrictions explicitly in the packet,
especially read-only/no edits, no commit/push/delegation and build/GUI ownership. Generic task
labels do not load role TOMLs or enforce their sandbox requests; report enforcement limits.
If named agent selection is exposed instead, select the matching project agent definition.
In Claude Code use its native Agent tool with the named project agent. Do not try to pass GPT
model IDs to Claude agents or Claude model IDs to Codex agents.

Announce the chosen worker, deliverable and model/effort briefly. Requested settings are not
runtime confirmation: distinguish selected, confirmed and unobservable values. Account policy,
CLI flags and app model pickers can override project defaults. Never switch the running parent
implicitly. If a worker model/tool is unavailable, report it and do that bounded work in the
parent; do not retry spawns indefinitely, invent tool arguments, launch nested CLIs or add a proxy.
A worker unable to delegate remains useful. Parent-only completion is a supported path.

## Keep work independent

- Inspect the task and relevant flow first. Delegate only when the parent can make useful
  independent progress. A typo, one-file fix or command already running needs no team.
- Default to one or two workers; at most three concurrently, or the host's lower limit.
  Workers do not spawn workers. This is an instruction cap on Claude, not a scheduler guarantee.
- Each task packet states: goal; exact checkout and baseline; owned files or read-only scope;
  relevant callers/invariants; acceptance criteria; commands/artifacts; stop condition.
  Include root/subtree instructions and the necessary raw evidence. Avoid full chat histories.
- Assign disjoint file ownership. The parent owns shared wiring, docs/progress.md and Git/PR
  operations unless explicitly transferred. Workers never commit, push, merge or edit others'
  files. Pause an owner before reassigning its files. Reuse the same worker for a correction.
- Use the same checkout for disjoint changes. If isolation is needed, create a worktree from
  the task's exact commit and explicitly carry required uncommitted changes; verify its HEAD.
  Do not assume a Claude automatic worktree starts at the parent's current branch.
- Serialize Cargo builds sharing a target directory and all GUI automation. One verifier owns
  the native window. Performance measurement runs without competing builds/tests; capture the
  baseline before edits or from a verified baseline worktree using separate outputs.
- Workers return status (done/partial/blocked), changed paths or findings, commands with exit
  results, artifact paths, uncertainties and the next decision. Keep ordinary reports short;
  preserve full logs as local artifacts without secrets. A turn limit yields partial, not done.

## Correct, verify, accept

Inspect worker diffs and evidence before accepting them. On a concrete miss, send one targeted
correction with the failing case. If the same failure persists, stop and return it to the lead
for diagnosis/replanning; do not cycle through cheap agents guessing at the same problem.
Mechanical conflict resolution must preserve both sides' intent and run affected checks.
Never accept blanket ours/theirs, lockfile guessing, or removed tests as a conflict fix.

The verifier follows the delivery skill for applicable commands, synthetic `--demo` screenshots,
and reproducible before/after measurements. It reports missing GUI/platform tools honestly.
A screenshot is not a performance measurement or live Discord proof. Avoid rebuilding an
unchanged application for docs-only evidence. Run repository-required checks once on the final
integrated state; rerun only checks invalidated by later changes. Parent inspects logs/artifacts
and the integrated diff rather than repeating every successful expensive command.

For complex changes (authentication, permissions, concurrency, persistence, protocol behavior,
or a large multi-worker diff), assign a fresh read-only reviewer the final diff, requirements
and relevant raw tests. A reviewer without shell access must receive the complete final diff
(including deletions) plus command outputs, exits and artifact paths; it can inspect files
but cannot claim to have rerun commands. Its verdict supplements actual validation.
Request `ship`, `fix-first` or `rethink` with concrete paths/evidence;
the lead adjudicates findings, fixes real issues, and requests focused re-review where needed.
Do not require an extra reviewer for trivial documentation or mechanical edits.

Finish the authorized delivery through the existing PR workflow. Report actual checks, evidence
and blockers. If usage is exposed, report it with its coverage; otherwise say cost was unmeasured
when discussing efficiency. Never infer dollars or savings from agent count or model names.
