# Omniphony

Omniphony is an open-source spatial audio renderer for headphones.

Its goal is to occupy the same broad class of system-audio role as proprietary headphone spatial renderers while keeping the renderer, scene model, source-authority rules, DSP, validation, and research inspectable.

> **One open spatial renderer that enhances stereo, preserves authored spatial truth, accepts richer scenes when available, and performs the final headphone render itself.**

Windows is the first product host. The renderer, source-scene contract, and DSP core are designed to remain portable.

## Product law

Omniphony is one renderer whose behavior becomes more source-authoritative as richer input becomes available.

```text
stereo
→ preserve the finished master
→ infer only missing spatial structure
→ render through Omniphony

5.1 / 7.1 / height PCM
→ preserve authored channels and positions
→ infer less because more scene geometry is known
→ render through the same renderer

static spatial scene
→ preserve supplied fixed roles
→ avoid reconstructing geometry already supplied
→ render through the same renderer

dynamic XYZ objects
→ preserve object identity and continuous motion
→ give supplied geometry maximum authority
→ render through the same renderer
```

The richer the source truth, the less Omniphony invents. Every path ends in one binaural render to an ordinary stereo headphone endpoint.

## Repository design philosophy

> **Single-owner, evidence-stratified, derivation-first design.**
>
> **Store authority. Derive views. Preserve evidence. Promote slowly.**

Omniphony keeps durable law, changing state, evidence, and generated views separate:

| Surface | Canonical responsibility |
| --- | --- |
| `README.md` | durable product identity, stable public architecture, and entry routes |
| `AGENTS.md` | governing development, evidence, realtime, and listening law |
| `ROADMAP.md` | unresolved current work, gates, and frontier |
| `docs/listening-history.md` | physical listening evidence, accepted/rejected audible mechanisms, and historical controls |
| `docs/*-contract.md` and other focused docs | bounded technical/product contracts and preserved research evidence |
| `omniphony-renderer/` | executable renderer and host implementation |
| tests and `.github/` | executable validation and CI behavior |
| Git history | chronology and repository versioning |

A mechanically recoverable inventory, implementation status table, dependency map, or test list should be derived from code or tooling when practical rather than maintained as a second writable truth. Current project status belongs in `ROADMAP.md`, not in durable architecture prose. Historical evidence stays preserved even after the current view changes.

## Source authority

> **Preserve the richest source representation available and invent only what is missing.**

Keep source truth, signal evidence, presentation hypotheses, and placement choices distinct.

Static scene lanes use explicit authority states:

```text
AUTHORED  source or host supplied this signal / position
DERIVED   Omniphony inferred bounded support
EMPTY     no trustworthy signal is assigned
```

For stereo, the finished master remains the musical authority and inferred spatial support stays bounded. For authored multichannel, height, and object audio, supplied geometry outranks inference. Already-binaural material must not be blindly virtualized a second time.

## Canonical source scene

The fixed Windows spatial vocabulary is a 17-position **8.1.4.4** scene:

```text
horizontal
FL FR C LFE SL SR BL BR BC

upper
TFL TFR TBL TBR

lower
BFL BFR BBL BBR
```

This is a semantic coordinate frame, not a claim that every source contains seventeen authored channels. Dynamic XYZ objects remain continuous objects beside the static frame rather than being snapped into fixed anchors.

The semantic source scene and internal rendering geometry are deliberately different concepts. The current renderer may use a denser support shell internally, but internal support directions are not authored input channels.

## Windows product boundary

The intended product experience is simple:

```text
Windows audio
     ↓
Omniphony
     ↓
headphones
```

The Windows host owns capture/playback integration, endpoint lifecycle, recovery, installation, update, uninstall, and optional preferences. The portable core owns source-scene semantics and rendering. Windows concepts such as WASAPI, WDK, registry state, endpoint identity, and tray/service lifecycle must not leak into the renderer's portable semantic contract.

Normal rendering is headless. It does not require a virtual cable, loopback host, or foreground audio application to remain open.

The detailed Windows product and host contract is [`docs/omniphony-for-windows.md`](docs/omniphony-for-windows.md). Current unresolved Windows work is tracked only in [`ROADMAP.md`](ROADMAP.md).

## Stereo fidelity law

Stereo remains a first-class source type rather than a compatibility afterthought.

The finished master stays explicitly present. Omniphony may infer bounded width, depth, height, ambience, source extent, and externalization support, but spatial dimension may not be purchased by damaging clarity, impact, center stability, timbre, dynamics, rhythmic precision, or bass integrity.

> **OFF may collapse the world. It may not bring the rhythm section back to life.**

Physical listening decides perceptual promotion. Measurements, papers, and simulations guide candidate mechanisms but do not redefine listening success after the fact. Accepted and rejected audible mechanisms are preserved in [`docs/listening-history.md`](docs/listening-history.md).

## Realtime law

Realtime paths must remain bounded and deterministic for equivalent continuous input/state. Callback-facing code should use preallocation, bounded queues, explicit discontinuity/reset behavior, and worker-owned allocating DSP where needed.

Realtime callbacks must not perform filesystem or network I/O, device/session enumeration, unbounded allocation, UI work, blocking logging, research-time analysis, or other unbounded operations.

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

Claims should stop at the strongest evidence actually obtained.

For current incomplete work and acceptance gates, see [`ROADMAP.md`](ROADMAP.md). For Windows product semantics, see [`docs/omniphony-for-windows.md`](docs/omniphony-for-windows.md). For physical listening evidence, see [`docs/listening-history.md`](docs/listening-history.md).

## Repository map

```text
omniphony-renderer/renderer/
  portable DSP, HRTF, inference, scene, and source-rendering machinery

omniphony-renderer/orender_engine/
  headless renderer construction and execution boundary

omniphony-renderer/realtime_ffi/
  realtime ABI used by host paths

omniphony-renderer/windows_installer/
  Windows host, APO, installer, diagnostics, and product integration

layouts/
  canonical and internal rendering geometry

docs/
  focused contracts, preserved evidence, and research history
```

The filesystem is an ownership map, not a status dashboard. When exact current contents matter, inspect the repository rather than copying a directory inventory into more documents.

## Build and tests

From `omniphony-renderer/`:

```sh
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
