# Omniphony for Windows

This document is the canonical **durable Windows product/host contract** for Omniphony.

It does **not** own current implementation status, the active engineering frontier, or historical experiments.

- unresolved current work and acceptance gates → `../ROADMAP.md`;
- executable behavior → `../omniphony-renderer/`, tests, and `.github/`;
- physical listening evidence → `listening-history.md`;
- Windows spatial-source semantics → `windows-spatial-input-contract.md`;
- experimental provider findings → the focused Windows provider research/experiment documents;
- chronology and former implementations → Git history.

> **The Windows host may change. The Omniphony renderer and source-authority laws must survive the host.**

## Product boundary

Omniphony for Windows is a system-wide spatial renderer for headphones.

The intended user experience is deliberately small:

```text
install once
→ choose / retain the physical headphone endpoint
→ Omniphony renders system audio headlessly
→ optional tray/preferences surface
→ headphones
```

Normal product operation should require:

- one installer path;
- no virtual cable as a user-facing product requirement;
- no loopback host that must remain open;
- no console or foreground audio-host window;
- no resident UI process carrying the audio stream;
- a small control/recovery surface at most;
- rendering that continues when that UI surface is closed.

Windows is the first host, not the architecture of Omniphony. A future macOS, Linux, game-engine, XR, or media-player host should reuse the portable scene and renderer contracts without inheriting WASAPI, WDK, registry, endpoint, tray, or Windows-service concepts.

## One renderer across source types

Omniphony is not a stereo enhancer plus a separate surround/object renderer. It is one renderer whose behavior becomes more source-authoritative as richer input arrives.

```text
stereo
→ preserve the finished master
→ infer only missing spatial support

authored PCM bed
→ preserve supplied channels / positions
→ infer less

static spatial objects
→ preserve supplied roles and PCM

dynamic objects
→ preserve identity, PCM, and continuous geometry

all trustworthy source forms
→ one Omniphony scene
→ one final binaural render
→ stereo headphones
```

> **The richer the source truth, the less Omniphony invents.**

Already-binaural stereo is not equivalent to raw spatial objects. When reliable provenance says another renderer has already produced the final headphone presentation, Omniphony must not blindly perform a second HRTF virtualization pass.

## Source authority

The Windows adapter must preserve provenance rather than silently reclassify inferred information as authored information.

The fixed spatial vocabulary is the 17-position **8.1.4.4** static scene:

```text
horizontal
FL FR C LFE SL SR BL BR BC

upper
TFL TFR TBL TBR

lower
BFL BFR BBL BBR
```

Each static lane has one authority state:

```text
AUTHORED  source or host supplied the signal / position
DERIVED   Omniphony inferred bounded support
EMPTY     no trustworthy signal is assigned
```

The 17-position scene is a semantic source vocabulary, not a claim that every stream contains seventeen channels.

Dynamic XYZ objects remain continuous objects beside the static scene. Do not snap them to static anchors merely to fit an implementation convenience.

LFE remains semantically distinct from directional sources.

## Portable-core boundary

Windows owns host concerns:

```text
device/session discovery
Windows audio APIs
endpoint association
format translation
clock/cadence adaptation
lifecycle and recovery
installation / update / uninstall
Windows UI and service integration
```

The portable Omniphony core owns:

```text
source scene
source authority
channel/object geometry
presentation state
spatial rendering
binaural output
```

Do not move Windows endpoint identities, device names, registry state, WASAPI concepts, COM lifetime rules, or installer state into portable renderer semantics to solve a host problem.

## Ingress law

Conventional PCM and Windows Spatial Audio are different ingress representations into the same portable scene.

For conventional PCM, the host must preserve every trustworthy authored channel identity it receives. Missing roles remain missing rather than being synthesized and relabeled as authored.

For Windows Spatial Audio, a claimed object path must prove that the original static/dynamic representation reaches Omniphony **before** another headphone renderer collapses it to binaural stereo.

