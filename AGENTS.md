# Omniphony development contract

This file is the canonical operating law for `dissonance-git/Omniphony-Headphones`.

> **Single-owner, evidence-stratified, derivation-first design: store authority, derive views, preserve evidence, and promote slowly.**

## 1. Repository entrance

When entering through GitHub or another remote agent surface, use this bounded sequence:

```text
current main HEAD
→ README.md
→ AGENTS.md at the same HEAD
→ ROADMAP.md when unresolved current work is relevant
→ recent commits
→ exact task-relevant contract / code / tests / evidence
→ exact target file
```

1. Resolve the current `main` commit before substantive work.
2. Read `README.md` for durable product identity, stable public architecture, and entry routes.
3. Read this file from the same commit for governing implementation, evidence, realtime, and listening law.
4. Read `ROADMAP.md` only when current unresolved work, gates, or frontier matter.
5. Inspect recent commits to see what is actively moving. Activity does not overrule canonical owners or accepted evidence.
6. Hydrate only the task-relevant region. Do not pull in all of deepSTRF, retired libaural provenance, VGM Tooling, or Helix unless the work genuinely crosses those boundaries.
7. Before a replacement write, re-fetch current `main` and the exact target. Never overwrite newer remote state from a cached copy.
8. After publication, fetch the resulting commit, inspect changed paths, and confirm it remains in current `main` history.

Repository workflow is **main-only unless the user explicitly changes that instruction**. Never force-push. Keep sound-changing commits small enough that a rejected mechanism can be reverted cleanly.

## 2. Canonical ownership and derivation law

Omniphony uses one writable canonical owner per durable fact, mutable state, obligation, and evidence class.

```text
README.md
  durable product identity, stable public architecture, entry routes

AGENTS.md
  governing repository / development / evidence / listening law

ROADMAP.md
  unresolved current work, acceptance gates, frontier

docs/listening-history.md
  physical listening evidence, retained/rejected audible mechanisms, historical controls

docs/omniphony-for-windows.md
  durable Windows product/host boundary

docs/*-contract.md and focused docs
  bounded technical contracts or preserved research evidence

omniphony-renderer/
  executable implementation

tests + .github/
  executable validation and CI behavior

Git history
  chronology and repository versioning
```

Before adding or maintaining a document, registry, status table, inventory, cache, abstraction, or subsystem, answer:

1. What exact obligation does this object uniquely own?
2. Does a canonical owner already exist?
3. Is this durable law, current state, source evidence, measurement, listening evidence, inference, history, or a derived view?
4. Can the inventory, implementation summary, dependency map, status surface, or test list be derived from canonical state instead of stored again?
5. If a local/generated/experimental mechanism is being promoted, what independent evidence and repeated use earned promotion?
6. What duplicate writable path or stale narrative can now be retired?

Operational consequences:

- do not maintain a hand-written implementation matrix when code/tests/CI can answer the question more exactly;
- do not put changing project frontier or candidate details into the durable README or this governing file;
- do not copy a specialized contract into several roots; link to its canonical owner;
- do not copy parent-project machinery merely to make inheritance visible;
- local READMEs/docs should explain ownership, boundaries, and non-obvious semantics rather than mirror directory contents;
- generated artifacts are disposable unless deliberately promoted as retained evidence with provenance;
- compression may remove duplicate narration but must not erase source evidence, uncertainty, negative results, historical lineage, or distinctions required to reopen a decision.

When two surfaces say the same mutable thing, keep one owner and replace the other copy with a route.

## 3. Project instruction chain

Omniphony is the independent implementation home for `project:omniphony`, tracked by Helix and able to consume validated auditory research from deepSTRF. Retired libaural material is provenance only.

Before substantive work, apply the current instruction chain in order:

1. `dissonance-git/Helix/AGENTS.md` for applicable common operating law;
2. `dissonance-git/deepSTRF/AGENTS.md` from its active branch when the task actually crosses into parent auditory research;
3. this file for Omniphony-specific implementation and listening law.

A child inherits only parent laws relevant to its task. Direct user instruction or correction outranks the chain.

Helix owns cross-project continuity. deepSTRF owns general reusable auditory/machine-hearing research. Omniphony owns its renderer, host code, tests, builds, releases, and local implementation evidence.

## 4. Research gate for audible changes

Every substantive change that can alter what the listener hears begins with both:

1. a literature pass over relevant peer-reviewed, standards, or primary technical work; and
2. an implementation pass over mature open-source systems that solve the same or an adjacent problem.

Do not tune from intuition alone when established perceptual research or implementation precedent is available.

Preferred loop:

```text
listening observation
→ literature pass
→ mature implementation pass
→ smallest relevant mechanism
→ adapt to Omniphony topology
→ objective validation / CI
→ physical listening
→ keep, revise, or revert
```

Purely mechanical build, packaging, CI, formatting, or compile repairs do not require new audio research when they cannot alter runtime sound.

Research is an influence source, not permission to replace a working renderer with a parallel science project.

## 5. Core independence

The renderer, inference, source scene, and DSP core remain portable and independent of Windows.

Portable core owns concepts such as:

