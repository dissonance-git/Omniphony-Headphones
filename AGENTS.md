# Omniphony development contract

This file is the canonical operating law for `dissonance-git/Omniphony-Headphones`.

> **Single-owner, evidence-stratified, derivation-first design: store authority, derive views, preserve evidence, and promote slowly.**

The important repository interpretation is:

> **Git owns history. Living documents own only current obligations.**

## 1. Repository entrance

Use this bounded sequence:

```text
current main HEAD
→ README.md
→ AGENTS.md at the same HEAD
→ ROADMAP.md when unresolved current work matters
→ exact task-relevant living contract
→ exact code / tests / CI
→ target file
```

1. Resolve current `main` before substantive work.
2. Read `README.md` for durable product identity and architecture.
3. Read this file for governing implementation, evidence, realtime, and listening law.
4. Read `ROADMAP.md` only when unresolved current work, gates, or frontier matter.
5. Read only the smallest task-relevant living contract.
6. Inspect code/tests/CI for executable truth rather than relying on prose inventories.
7. Before replacement writes, re-fetch the target from current `main`.
8. After publication, inspect the exact changed paths and resulting content.

Repository workflow is main-only unless the user explicitly changes that instruction. Never force-push. Keep sound-changing commits small enough that a rejected mechanism can be reverted cleanly.

## 2. Single-owner and derivation law

Every durable fact, mutable state, obligation, and current decision gets one writable canonical owner.

```text
README.md
  durable product identity / public architecture

AGENTS.md
  governing repository / evidence / listening / realtime law

ROADMAP.md
  unresolved current work / gates / frontier

docs/music-presentation-contract.md
  stereo and music-presentation obligations

docs/scene-renderer-contract.md
  source / scene / channel-object / renderer semantics

docs/realtime-control-contract.md
  sample-time / realtime / latency / failure semantics

docs/omniphony-for-windows.md
  Windows host / ingress / egress / lifecycle / installation

docs/headphone-calibration.md
  listener and headphone calibration

docs/osc-control-contract.md
  OSC/control protocol semantics

docs/game-music-realtime-source-contract.md
  recovered-source / game-music realtime semantics

omniphony-renderer/
  executable implementation

tests + .github/
  executable validation and CI behavior

Git history
  chronology and retired alternatives
```

Before adding or maintaining a document, registry, status table, report, cache, abstraction, or subsystem, answer:

1. What exact current obligation does it uniquely own?
2. Does a canonical owner already exist?
3. Can the information be derived from code/tests/CI instead?
4. Is this durable law, unresolved current work, executable behavior, or merely chronology?
5. If a mechanism is being promoted, what evidence earned a live invariant?
6. What duplicate writable path can now be removed?

If the answer is “this preserves an old experiment, old tuning, old implementation state, old listening comparison, old migration, or old research trail,” the working tree is the wrong owner. Git already preserves it.

## 3. Evidence without museums

“Preserve evidence” means preserve the **current consequence** of evidence, not a parallel historical corpus.

Use this promotion shape:

```text
observation / experiment / research
→ decision
→ fold stable consequence into living contract or executable regression
→ keep unresolved consequence in ROADMAP.md
→ delete superseded narrative
→ Git retains chronology
```

Examples:

- an audible mechanism that is retained becomes a current music/renderer invariant and, where possible, a regression test;
- a rejected mechanism may create a durable prohibition or validation requirement, but its diary does not stay beside the product;
- a fixed DSP defect belongs in code/tests, not a permanent validation report;
- an unfinished capability belongs in the roadmap, not a status section inside architecture docs;
- citations may remain in a living contract when they directly support a current obligation, but a research ledger does not exist merely to remember that papers were read.

Negative and ambiguous evidence matter. Convert their decision-relevant content into a live boundary, test, or unresolved roadmap item. Do not keep them as standalone chronicles.

## 4. Documentation law

A working-tree document must be one of:

```text
durable product/governance law
current unresolved frontier
focused current technical contract
public contribution/license/attribution material
```

Do not retain:

- listening histories;
- research ledgers;
- experiment reports;
- dated frontier snapshots;
- migration ledgers;
- “current implementation” inventories that code can answer;
- completed phase plans;
- retired profile matrices;
- machine-specific debugging transcripts;
- obsolete transport descriptions;
- frozen numerical tuning narratives whose authority now lives in code/config/tests.

A focused contract describes what must remain true, not how the project arrived there.

When two documents overlap, merge toward the narrower canonical owner and delete the weaker surface rather than adding cross-references between duplicates.

## 5. Project boundaries

Helix owns cross-project continuity. deepSTRF owns reusable auditory/machine-hearing research when that work genuinely lives there. Omniphony owns its renderer, host code, source semantics, tests, builds, releases, and product contracts.

Import only the smallest validated distinction needed by Omniphony. Do not copy parent-project research machinery or provenance logs into this repository.

## 6. Research gate for audible changes

Every substantive sound-changing intervention begins with both:

1. relevant peer-reviewed, standards, or primary technical work; and
2. mature implementation precedent where available.

