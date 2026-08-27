# Omniphony development contract

This file is the canonical operating law for `dissonance-git/Omniphony-Headphones`.

> **More capability, fewer conceptual machines.**
>
> **One concept, one writable owner. Store authority. Derive views. Preserve evidence. Promote slowly.**
>
> **Living documents own current obligations. Git owns superseded repository narrative and chronology.**

## 1. Enter through current authority

For substantive work:

```text
current main HEAD
→ README.md
→ AGENTS.md at the same HEAD
→ ROADMAP.md only when unresolved work matters
→ smallest task-relevant living contract
→ exact code / tests / CI
→ target
```

Resolve current `main` before reasoning about mutable repository state. Search is for discovery; exact refs, commits, trees, blobs, files, code, tests, and CI establish the selected snapshot.

Repository publication is main-only unless the user explicitly changes that instruction. Never force-push. Preserve unrelated concurrent work.

## 2. Canonical ownership

Every durable mutable fact gets one writable owner.

```text
README.md
  product identity / public architecture / entry routes

AGENTS.md
  repository / change / evidence / concurrency / publication law

ROADMAP.md
  unresolved gates / blockers / sequencing only

CONTRIBUTING.md
  contributor setup / testing / release procedure

docs/scene-renderer-contract.md
  source / scene / fixed-channel / object semantics

docs/music-presentation-contract.md
  protected stereo and music-presentation obligations

docs/binaural-renderer.md
  portable binaural DSP invariants

docs/realtime-control-contract.md
  sample-time / realtime / latency / failure semantics

docs/omniphony-for-windows.md
  Windows ingress / egress / lifecycle / installation

docs/headphone-calibration.md
  listener / HRTF / headphone calibration boundary

docs/osc-control-contract.md
  OSC/control semantics

docs/game-music-realtime-source-contract.md
  recovered-source / game-music realtime semantics

omniphony-renderer/
  executable implementation

.github/workflows/
  executable validation / packaging / release behavior

.agents/
  agent procedures and connector re-entry routes, never product truth

Git history
  chronology and retired alternatives
```

Generated inventories, search results, connector maps, status summaries, and task capsules are projections. They may help orientation, but they are not second truth stores.

## 3. Semantic collapse and repository cleanup

A representation change wins only if protected capability remains reachable while maintenance, context, routing, or verification cost falls.

Before keeping or adding a document, registry, cache, abstraction, directory, workflow, or subsystem, ask:

1. What current obligation does it uniquely own?
2. Can the fact be derived from code/tests/CI or another owner?
3. Is the object still required evidence, or merely repository history?
4. Which weaker or duplicate surface can be removed if this survives?
5. What breaks if this object disappears while Git history remains?

Prefer:

```text
existing owner over parallel owner
derive over duplicate
fold over archive
semantic name over lifecycle name
one executable invariant over prose-only duplication
```

Do not create `new`, `v2`, `final`, `replacement`, `old`, `archive`, `legacy`, `misc`, or `backup` as active canonical namespaces merely to avoid understanding current ownership.

Cleanup is:

```text
identify current obligations + required evidence
→ choose one owner for each overlap
→ fold surviving consequences into owners/tests
→ derive recoverable views
→ delete completed/duplicate/lifecycle-shaped surfaces
→ verify inbound routes and protected behavior
→ let Git retain the walk
```

## 4. Naming and hierarchy

Conventional root files keep their conventional names. New human-facing Markdown, scripts, folders, task keys, handoff keys, route labels, and other repository slugs use lowercase kebab-case when platform/tool/schema contracts permit it.

Language-native code identifiers keep their language convention. Existing external ABI/schema IDs and compatibility filenames are preserved until a deliberate migration proves every consumer.

Folders own stable semantic categories, not work sessions or arbitrary relationships. A standalone repository is already a project boundary; do not recreate Omniphony under another `projects/omniphony` tree.

## 5. Product boundaries

Omniphony is platform-agnostic. Windows is the current reference/hardening host, not portable architecture.

