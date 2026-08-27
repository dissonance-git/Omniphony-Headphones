---
name: codex-handoff
description: >
  Stage concrete Omniphony work that cannot be executed through the current
  chatspace and GitHub connector but can be resumed in a local/Codex-capable
  environment.
---

# Codex handoff

GitHub Issues are the authoritative queue for concrete capability debt.

```text
title prefix: CODEX:
body marker: <!-- omniphony-codex-handoff:v1 -->
```

Do not create a parallel JSON, Markdown, roadmap, or task-file queue.

## When to stage

Use a handoff for an exact actionable blocker such as:

- local build/test execution unavailable;
- Windows/macOS/Linux/Android runtime or kernel/API probe required;
- hardware/physical endpoint measurement required;
- dependency/toolchain installation required;
- binary artifact inspection or signing required;
- CI diagnostics unavailable through the connector;
- a repair whose safety depends on real process behavior unavailable here.

Do not stage:

- ordinary research uncertainty;
- a user decision;
- vague future work;
- work the connector can still complete;
- publication contention;
- a duplicate of an existing open handoff.

## Duplicate rule

Search open `CODEX:` issues for the stable handoff key or exact obligation. Update the existing owner instead of creating a duplicate.

## Required packet

Every issue body begins with:

```text
<!-- omniphony-codex-handoff:v1 -->

handoff-key: <stable-lowercase-kebab-case-key>
priority: P0 | P1 | P2
blocked-interface: <exact boundary>
source-commit: <full SHA>
required-capabilities:
  - <capability>
```

Then include:

```text
## Obligation
Exact work to accomplish.

## Why this is staged
Concrete interface/runtime blocker.

## Already completed
Work/evidence already established through chatspace/GitHub.

## Affected routes
Files, workflows, commits, issues, artifacts.

## Re-entry
First commands/inspection route.

## Acceptance criteria
Observable closure conditions.

## Guardrails
Things that must not be weakened or silently bypassed.

## Evidence to attach before closing
Exact test/runtime output, artifact/hash, commit SHA, or physical result.
```

## Freshness and closure

The source commit is orientation, not overwrite permission.

A local/Codex executor must fetch current `main`, inspect changes since the source commit, preserve concurrent work, and re-run the blocked obligation on current bytes.

Do not close an issue because code was merely written. Attach the evidence satisfying its acceptance criteria. Close obsolete/duplicate work explicitly rather than manufacturing success.

Codex issues are execution-handoff state, not product truth, scientific evidence, or a second roadmap.
