---
name: github-workspace
description: >
  Operate Omniphony through the GitHub connector as one coherent repository
  workspace. Use for connector-only reading, multi-file edits, concurrent main
  movement, Actions inspection, publication, and re-entry.
---

# GitHub workspace

This procedure is subordinate to repository `AGENTS.md`.

## Core model

```text
Git is repository truth.
Workspace state is ephemeral.
Remote movement is awareness before interference.
Validation belongs to an exact target SHA.
```

The connector is an agent-computer interface. Do not imitate a local checkout by issuing floating file reads one at a time.

## 1. Capability handshake

Before depending on an operation, identify what the current connector can actually do:

```text
read ref/commit/tree/blob/file
search
compare commits
create/update/delete file
create blob/tree/commit
fast-forward ref
inspect workflow/job/log/artifact
rerun existing validation when exposed
issue mutation when exposed
```

Repository control and arbitrary fresh runtime execution are separate capabilities.

## 2. Freeze one exact snapshot

Record:

```text
repository
publication target
accepted-head
accepted-tree
read-set
write-set
dependency-set
protected-set
acceptance conditions
validation target
```

Read mutable files at `accepted-head`, not at floating `main`.

Prefer:

```text
branch ref
→ exact commit
→ exact tree/scope
→ exact blob/file
```

Search locates candidates. It does not prove absence or establish current bytes.

## 3. Orient progressively

Build the smallest map that can change the next decision:

```text
owner/path
→ relevant symbols/headings
→ imports/consumers/tests
→ exact excerpts
→ whole file only when necessary
```

Mark unobserved scope as unknown. Truncation, pagination, and ranked-search omission are not absence.

Batch independent reads against the same pinned ref when possible.

## 4. Stage an overlay

Treat proposed changes as an overlay on `accepted-head`.

### One independent text file

A contents-style compare-and-swap write is acceptable only when the file's current blob SHA is fresh and no coupled owner must move with it.

### Coupled change

Use Git objects:

```text
accepted tree
→ create all replacement blobs
→ create one candidate tree
→ create one candidate commit
```

Do not publish half of a coupled contract just because contents updates are convenient.

Keep candidate blob SHAs/content hashes as disposable re-entry handles.

## 5. Refresh awareness

Before publication and after any material interruption:

1. fetch newest `main`;
2. if unchanged, continue;
3. if moved, compare `accepted-head...new-head`;
4. inspect compact intervening commit/path summaries;
5. deepen only where active read/write/dependency/protected state or useful new context requires it.

Classify movement as:

```text
remote-context-available
  disjoint and no premise invalidated

refresh-context
  supporting premise changed

write-overlap-review
  intended write changed remotely

protected-owner-changed
  README/AGENTS/contract/governance premise changed

history-diverged
  not a simple fast-forward relationship
```

A moved head does not automatically invalidate staged semantic bytes. Rebuild the Git parent/tree on the newest accepted head when the meaning still holds.

Path-disjoint remote commits can contain positive interference. Adopt a better newly landed owner/test/implementation when it materially improves the current task instead of reverting it to recover the old snapshot.

## 6. Publication churn

Never force-push.

If multiple compatible publication races occur, bound immediate retries. Preserve the overlay and exact staged blobs rather than entering an infinite refresh/rebuild loop.

Contention alone is not a Codex capability block.

## 7. Publish

For a coupled candidate:

```text
refresh main
→ ensure accepted premises still hold
→ create/rebuild candidate on newest accepted tree
→ create commit with parent = newest accepted head
→ refresh main once more
→ fast-forward ref only
→ re-fetch main
```

Substantial direct-main commits should include:

```text
omniphony-task: <kebab-case-key>
omniphony-change-kind: <actual-kind>
omniphony-validation: <actual-state>
omniphony-handoff: <optional issues>
```

Trailers are routing hints, not proof.

## 8. Verify

After publication:

- verify target ref equals the intended commit;
- inspect exact changed paths/content;
- confirm unrelated concurrent work remains;
- inspect Actions/status for that exact SHA;
- report only validation that actually executed.

Distinguish:

```text
workflow not planned / no job
runner/backend failed before steps
workflow runtime failed
test/build step failed
completed success
```

Do not rewrite tests or workflow requirements merely to hide backend/startup failure.

## 9. Re-entry

For a long interruption preserve only the smallest sufficient task capsule:

```text
goal
accepted-head/tree
read/write/dependency/protected sets
staged blob identities
what is verified
validation target/state
remaining capability blocks
next action
```

This capsule is ephemeral and may live in conversation/task state. Do not create a durable workspace database.

On resume, fetch current `main`, compare from the accepted head, rehydrate invalidated premises only, recover staged blobs if useful, and continue.

## 10. Capability debt

If all repository-native work is complete but an actionable step requires unavailable local execution, OS/hardware access, dependency installation, binary inspection, or inaccessible runtime diagnostics, invoke the `codex-handoff` procedure.

Do not downgrade evidence just because execution is unavailable.
