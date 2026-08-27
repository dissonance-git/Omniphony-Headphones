# Omniphony development contract

This file is the canonical operating law for `dissonance-git/Omniphony-Headphones`.

> **More capability, fewer conceptual machines.**
>
> **One concept, one writable owner. Store authority. Derive views. Preserve evidence. Promote slowly.**
>
> **Living owners keep current obligations. Git keeps the walk.**

Direct user correction or instruction outranks repository prose.

## 1. First-connection skill preflight

Every substantive reasoning-agent entry inherits [`.agents/skills/skill-preflight/SKILL.md`](.agents/skills/skill-preflight/SKILL.md).

Before repository, DSP, host, installer, research, review, or publication action:

```text
identify Omniphony
→ read current AGENTS.md authority
→ read the smallest relevant README.md identity/routing surface
→ enumerate .agents/skills/*/SKILL.md
→ compare skill names/descriptions with the exact obligation
→ select process/control skills first
→ read selected current skill bodies
→ build the smallest sufficient context
→ act
→ verify
```

Do not front-load every skill body. Current repository bytes outrank remembered procedure.

For GitHub-native work, the preferred first-connection packet is derived from current state rather than stored as a mutable connector map. When local execution exists, `python .agents/github-agent-bootstrap.py --json` emits the exact HEAD/tree, authority path, skill-preflight owner, and live skill fingerprint. Connector-only agents should emulate that packet from exact GitHub evidence.

Refresh skill awareness when `AGENTS.md`, `README.md`, `.agents/skills/`, or the exact obligation changes materially.

## 2. Enter through current authority

For substantive work:

```text
current main HEAD
→ README.md
→ AGENTS.md at the same HEAD
→ selected current skills
→ ROADMAP.md only when unresolved priority matters
→ smallest task-relevant living contract
→ exact code / tests / CI
→ target
```

Resolve current `main` before reasoning about mutable repository state. Search is for discovery; exact refs, commits, trees, blobs, files, code, tests, and CI establish the selected snapshot.

Repository publication is main-only unless the user explicitly changes that instruction. Never force-push. Preserve unrelated concurrent work.

## 3. One owner per concept

Canonical roles are:

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
  agent procedure / derived first-connection tooling only

Git history
  chronology and retired alternatives
```

Generated inventories, search results, connector packets, status summaries, caches, and task capsules are projections. They are not peer truth stores.

## 4. Semantic collapse and cleanup

A representation change wins only if protected capability remains reachable while maintenance, context, routing, latency, or verification cost falls.

Repository cleanup is:

```text
identify current obligations + required evidence
→ choose one owner for every overlap
→ fold surviving consequences into owners/tests
→ derive recoverable views
→ remove duplicate/completed/lifecycle-shaped surfaces
→ verify inbound routes and protected behavior
→ let Git retain the walk
```

Before keeping or adding a document, registry, cache, abstraction, wrapper, directory, workflow, compatibility route, or subsystem, ask:

1. What current obligation does it uniquely own?
2. Can the fact or relation be derived from another owner?
3. Is the exact object still required evidence, provenance, or compatibility?
4. Which weaker surface can disappear if this survives?
5. What required consequence becomes unrecoverable if it is removed?

Prefer existing owner over parallel owner, derive over duplicate, fold over archive, semantic names over lifecycle names, and executable invariants over prose duplication.

Do not create active `new`, `v2`, `final`, `replacement`, `old`, `archive`, `legacy`, `misc`, or `backup` namespaces merely to avoid understanding current ownership.

## 5. Naming and hierarchy

Conventional root files keep conventional names.

New human-facing Markdown, scripts, folders, task/handoff keys, route labels, and repository slugs use lowercase kebab-case when platform/tool/schema contracts permit it. Python scripts and identifiers use lowercase snake_case. Language-native code identifiers keep their language convention.

Existing ABI/schema IDs, platform registration names, compatibility filenames, and external contracts stay exact until a deliberate migration proves every consumer.

Folders own stable semantic categories, not chronology or work sessions.

## 6. Product and source boundaries

Omniphony is platform-agnostic. Windows is the current reference/hardening host, not portable architecture.

Portable core owns source authority, canonical scene, geometry, source/sample time, stable source identity, presentation state, spatial compilation, and binaural rendering.

Platform hosts own device/session discovery, native audio APIs, ingress/egress, cadence/format adaptation, lifecycle/recovery, platform UI/service integration, and installation/update/uninstall.

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
→ preserve supplied field

already-binaural
→ do not blindly spatialize again
```

`AUTHORED`, `DERIVED`, and `EMPTY` are provenance states. There is no global spatial mode. Stream-local source semantics may differ concurrently.

Every source is spatially rendered by Omniphony at most once.

## 7. Fidelity, audible change, and realtime law

Dimension may not be purchased by damaging the source.

Protected Current invariants include direct finished-master identity, bass/body and groove, transient ownership, center solidity, clarity and tonal identity, dynamics/headroom, authored stereo motion, and the accepted front/back, height, early-field, and late-field behavior.

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

Realtime work remains bounded and deterministic for equivalent continuous input/state. Do not perform filesystem/network I/O, device enumeration, UI work, SOFA parsing, large HRTF construction, unbounded inference, large/unbounded allocation, thread creation, blocking logging, or waits from the audio callback.

Callback size is transport, not source or acoustic semantics.

## 8. Repository mutation and concurrency

Repository changes use [`.agents/skills/repo-change/SKILL.md`](.agents/skills/repo-change/SKILL.md).

GitHub-backed work additionally uses [`.agents/skills/github-workspace/SKILL.md`](.agents/skills/github-workspace/SKILL.md). Long or hot-`main` connector work also uses [`.agents/skills/github-workspace-liveness/SKILL.md`](.agents/skills/github-workspace-liveness/SKILL.md).

The operating loop is:

```text
observe exact state
→ orient narrowly
→ stage an overlay
→ verify what can actually execute
→ refresh awareness of current main
→ continue from the newest accepted head
```

Remote movement is awareness before conflict. Path-disjoint concurrent work may still introduce useful evidence, tests, helpers, or owners. Absorb relevant positive interference without discarding unaffected progress.

Validation belongs to an exact target SHA. A pass from one commit does not transfer to another.

For substantial direct-main agent commits, use retrospective routing trailers:

```text
omniphony-task: <lowercase-kebab-case-key>
omniphony-change-kind: <actual-landed-kind>
omniphony-validation: <actual-validation-state>
omniphony-handoff: <optional issue numbers>
```

These are coordination hints, not evidence.

## 9. Evidence, validation, and capability debt

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

Repository control and fresh runtime execution are separate capabilities. If a concrete actionable remainder is blocked specifically by the current interface/runtime, use [`.agents/skills/codex-handoff/SKILL.md`](.agents/skills/codex-handoff/SKILL.md). The authoritative capability-debt queue is open GitHub issues with title prefix `CODEX:` and body marker `<!-- omniphony-codex-handoff:v1 -->`.

Do not create a parallel JSON/Markdown queue.

## 10. Completion

Before publication, refresh `main`, inspect intended writes plus changed supporting/protected premises, preserve unrelated work, inspect the exact candidate diff, run or route proportionate validation, and remove superseded surfaces made redundant by the change.

After publication:

1. re-fetch `main`;
2. verify the intended commit and exact changed paths/content;
3. confirm the commit remains in current `main` history;
4. inspect target-SHA validation that actually executed;
5. leave unresolved work only in `ROADMAP.md` or a concrete Codex issue;
6. do not create a history document for completed cleanup.

The repository should be cheaper to understand and safer to resume after every change.