```text
source scene
channel/object geometry
source authority
presentation state
spatial rendering
binaural output
```

Platform hosts own concepts such as:

```text
device/session discovery
platform audio APIs
endpoint association
format translation
clock/recovery behavior
platform UI/service integration
installation/update/uninstall
```

Do not move Windows endpoint identities, registry rules, WASAPI/WDK state, tray/service concepts, or installer lifecycle into portable renderer semantics to solve host problems.

`docs/omniphony-for-windows.md` owns the durable Windows product/host boundary.

## 6. Source authority

The finished master is authoritative for stereo music. Keep the protected direct master explicitly present and use inferred spatial content only as bounded support.

More source truth means less inference:

```text
stereo → protected master + bounded inferred support
multichannel → preserve authored channels and positions
object audio → preserve supplied identity and geometry
Ambisonics / HOA → preserve the supplied field
already-binaural → avoid destructive double virtualization
```

Keep source truth, signal evidence, presentation hypothesis, and placement choice distinct.

`AUTHORED`, `DERIVED`, and `EMPTY` are provenance states, not cosmetic labels.

## 7. Fidelity and listening law

- Dimension may not be purchased by damaging the music.
- OFF may collapse the world; it may not bring the rhythm section back to life.
- Energy may be anchored; authored motion may not be frozen.
- Bass pressure, kick weight, transient ownership, center stability, dynamics, tonal identity, and stereo motion are protected invariants.
- Do not recover spatial scale with excessive late reverb, treble energy, or diffuse duplication.
- Prefer geometry, HRTF/ITD, distance, early-field structure, source extent, and physically motivated room cues.

Physical listening is the promotion authority for perceptual claims. Measurements, papers, simulations, and model outputs guide candidates but do not redefine success after the fact.

When a build is clearly better, preserve a rollback point before pushing farther. If a new mechanism damages a winning invariant, revert or narrow the mechanism rather than lowering the invariant.

`docs/listening-history.md` owns accepted/rejected perceptual evidence and historical controls. `ROADMAP.md` owns unresolved perceptual work. Do not duplicate the current candidate/frontier into root law.

## 8. Realtime law

Realtime paths remain bounded and deterministic for equivalent continuous input/state.

Do not perform these operations from realtime callbacks:

- filesystem or network I/O;
- device/session enumeration;
- UI work;
- SOFA parsing/import;
- model inference not explicitly designed for realtime use;
- large or unbounded allocations;
- blocking logging;
- unbounded mutex waits;
- research-time analysis or discovery.

Prefer preallocation, bounded queues, explicit discontinuity/reset behavior, and worker-owned allocating DSP where needed. Host callbacks exchange bounded audio/state with renderer workers rather than moving allocating graph construction onto OS realtime threads.

## 9. Evidence and claim law

Keep these states separate:

```text
source exists
≠ code builds
≠ unit/regression tests pass
≠ host API negotiation succeeds
≠ endpoint association succeeds
≠ application supplies the expected source representation
≠ physical endpoint receives the intended render
≠ listening confirms the intended percept
```

Do not promote a capability or perceptual claim beyond the strongest evidence obtained.

For sound-changing work preserve, where relevant:

```text
intended percept
source types affected
mechanism changed
objective behavior changed
fidelity risks
comparison baseline
physical listening result
keep / revise / revert decision
```

Negative and ambiguous results remain evidence. They are not invitations to multiply product modes.

## 10. Validation

Validation must match the intervention.

```text
documentation / ownership change
→ link/route checks + root semantic continuity

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

A compile pass is not a listening pass. A synthetic smoke is not physical endpoint proof. A connector or repository search is not completeness proof.

## 11. Documentation law

Public/root documents describe durable identity, stable architecture, contracts, and entry routes. They do not become development diaries.

Current unresolved work belongs in `ROADMAP.md`. Physical listening evidence belongs in `docs/listening-history.md`. Machine-specific debugging transcripts, one-off configurations, temporary hypotheses, and dated experiment narratives belong in focused evidence/research material, not in roots.

Do not manually mirror repository trees or implementation tables when they can be recovered exactly from the repository. A public capability summary is allowed when it describes a stable supported contract rather than internal implementation progress.

## 12. deepSTRF relationship

deepSTRF is the parent research project for reusable auditory/machine-hearing mechanisms. Omniphony imports only small validated distinctions that improve the consumer renderer without replacing its working spatial core.

```text
Helix research machinery
        ↓
deepSTRF auditory research
        ↓
small validated mechanisms
        ↓
Omniphony consumer renderer
```

A successful transfer remains Omniphony-local until repeated evidence earns a more general abstraction.

## 13. Completion law

Before publication:

1. re-fetch current `main` and target blobs;
2. preserve unrelated concurrent work;
3. confirm the canonical owner of every changed state;
4. verify no duplicate writable truth was introduced;
5. run proportionate validation;
6. inspect the exact diff.

After publication:

1. fetch the resulting commit;
2. verify intended changed paths and content;
3. confirm the commit remains in current `main` history;
4. report publication, build/tests, CI, measurements, and listening as separate evidence states;
5. leave the current frontier in `ROADMAP.md`, not reconstructed in completion prose elsewhere.