Portable core owns:

```text
source authority
canonical scene
channel/object geometry
sample-time semantics
stable source identity
presentation state
spatial compilation
binaural rendering
```

Platform hosts own:

```text
device/session discovery
platform audio APIs
source ingress
endpoint/egress
format/cadence adaptation
lifecycle/recovery
platform UI/service integration
installation/update/uninstall
```

A future macOS, Linux, Android, or other port is an adapter/lifecycle implementation around the same core. Do not import WASAPI, WDK, APO, COM, registry, Core Audio, Linux audio-server, or Android session concepts into portable scene law.

## 6. Source authority and one-render law

Preserve the strongest source truth available:

```text
stereo
→ protected master + bounded DERIVED support

multichannel / height bed
→ preserve AUTHORED channels and positions

static spatial roles
→ preserve AUTHORED roles

dynamic objects
→ preserve identity + PCM + continuous AUTHORED geometry

Ambisonics / HOA
→ preserve the supplied field

already-binaural
→ do not blindly virtualize again
```

`AUTHORED`, `DERIVED`, and `EMPTY` are provenance states.

Every source is spatially rendered by Omniphony at most once. A native spatial source must reach Omniphony before another headphone renderer collapses it to binaural stereo, or Omniphony must treat that already-rendered result according to the already-binaural policy.

There is no global spatial mode. Stream-local source semantics may differ concurrently.

## 7. Fidelity and audible-change law

Dimension may not be purchased by damaging the source.

Protected Current invariants include:

- direct finished-master identity;
- bass/body and groove floor;
- transient ownership;
- center solidity;
- clarity and tonal identity;
- dynamics and headroom;
- authored stereo motion;
- accepted front/back, height, early-field, and late-field behavior.

Prefer geometry, HRTF/ITD, distance, directional early structure, source extent, and physically motivated room cues over indiscriminate reverb, treble energy, or diffuse duplication.

Every substantive sound-changing intervention begins with relevant primary/peer-reviewed work plus mature implementation precedent where available, then:

```text
bounded candidate
→ objective validation
→ clean-route physical listening
→ keep / revise / revert
→ promote surviving invariant
→ delete experiment narrative
```

Physical listening is promotion authority for perceptual claims. Measurements and papers guide candidates but do not overwrite the listening result.

## 8. Realtime law

Realtime work remains bounded and deterministic for equivalent continuous input/state.

Do not perform from realtime callbacks:

- filesystem/network I/O;
- device/session enumeration;
- UI work;
- SOFA parsing or large HRTF construction;
- unbounded inference;
- large/unbounded allocation or deallocation;
- thread creation;
- blocking logging or waits.

Callback size is transport, not source or acoustic semantics. Preserve explicit discontinuity/reset behavior, bounded queues, and worker-owned allocating work where needed.

## 9. GitHub connector workspace law

When GitHub is the transport, read [`.agents/skills/github-workspace/SKILL.md`](.agents/skills/github-workspace/SKILL.md). The connector is a repository workspace, not a bag of unrelated file calls.

Use:

```text
observe
→ orient
→ act
→ verify
→ refresh-awareness
→ continue
```

### Observe and orient

Freeze one exact base head and tree. Batch independent exact reads against that ref. Prefer exact tree membership and blob/file reads before ranked search. Search locates; exact reads establish state.

Track ephemeral task sets:

```text
accepted-head
read-set
write-set
dependency-set
protected-set
staged-overlay
validation-target/state
capability-blocks
```

Do not persist these as a second workspace ledger.

### Act

One independent text file may use a contents compare-and-swap update when its current blob SHA is fresh.

Coupled changes should be staged as Git objects:

```text
fresh head/tree
→ create all blobs
→ create one candidate tree
→ create one commit with parent = accepted head
→ refresh main
→ fast-forward only if compatible
```

Serialize dependent writes to the same path.

### Refresh awareness

A moved `main` is always an awareness event before it is a conflict decision.

