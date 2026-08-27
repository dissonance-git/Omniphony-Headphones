# Omniphony

Omniphony is an open-source, platform-agnostic spatial audio compiler and headphone renderer.

It is designed to sit at the operating-system audio boundary, preserve the strongest spatial truth each stream actually supplies, lower that truth into one canonical scene, and perform exactly one final binaural render.

> **More source truth means less inference. One scene. One renderer. One headphone output.**

Windows is the reference and hardening host because it is the environment currently used for physical listening, endpoint integration, installer work, and native spatial-ingress proof. Windows is not the architecture. The portable core is intended to be reused by future macOS, Linux, Android, and other hosts.

## Product model

```text
ordinary stereo
→ preserve the finished master
→ derive only bounded missing spatial support

authored channel bed
→ preserve supplied channels and positions

static spatial roles
→ preserve supplied roles

dynamic objects
→ preserve identity + PCM + continuous geometry

Ambisonics / HOA
→ preserve the supplied field

already-binaural material
→ avoid a second spatial render

all trustworthy source forms
→ canonical Omniphony scene
→ one portable spatial renderer
→ one final binaural render
→ headphones
```

Source semantics are stream-local. There is no global "spatial mode" that rewrites unrelated streams. A stereo music player and a native-spatial game may coexist while each keeps the strongest representation its own host path exposes.

The canonical source/scene law lives in [`docs/scene-renderer-contract.md`](docs/scene-renderer-contract.md).

## Windows reference host

The current Windows product target is deliberately simple:

```text
Windows applications
      ↓
platform ingress
      ↓
canonical Omniphony scene
      ↓
portable renderer
      ↓
physical headphones
```

Ordinary shared PCM and Windows Spatial Audio are different ingress seams into the same renderer. A conventional stereo or multichannel APO path does not prove native object interception; native spatial claims remain gated until a real application supplies authored objects to Omniphony and the physical one-render path is proven end to end.

Windows-specific endpoint, APO, COM, registry, provider, lifecycle, installer, and recovery state belongs to the Windows host. It must not leak into portable scene or renderer semantics.

See [`docs/omniphony-for-windows.md`](docs/omniphony-for-windows.md).

## Protected stereo presentation

Stereo is a first-class source type, not a compatibility fallback.

The finished master remains present while Omniphony adds bounded derived support. Spatial scale may not be purchased by damaging clarity, bass/body, transient ownership, center stability, dynamics, timbre, rhythmic precision, or authored motion.

> **OFF may collapse the world. It may not bring the rhythm section back to life.**

The Windows-validated Current presentation is a protected renderer baseline. Audible changes begin outside the accepted path and enter Current only after engineering validation plus clean-route physical listening. Once promoted, the surviving invariant belongs in the living contract and executable regression surface; Git keeps the discarded experiment history.

See [`docs/music-presentation-contract.md`](docs/music-presentation-contract.md) and [`docs/binaural-renderer.md`](docs/binaural-renderer.md).

## Repository landmarks

The repository uses the same surface law as Helix: **one concept, one writable owner; derive what can be derived; Git owns superseded repository narrative.**

| Surface | Canonical responsibility |
| --- | --- |
| `README.md` | product identity, architecture, entry routes |
| `AGENTS.md` | repository/change/evidence/concurrency law |
| `ROADMAP.md` | unresolved gates and sequencing only |
| `CONTRIBUTING.md` | contributor setup and validation procedure |
| `docs/scene-renderer-contract.md` | source, scene, channel/object semantics |
| `docs/music-presentation-contract.md` | stereo/music presentation invariants |
| `docs/binaural-renderer.md` | portable binaural rendering invariants |
| `docs/realtime-control-contract.md` | sample-time, realtime, latency, failure law |
| `docs/omniphony-for-windows.md` | Windows ingress/egress/lifecycle/install contract |
| `docs/headphone-calibration.md` | listener/headphone calibration boundary |
| `docs/osc-control-contract.md` | OSC/control semantics |
| `docs/game-music-realtime-source-contract.md` | recovered-source/game-music semantics |
| `omniphony-renderer/` | executable implementation |
| `.github/workflows/` | executable CI and release validation |
| `.agents/` | agent procedure and connector re-entry routes |
| Git history | chronology and retired alternatives |

A working-tree document must own a current obligation. Historical listening notes, completed research stages, old host instructions, migration diaries, duplicate status surfaces, and inert workflow copies do not stay active merely because they once mattered.

## Portable core

Portable code owns:

```text
source authority
canonical scene
fixed channels + dynamic objects
metric geometry + source time
presentation state
spatial compilation
HRTF / ITD / room rendering
final binaural output
```

Platform hosts own only platform-specific discovery, ingress, egress, cadence, device/session lifecycle, packaging, update/uninstall, and UI/service integration.

Future macOS, Linux, and Android ports should be host adapters around the same core, not separate Omniphony products.

## Realtime and evidence law

Realtime behavior is defined on the logical sample timeline, not on callback accidents. Audio callbacks stay bounded and deterministic; blocking I/O, device discovery, large/unbounded allocation, UI work, and research-time analysis stay off the realtime path.

Evidence states remain separate:

```text
source exists
≠ code builds
≠ tests pass
≠ host API negotiates
≠ intended representation enters Omniphony
≠ physical endpoint receives one-render output
≠ listening confirms the intended percept
```

Claims stop at the strongest evidence actually obtained.

## Build and contribution entry

From `omniphony-renderer/`, the common portable checks include:

```sh
cargo test -p scene_contract
cargo test -p renderer
cargo test -p orender_engine --lib --tests
cargo test -p source_ffi --lib --tests
cargo test -p realtime_ffi
```

Windows host/APO changes also require the task-relevant Windows CI gates.

Start with [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`AGENTS.md`](AGENTS.md). Reasoning agents entering through GitHub derive their first-connection packet from [`.agents/github-agent-bootstrap.py`](.agents/github-agent-bootstrap.py) plus the live [`.agents/skills/`](.agents/skills/) inventory; no committed connector-state file owns current repository truth. Current unresolved work lives only in [`ROADMAP.md`](ROADMAP.md).

## Success condition

> **A finished source keeps its identity, weight, dynamics, clarity, and authored spatial truth while the headphones present a stable external 3-D world with convincing front/back, height, depth, motion, extent, and envelopment.**
