# Omniphony

Omniphony is an open-source spatial audio renderer for headphones.

Its goal is to occupy the same broad class of system-audio role as proprietary headphone spatial renderers while keeping the renderer, scene model, source-authority rules, DSP, validation, and host integration inspectable.

> **One open spatial renderer that enhances stereo, preserves authored spatial truth, accepts richer scenes when available, and performs the final headphone render itself.**

Windows is the first product host. The renderer, source-scene contract, and DSP core remain portable.

## Product law

Omniphony is one renderer whose behavior becomes more source-authoritative as richer input becomes available.

```text
stereo
→ preserve the finished master
→ infer only missing spatial structure

authored channel bed
→ preserve supplied channels and positions
→ infer less

static spatial scene
→ preserve supplied roles

dynamic objects
→ preserve identity, PCM, and continuous geometry

all trustworthy source forms
→ one Omniphony scene
→ one final binaural render
→ stereo headphones
```

> **The richer the source truth, the less Omniphony invents.**

## Repository design philosophy

> **Single-owner, evidence-stratified, derivation-first design.**
>
> **Store authority. Derive views. Preserve evidence. Promote slowly.**

In this repository, **preserve evidence does not mean preserve historical documents**.

Decision-relevant evidence must be absorbed into the living owner that uses it:

```text
stable product consequence
→ README.md or a focused living contract

current unresolved consequence
→ ROADMAP.md

executable invariant
→ code / tests / CI

current contributor/governance rule
→ AGENTS.md

retired experiment, superseded tuning, chronology
→ Git history only
```

A document exists only when it owns a current obligation that cannot be derived more exactly from code, tests, CI, or another canonical owner. Research notes, experiment diaries, status ledgers, implementation inventories, migration chronicles, listening histories, and frozen frontier snapshots do not remain in the working tree after their useful conclusions have been folded into living owners.

The canonical writable surfaces are:

| Surface | Responsibility |
| --- | --- |
| `README.md` | durable product identity and public architecture |
| `AGENTS.md` | governing repository, evidence, realtime, and listening law |
| `ROADMAP.md` | unresolved current work, gates, and frontier |
| `docs/music-presentation-contract.md` | stereo/music presentation obligations |
| `docs/scene-renderer-contract.md` | source, scene, channel/object, and renderer semantics |
| `docs/realtime-control-contract.md` | sample-time, bounded realtime, continuity, latency, and failure law |
| `docs/omniphony-for-windows.md` | durable Windows product, ingress, egress, lifecycle, and installation contract |
| `docs/headphone-calibration.md` | listener/HRTF/headphone calibration boundary |
| `docs/osc-control-contract.md` | OSC/control protocol semantics |
| `docs/game-music-realtime-source-contract.md` | realtime recovered-source/game-music semantics |
| `omniphony-renderer/` | executable implementation |
| tests and `.github/` | executable validation and CI behavior |
| Git history | chronology and retired alternatives |

When two surfaces describe the same mutable fact, keep the canonical owner and delete or derive the other surface.

## Source authority

> **Preserve the richest source representation available and invent only what is missing.**

Keep source truth, signal evidence, presentation hypothesis, and placement choice distinct.

Static scene lanes use explicit authority states:

```text
AUTHORED  source or host supplied this signal / position
DERIVED   Omniphony inferred bounded support
EMPTY     no trustworthy signal is assigned
```

For stereo, the finished master remains the musical authority and inferred spatial support stays bounded. For authored multichannel, height, and object audio, supplied geometry outranks inference. Already-binaural material must not be blindly virtualized a second time.

## Canonical source scene

The fixed spatial vocabulary is a 17-position **8.1.4.4** semantic frame:

```text
horizontal
FL FR C LFE SL SR BL BR BC

upper
TFL TFR TBL TBR

lower
BFL BFR BBL BBR
```

This is a coordinate vocabulary, not a claim that every source contains seventeen authored channels. Dynamic XYZ objects remain continuous objects beside the static frame rather than being snapped into fixed anchors.

The semantic source scene and internal rendering geometry are different concepts. Internal support directions are renderer geometry, not authored input channels.

Portable scene semantics sit below the renderer in a host-neutral contract layer. That layer owns stable source/object identity continuity, metric XYZ and radial distance, exact lowering from rational source time to half-open sample spans, and bounded stable source slots. Platform adapters lower their native metadata into that contract; the renderer consumes the same contract. Callback size, Windows ABI shape, and renderer internals therefore cannot silently become scene semantics.

See [`docs/scene-renderer-contract.md`](docs/scene-renderer-contract.md).

