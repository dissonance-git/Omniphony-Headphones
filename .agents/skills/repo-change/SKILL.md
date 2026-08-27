---
name: repo-change
description: Execute a bounded Omniphony repository change safely and prove what actually landed. Use for code, documentation, DSP, schema, test, configuration, cleanup, host, installer, or migration edits, especially when concurrent main or GitHub connector publication matters.
---

# Repository change

This skill is procedural and subordinate to `AGENTS.md`.

## Core invariant

```text
inspect current truth
→ identify one bounded change
→ preserve unrelated work
→ stage through an allowed route
→ validate actual result
→ publish from a fresh base
→ verify publication
```

## Execution route

Use a real local checkout when available. Use `github-workspace` when GitHub is the authoritative transport. Do not require a local checkout merely because one is familiar.

For connector work, `github-workspace` owns exact snapshotting, concurrent refresh, overlays, and fast-forward publication. This skill owns mutation/completion, not a competing concurrency protocol.

## Freeze the base

Before editing, record repository, publication target, source head/tree, paths in scope, protected paths/evidence, acceptance conditions, and required validation.

## Inspect before editing

At minimum inspect root authority, current implementation/document, relevant tests/validators, consumers/schemas when semantics propagate, and recent commits touching the area when churn is plausible.

Determine generators before editing generated outputs. Preserve protected listening-baseline bytes and external/ABI identities unless the task explicitly changes them.

## Smallest sufficient edit

Prefer:

```text
existing owner over parallel owner
one executable invariant over prose-only policy
bounded replacement over unrelated churn
semantic collapse over archive/tombstone proliferation
```

Do not broaden a task merely because nearby cleanup looks attractive.

## Connector-safe publication

One independent text path may use a fresh blob-SHA compare-and-swap.

Coupled work uses:

```text
fresh source head/tree
→ create every replacement blob
→ create one candidate tree
→ create one candidate commit
→ refresh main
→ inspect/absorb intervening work
→ rebuild parent/tree if still compatible
→ fast-forward only
→ re-fetch ref and changed paths
```

Never force-push.

## Validation

Match validation to the claim:

```text
documentation/ownership
→ route/link/contract checks

portable renderer
→ focused compile/test + affected renderer suite

realtime/ABI
→ ABI/lifecycle/non-finite/discontinuity/boundedness checks

Windows host/APO/installer
→ exact task-relevant Windows build/runtime/package gates

perceptual DSP
→ engineering evidence + physical listening
```

A connector write is not a test pass. A pass from another SHA is not evidence for the published bytes.

## Completion

After publication, fetch the target ref, prove the intended commit is current, inspect exact changed paths/content, inspect target-SHA validation, report unexecuted checks separately, and avoid creating a history document for completed work.
