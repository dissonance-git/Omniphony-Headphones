---
name: github-workspace-liveness
description: Keep long-running Omniphony GitHub-connector work moving under concurrent main pushes, context pressure, inspection backlogs, publication races, and repeated connector failures without discarding useful staged progress.
---

# GitHub workspace liveness

This is the scheduling subskill for `github-workspace`. It does not decide semantic compatibility and does not publish Git.

> **Preserve progress; bound retries; coalesce awareness; checkpoint before exhaustion.**

## Freeze resistance

### Remote refresh storms

Do not chase every intermediate head.

```text
last accepted head
→ newest observed main
→ one compare
→ one continuation-frontier decision
```

Intermediate commits remain in Git history without each becoming a mandatory restart.

### Deep-inspection floods

Process deterministic attention candidates in bounded waves. Preserve the uninspected tail explicitly rather than truncating it.

### Publication race livelock

After a bounded number of compatible lost races, checkpoint ephemeral task state and staged blob identities, stop immediate retries, and resume from the newest head.

Never force-push and never restart the whole task merely because `main` is busy.

### Connector degradation

Repeated identical tool failure is not progress. Checkpoint, try another admitted route, and preserve the explicit capability block.

### Context pressure

Checkpoint before exact task state becomes fragile. Prefer staged Git blob SHAs for exact candidate bytes.

Checkpointing is not completion and not a durable workspace ledger.

## Multi-agent awareness

Classify concurrent work by both interference risk and potential information gain.

```text
compact awareness of intervening commits
→ deep-read only relevant candidates
→ absorb useful work
→ invalidate only affected support
→ preserve unaffected overlay
```

Another agent may improve the active task unexpectedly.

## Capability debt

Use `codex-handoff` only when the remaining material obligation requires a capability the current environment actually lacks. Repository churn is not a capability block.