Preferred loop:

```text
listening observation
→ literature / implementation pass
→ smallest relevant mechanism
→ bounded Omniphony experiment
→ objective validation / CI
→ physical listening
→ keep, revise, or revert
→ fold the surviving rule into a living owner
```

Pure mechanical build, packaging, CI, formatting, or compile repairs do not require new audio research when they cannot alter runtime sound.

Research is an input to a decision, not a permanent documentation category.

## 7. Core independence

The renderer, inference, source scene, and DSP core remain portable and independent of Windows.

Portable core owns:

```text
source scene
channel/object geometry
source authority
presentation state
spatial rendering
binaural output
```

Platform hosts own:

```text
device/session discovery
platform audio APIs
endpoint association
format translation
clock/recovery behavior
platform UI/service integration
installation/update/uninstall
```

Do not move endpoint identities, registry rules, WASAPI/WDK state, tray/service concepts, or installer lifecycle into portable renderer semantics to solve host problems.

## 8. Source authority

The finished master is authoritative for stereo music. Keep the protected direct master explicitly present and use inferred spatial content only as bounded support.

More source truth means less inference:

```text
stereo → protected master + bounded DERIVED support
multichannel → preserve authored channels and positions
object audio → preserve supplied identity and geometry
Ambisonics / HOA → preserve the supplied field
already-binaural → avoid destructive double virtualization
```

Keep source truth, signal evidence, presentation hypothesis, and placement choice distinct.

`AUTHORED`, `DERIVED`, and `EMPTY` are provenance states, not cosmetic labels.

## 9. Fidelity and listening law

- Dimension may not be purchased by damaging the music.
- OFF may collapse the world; it may not bring the rhythm section back to life.
- Energy may be anchored; authored motion may not be frozen.
- Bass pressure, kick weight, transient ownership, center stability, dynamics, tonal identity, and stereo motion are protected invariants.
- Do not recover spatial scale with indiscriminate late reverb, treble energy, or diffuse duplication.
- Prefer geometry, HRTF/ITD, distance, directional early-field structure, source extent, and physically motivated room cues.

Physical listening is the promotion authority for perceptual claims. Measurements, papers, simulations, and model outputs guide candidates but do not redefine success after the fact.

When listening changes the durable rule, update `docs/music-presentation-contract.md`, `docs/scene-renderer-contract.md`, a narrower live contract, and/or an executable regression. Do not create a listening archive.

## 10. Realtime law

Realtime paths remain bounded and deterministic for equivalent continuous input/state.

Do not perform these operations from realtime callbacks:

- filesystem or network I/O;
- device/session enumeration;
- UI work;
- SOFA parsing/import;
- unbounded model inference;
- large or unbounded allocation/deallocation;
- thread creation;
- blocking logging;
- unbounded waits;
- research-time discovery.

Prefer preallocation, bounded queues, explicit discontinuity/reset behavior, and worker-owned allocating DSP where needed. Playback continuity may never depend on an optional analyzer, model, cache, or UI process.

`docs/realtime-control-contract.md` owns the detailed sample-time, publication, recovery, and latency semantics.

## 11. Evidence and claims

Keep these states separate:

```text
source exists
≠ code builds
≠ unit/regression tests pass
≠ host API negotiation succeeds
≠ endpoint association succeeds
≠ application supplies the expected representation
≠ physical endpoint receives the intended render
≠ listening confirms the intended percept
```

Do not promote a capability or perceptual claim beyond the strongest evidence obtained.

For sound-changing work, record the needed evidence in the change/PR/commit process while evaluating it. After the decision, keep only the live consequence in canonical owners.

## 12. Validation

Validation must match the intervention.

```text
documentation / ownership change
→ route/link checks + semantic continuity

portable renderer change
→ focused unit/regression tests + affected renderer suite

realtime / ABI change
→ lifecycle, boundedness, non-finite, discontinuity, and ABI tests

Windows host / installer change
→ applicable APO, COM/lifecycle, endpoint, manifest, packaging, rollback, and CI checks

audible DSP change
→ engineering validation + controlled physical listening
```

CI failure is evidence. Do not make a gate green by weakening a valid requirement.

A compile pass is not a listening pass. A synthetic smoke is not physical endpoint proof. A repository search is not completeness proof.

Do not retain completed validation reports when the surviving result can be encoded in tests, code, or a living contract.

## 13. Completion law

Before publication:

1. re-fetch current `main` and target blobs;
2. preserve unrelated concurrent work;
3. confirm the canonical owner of every changed state;
4. verify no duplicate writable truth was introduced;
5. run proportionate validation;
6. inspect the exact diff;
7. remove any superseded document or mechanism made redundant by the change.

After publication:

1. fetch the resulting commit;
2. verify intended changed paths and content;
3. confirm the commit remains in current `main` history;
4. report build/tests, CI, measurements, and listening as separate evidence states;
5. leave unresolved work in `ROADMAP.md`;
6. do not create a history document to memorialize the work.