## Windows product boundary

The intended product experience is simple:

```text
Windows audio
     ↓
Omniphony
     ↓
headphones
```

The Windows host owns capture/playback integration, source ingress, endpoint lifecycle, recovery, installation, update, uninstall, and optional preferences. The portable core owns source-scene semantics and rendering.

Normal rendering is headless. It does not require a virtual cable, loopback host, or foreground audio application to remain open.

See [`docs/omniphony-for-windows.md`](docs/omniphony-for-windows.md). Current unresolved Windows work belongs only in [`ROADMAP.md`](ROADMAP.md).

## Stereo fidelity law

Stereo is a first-class source type rather than a compatibility afterthought.

The finished master stays explicitly present. Omniphony may infer bounded width, depth, height, ambience, source extent, and externalization support, but spatial dimension may not be purchased by damaging clarity, impact, center stability, timbre, dynamics, rhythmic precision, authored motion, or bass integrity.

> **OFF may collapse the world. It may not bring the rhythm section back to life.**

Physical listening decides perceptual promotion. Once a result changes the durable product rule, that rule is folded into the living music contract or an executable regression. The discarded comparison narrative remains only in Git history.

See [`docs/music-presentation-contract.md`](docs/music-presentation-contract.md).

## Protected perceptual baseline

The accepted default Windows music presentation is a protected baseline, not a temporary tuning waypoint.

Future work may expand authored-scene semantics, radial distance, personalization, platform integration, diagnostics, or optional capabilities, but a sound-changing mechanism does not enter the default music path merely because it is more sophisticated or passes engineering tests. It must preserve the accepted baseline's bass/body, transient ownership, center solidity, clarity, dynamics, authored motion, externalization, and route reliability under clean physical listening.

New audible mechanisms therefore begin as bounded candidates outside the protected default. If a candidate wins controlled listening, its durable consequence replaces the weaker rule. If it does not, the accepted baseline remains the product.

The accepted Current sound is also guarded executably: sound-owning modules/configuration and selected output-safety constants are pinned by CI. Ordinary host, scene, and architecture work must not require that guard to move. An intentional listening promotion changes the sound and its baseline guard together.

The accepted Current early-field geometry includes true-direction second-order front/top-front image paths routed through four dedicated measured-HRTF precision buses. That mechanism conserves the existing front early tap-power budget and keeps the sub-300 Hz early return coherent, so the stronger frontal boundary is geometry rather than extra wet energy. It is now part of the protected reference sound.

Listener-specific or custom HRTF selection remains a future optional capability. It is not a prerequisite for the Current reference path and should not be mixed into sound-preserving cleanup of the accepted baseline.

Optional features such as head tracking are enhancements for listeners who want them. They are not required for normal Omniphony playback, calibration, or the reference listening path.

## Realtime law

Realtime paths remain bounded and deterministic for equivalent continuous input and state. Audible changes live on the sample timeline, not on callback boundaries.

Realtime callbacks must not perform filesystem or network I/O, device/session enumeration, unbounded allocation, UI work, blocking logging, research-time analysis, or other unbounded operations. Playback must remain functional when optional analysis, models, caches, or control surfaces are unavailable.

See [`docs/realtime-control-contract.md`](docs/realtime-control-contract.md).

## Evidence states

Do not collapse these into one status:

```text
source exists
≠ code builds
≠ tests pass
≠ host API negotiation succeeds
≠ endpoint association succeeds
≠ a real application supplies the expected source representation
≠ physical playback reaches the intended endpoint
≠ physical listening confirms the intended percept
```

Claims stop at the strongest evidence actually obtained. Unresolved evidence belongs in `ROADMAP.md`; resolved engineering evidence belongs in tests/code/CI or the current contract consequence it justifies.

## Build and tests

From `omniphony-renderer/`:

```sh
cargo test -p scene_contract
cargo test -p renderer
cargo test -p orender_engine --lib --tests
cargo test -p source_ffi --lib --tests
cargo test -p realtime_ffi
```

Windows-host changes should also pass the task-relevant APO, COM/lifecycle, realtime ABI, installer, endpoint, and packaging checks in CI.

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for contribution workflow and [`AGENTS.md`](AGENTS.md) for the governing repository contract.

## Definition of success

> **A finished source keeps its identity, weight, dynamics, clarity, and authored spatial truth while gaining a stable external world with convincing width, depth, height, distance, motion, source extent, and envelopment.**

For stereo, Omniphony should create a richer spatial presentation without pretending inferred geometry was authored. For richer source formats, it should become progressively less inferential because the source has already supplied more of the world.