Compare the last accepted head directly with the newest observed head. Distinguish:

```text
remote-context-available
refresh-context
write-overlap-review
protected-owner-changed
history-diverged
```

Path-disjoint remote work may still be useful positive interference. Inspect compact intervening-commit summaries and absorb newly landed owners/tests/implementation when they materially improve the active task.

If a changed remote path shaped the reasoning, re-read that premise. If it touches an intended write, inspect exact diff/hunks. If it touches a governing/protected owner, re-enter its contract.

A Git parent may need rebuilding without invalidating the staged semantic edit. Preserve exact staged blobs where possible. Do not restart a whole task merely because `main` advanced.

Never force-push. Bound immediate publication retries on a hot `main`; preserve the staged overlay instead of entering an infinite refresh loop.

### Verify

Validation belongs to an exact target SHA. Never transfer a pass from one SHA to another.

Distinguish repository control from fresh runtime execution. A connector may read/write Git and inspect/rerun Actions without having a shell. Finish all repository-native work first and hand off only the genuinely unavailable execution step.

Treat partial results precisely. No search match, truncation, pagination, permission failure, missing job, backend startup failure, workflow runtime failure, and executed test failure are different states.

After publication, re-fetch `main`, verify the intended commit/path content, inspect CI that actually executed, and report unexecuted validation separately.

### Coordination trailers

For substantial direct-main agent commits, add retrospective routing trailers after the actual diff/validation state is known:

```text
omniphony-task: <lowercase-kebab-case-key>
omniphony-change-kind: <actual-landed-kind>
omniphony-validation: <actual-validation-state>
omniphony-handoff: <optional issue numbers>
```

These are coordination hints, not evidence. Git diff and validation state outrank them.

## 10. Codex capability-debt handoff

When a concrete actionable step is blocked specifically by the current chatspace/GitHub-connector/runtime surface, use [`.agents/skills/codex-handoff/SKILL.md`](.agents/skills/codex-handoff/SKILL.md).

The authoritative queue is open GitHub issues with:

```text
title prefix: CODEX:
body marker: <!-- omniphony-codex-handoff:v1 -->
```

Use a handoff for missing local execution, OS/hardware probes, dependency installation, inaccessible CI diagnostics, binary inspection, or other concrete capabilities a later local/Codex environment can exercise.

Do not create handoffs for ordinary research uncertainty, user decisions, vague ideas, work the connector can still perform, or publication contention alone.

Search for an existing matching open issue before creating another. GitHub issue state is the queue; do not create a parallel JSON/Markdown queue.

## 11. Evidence and validation

Keep evidence states separate:

```text
source exists
≠ code builds
≠ unit/regression tests pass
≠ host API negotiates
≠ endpoint association succeeds
≠ intended representation reaches the renderer
≠ samples are transformed exactly once
≠ physical endpoint receives output
≠ physical listening confirms the percept
```

Validation must match the intervention:

```text
documentation/ownership
→ route/link/semantic continuity

portable renderer
→ focused tests + affected renderer suite

realtime/ABI
→ boundedness + lifecycle + non-finite + discontinuity + ABI tests

Windows host/installer
→ APO + COM/lifecycle + endpoint + manifest + rollback + packaging/CI

audible DSP
→ engineering validation + controlled physical listening
```

CI failure is evidence. Never weaken a valid gate merely to make it green.

## 12. Completion

Before publication:

1. refresh current `main`;
2. inspect intended writes plus changed supporting/protected premises;
3. preserve unrelated work;
4. confirm canonical ownership;
5. inspect the candidate diff;
6. run or route proportionate validation;
7. remove superseded surfaces created redundant by the change.

After publication:

1. fetch the resulting commit/ref;
2. verify exact changed paths/content;
3. confirm the commit remains in current `main` history;
4. inspect target-SHA CI/status that actually executed;
5. leave unresolved work only in `ROADMAP.md` or a concrete Codex issue;
6. do not create a history document to memorialize completed work.

The repository should be easier to resume after every change.