A valid object ingress preserves, as applicable:

- static role identity;
- object identity;
- PCM;
- continuous coordinates;
- object volume;
- update timing;
- lifetime / end-of-stream semantics;
- authoritative host metadata.

Opening or probing `ISpatialAudioClient`, observing registry state, constructing a COM object, or compiling a provider is not by itself evidence that another application's authored objects reach Omniphony. Current proof obligations belong in `../ROADMAP.md`.

## Single-render egress law

Every source may be spatially rendered by Omniphony at most once.

```text
source
→ Omniphony spatial render
→ transparent physical egress
→ headphones
```

Forbidden:

```text
source
→ Omniphony spatial render
→ Omniphony spatial render again
→ headphones
```

Any Windows egress seam used after Omniphony has already produced final binaural stereo must therefore be demonstrably transparent to Omniphony's own spatial processing.

The physical endpoint may remain ordinary stereo even when richer source geometry exists upstream.

## Realtime law

Windows realtime callbacks must remain bounded and deterministic.

They must not perform:

- filesystem or network I/O;
- device/session enumeration;
- model or research-time inference;
- SOFA/profile parsing;
- unbounded allocation;
- renderer graph construction;
- thread creation;
- blocking logging;
- unbounded waits.

Prepare, allocate, discover, validate, and publish state outside the realtime callback. Callback-facing paths use bounded/preallocated state, explicit discontinuity semantics, and observable overflow/underrun behavior.

If a worker-based renderer is used, the callback boundary must fail safely without turning host scheduling into source-authority or DSP semantics.

## Clock and continuity law

Renderer-internal processing quantum and endpoint cadence are separate obligations.

The Windows host owns adaptation between them. It must not require a physical endpoint to adopt a renderer-internal block size merely because that size is convenient upstream.

Power cycling, temporary endpoint absence, device restart, sample-rate changes, and audio-service restarts are availability/lifecycle events, not permission to erase product installation state.

A genuinely changed endpoint identity may require reattachment, but temporary absence must remain recoverable.

## Installation and rollback law

Installation, upgrade, activation, repair, and uninstall are transactions.

Before mutating a working audio graph, Omniphony must retain enough prior state to recover ordinary audio if the richer path fails.

Required properties:

- fail closed before claiming an unproven richer ingress;
- never leave Windows selected on a provider/path that accepts audio but cannot deliver it;
- preserve the previous known-good state until the replacement is verified;
- do not overwrite in-use immutable binaries as an update strategy;
- keep machine-specific mutations attributable to Omniphony;
- restore Omniphony-owned changes on rollback/uninstall;
- never remove or replace the user's physical audio driver as part of ordinary uninstall.

Signing or DriverStore packaging may change deployment mechanics. It must not create a second renderer architecture or weaken these transaction laws.

## UI and profile law

The normal UI is a control surface, not an audio transport.

Closing a tray/preferences process must not stop rendering.

Public/default tuning and listener-specific tuning are separate authority layers. Hardware EQ, hearing-asymmetry compensation, individualized HRTF selection, comfort level, and other personal settings belong in an explicit profile unless separately generalized and validated.

A successful personal configuration is evidence for personalization, not automatic evidence for a public default.

## Evidence and promotion law

Keep these states distinct:

```text
source compiles
≠ tests pass
≠ host API negotiates
≠ endpoint path initializes
≠ a real application supplies the claimed source representation
≠ physical headphones receive the intended one-render output
≠ physical listening confirms the percept
```

Do not promote Windows capability beyond the strongest boundary actually crossed.

Current implementation status is derived from code, tests, CI, and current physical evidence. Do not recreate a hand-maintained implementation dashboard in this contract.

## Stable product target

The Windows host should be able to grow upward in source authority without changing Omniphony's identity:

```text
stereo
→ authored surround / height
→ static spatial objects
→ dynamic spatial objects
```

Each richer source representation should preserve more source truth, require less inference, and converge on the same portable renderer and one final binaural output.
