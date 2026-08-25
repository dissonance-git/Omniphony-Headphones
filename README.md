# Omniphony

Omniphony is an open-source spatial audio renderer for headphones.

Its goal is to occupy the same broad class of system audio role as proprietary headphone spatial renderers such as Dolby Atmos for Headphones, DTS Headphone:X, Windows Sonic, Sony 360-style rendering systems, and Waves Nx, while keeping the renderer, scene model, source-authority rules, DSP, validation, and research inspectable.

> **One open spatial renderer that enhances stereo, preserves native surround, accepts true spatial scenes when available, and performs the final headphone render itself.**

Windows is the first product host. The renderer, scene contract, and DSP core are designed to remain portable.

## Product law

Omniphony is not a stereo enhancer plus a separate surround renderer. It is one spatial renderer whose behavior becomes more source-authoritative as richer input becomes available.

```text
stereo
→ preserve the finished master
→ infer only spatial structure that the source does not explicitly contain
→ enhance through Omniphony

5.1 / 7.1 / height PCM
→ preserve authored channels and positions
→ infer less because more of the scene is already known
→ enhance through the same renderer

8.1.4.4 static spatial scene
→ preserve supplied fixed spatial roles
→ avoid reconstructing geometry already supplied by the source
→ enhance through the same renderer

8.1.4.4 + dynamic XYZ objects
→ preserve fixed scene structure and continuous object motion
→ give supplied geometry maximum authority
→ enhance through the same renderer
```

The richer the source truth, the less Omniphony invents.

Stereo is the hardest case because only two channels are available. Native surround should be a stronger input to the same enhancement system because authored direction replaces guesswork. Static and dynamic spatial objects are richer again.

Every path ends in one binaural render to an ordinary stereo headphone endpoint.

## Windows-wide architecture

The intended product experience is simple:

```text
Windows audio
     ↓
Omniphony
     ↓
headphones
```

Internally, Omniphony preserves the richest trustworthy representation supplied by the source:

```text
ordinary stereo ───────────────┐
5.1 / 7.1 PCM ─────────────────┤
height PCM ────────────────────┤
static spatial objects ────────┤
dynamic XYZ objects ───────────┤
                               ↓
                     canonical source scene
                               ↓
                       Omniphony renderer
                               ↓
                         binaural stereo
                               ↓
                           headphones
```

The Windows product is headless. Audio rendering does not depend on a resident foreground application, virtual cable, or loopback host. A small tray component may expose preferences, but it does not carry the audio stream.

## Current Windows baseline

The current Windows host accepts stereo and authored multichannel shared-mode PCM through a format-changing stream SFX while the physical headphone endpoint remains stereo.

The native surround baseline is:

```text
48 kHz / float32 / authored 7.1 client stream
        ↓
Omniphony stream SFX
        ↓
AUTHORED FL FR C LFE SL SR BL BR
        ↓
Omniphony source scene
        ↓
Current spatial renderer
        ↓
48 kHz / 32-bit / stereo endpoint
        ↓
headphones
```

The physical endpoint remaining stereo is intentional. Richer source geometry exists upstream of the final endpoint mix and is reduced to two channels by Omniphony.

A stereo endpoint EFX remains available as a transactional rollback and recovery floor. After successful native-surround promotion, the stream SFX is the steady-state path and the temporary stereo EFX is removed so the signal is rendered once.

## Canonical spatial scene

Omniphony uses a **17-position 8.1.4.4 static scene** as its canonical Windows spatial vocabulary:

```text
horizontal
FL FR C LFE SL SR BL BR BC

upper
TFL TFR TBL TBR

lower
BFL BFR BBL BBR
```

This is a coordinate vocabulary, not a claim that every source contains seventeen authored channels.

Every static lane has an authority state:

```text
AUTHORED  source or host supplied this signal / position
DERIVED   Omniphony inferred bounded support
EMPTY     no trustworthy signal is assigned
```

For a conventional authored 7.1 stream:

```text
FL FR C LFE SL SR BL BR = AUTHORED
BC TFL TFR TBL TBR BFL BFR BBL BBR = EMPTY unless separately earned
```

For stereo, Omniphony may derive bounded spatial support while preserving the finished master as the musical authority.

Dynamic spatial objects sit beside the static scene rather than being forced into it:

```text
8.1.4.4 static scene
        +
continuous dynamic XYZ objects
        ↓
one Omniphony source scene
```

When exact object coordinates are supplied, they outrank inferred geometry and should remain continuous as far into rendering as possible.

## Renderer geometry

The canonical 8.1.4.4 scene and Omniphony's internal rendering geometry are deliberately different concepts.

```text
source truth
        ↓
8.1.4.4-capable semantic scene
+ continuous objects where supplied
        ↓
source authority / provenance
        ↓
22-direction Current support shell
        ↓
HRTF / ITD / distance / room
        ↓
binaural stereo
```

The **8.1.4.4 scene is the semantic skeleton**. The **22-direction shell is internal rendering geometry**. It does not represent twenty-two authored Windows input channels.

## Stereo enhancement

Stereo remains a first-class source type rather than a compatibility afterthought.

The finished stereo master remains protected. Omniphony may analyze it to infer bounded width, depth, height, ambience, source extent, and externalization support, but spatial dimension may not be purchased by damaging clarity, impact, center stability, timbre, dynamics, or rhythmic precision.

> **OFF may collapse the world. It may not bring the rhythm section back to life.**

The stereo path therefore combines protected source material with bounded evidence-derived spatial support rather than treating a two-channel master as if it were an authored object scene.

### Current listening baseline

The retained stereo Current listening state is the `ddeb0b15` lineage.

Its accepted structure is:

```text
protected stereo master
+ coherent foundation
+ 17-lane evidence-derived support field
        ↓
22-direction cascaded Current shell
        ↓
SAF/KEMAR binaural render
        ↓
Noire X output profile
        ↓
stereo-linked transient enhancement
        ↓
output trim
        ↓
final peak guard
```

The frontal-hemisphere correction is physically accepted: the front now has real occupancy rather than leaving the convincing side/rear shell perceptually dominant. In the 1.2–5 kHz presence band, the derived field transfers 30% of existing rear support forward at baseline and up to 38% with stable frontal-anchor evidence. Top-rear support transfers top-front at 18% baseline and up to 22% with the same bounded evidence. These are sum-preserving transfers of already-earned support, not copied wet energy or a synthetic center channel; side support remains intact.

The Noire X Enhancement is a restrained production-style transient designer. Its audible action remains one stereo-linked broadband gain, while the detector may use frequency-aware analysis to decide which attacks should drive that gain. The attack/sustain form is retained as beneficial; the frequency-aware detector remains independently revisable if later isolated listening reveals a cost.

The next stereo spatial frontier is **frontal depth and externalization, not more frontal quantity**. Further work should prefer physically motivated front-specific early-field structure, binaural reflection cues, distance evidence, and other bounded externalization mechanisms over additional global rear-to-front transfer, synthetic-center duplication, widening, or extra late reverb.

## Native surround and height

When Windows or another host supplies authored multichannel PCM, Omniphony maps the supplied channel mask directly into authored source positions and bypasses stereo spatial inference for those channels.

```text
5.1 / 7.1 / height bed
        ↓
authored channel identity
        ↓
canonical scene
        ↓
Omniphony spatial enhancement
        ↓
one binaural render
```

LFE remains semantically distinct from directional HRTF sources. Missing channels remain empty rather than being silently promoted to authored content.

The stream APO/native-bed path currently supports and regression-tests stereo, authored 7.1, and authored 7.1.4 processing.

## Spatial objects

The ideal Windows ingress is the richest spatial representation the operating system can expose before another headphone renderer collapses it to stereo:

```text
8.1.4.4 static spatial roles
        +
dynamic XYZ objects
        ↓
Omniphony source scene
        ↓
Omniphony spatial enhancement
        ↓
Omniphony binaural render
        ↓
headphones
```

Raw Windows Spatial Audio object ingress is not yet claimed as complete. A supported system boundary must first be demonstrated for receiving another application's static and dynamic spatial representation before Windows Sonic, Dolby, DTS, or another renderer performs the final headphone render.

The Windows provider experiment now has a deliberately gated source-side chain:

1. a standards-shaped `ISpatialAudioClient` capability object;
2. an internal static-only `ISpatialAudioObjectRenderStream` lifecycle with documented `VT_BLOB` activation marshalling;
3. a fixed-topology static-object realtime ABI in `omniphony_realtime.dll` that preserves role identity and authored Windows positions, moves planar object PCM through preallocated rings, and runs the existing source-aware Omniphony renderer on a dedicated worker;
4. a C++ dynamic-loader bridge that opens that ABI only from an explicit absolute DLL path and validates the realtime ABI before creating a processor;
5. a preallocated single-producer/single-consumer stereo clock-domain queue that accepts Current's fixed 480-frame output quanta and can be drained at any legal endpoint period;
6. an exact-endpoint event-driven RAW stereo sink with `IAudioRenderClient` and the Windows sample-ready event;
7. a closed-gate output pump that pre-rolls silence, starts only the explicit RAW endpoint, follows the endpoint event clock, queries current padding, drains exactly the writable frames, and stops/resets cleanly.

The registry-free composed smoke exercises the COM-shaped static stream itself, snapshots each completed immutable-topology object quantum, and hands that planar PCM through the realtime bridge into the existing Current worker. Current's completed stereo quantum is then submitted into the clock-domain queue. That source-side chain is now compiled and regression-smoked without opening a physical endpoint:

```text
COM-shaped static object quantum
→ immutable static role order
→ OmniphonySpatialRealtimeBridge
→ omniphony_realtime.dll
→ existing Current source renderer
→ 480-frame binaural stereo
→ preallocated SPSC stereo queue
```

A separate finite `OmniphonySpatialClosedGateEgressProbe` now exists for the next physical test. It synthesizes two low-level authored static objects, sends them through the same COM-shaped stream and Current bridge, queues the resulting binaural stereo, and lets a Pro Audio event worker drain one explicitly named physical endpoint. It performs **no provider registration, no provider selection, and does not open the public Spatial Audio stream gate**. CI compiles this probe but deliberately never runs its audible physical-endpoint test.

### Single-render RAW egress contract

The returned binaural stereo must not pass through Omniphony's normal APO rendering path again. Windows RAW processing mode is therefore the provider-egress escape hatch, not another renderer mode.

Both Omniphony APOs inspect the Windows audio processing mode supplied at initialization. When that mode is `AUDIO_SIGNALPROCESSINGMODE_RAW`:

- the endpoint APO does not load or invoke `omniphony_realtime.dll`;
- the stream APO does not request a 7.1 upstream format or reduce channels;
- the stream APO accepts only an identity stereo float32 pair;
- valid stereo PCM is copied bit-for-bit when an out-of-place copy is needed;
- silent buffers remain silent;
- reported Omniphony processing latency is zero.

A dedicated raw-APO smoke encodes those invariants and also rejects 7.1 negotiation in RAW mode. This closes a source-level double-render hole before a provider output stream is allowed to exist.

The output side does not assume that the Windows endpoint period equals Omniphony's 480-frame object/render quantum. The RAW sink chooses the endpoint's own reported legal shared-engine period by default, while the preallocated stereo queue performs bounded block-size adaptation. This avoids turning a renderer-internal 10 ms quantum into an unnecessary device-compatibility requirement.

The source implementation now follows the event-driven WASAPI pattern used by Microsoft's renderer sample: pre-roll before `Start()`, wait for the endpoint sample-ready event, call `GetCurrentPadding`, compute writable frames, acquire exactly that many frames from `IAudioRenderClient`, copy queued stereo, and release them. This output pump compiles and its no-endpoint fail-closed contract smoke passes, but **physical playback through this path is not yet claimed**.

The next decisive evidence step is therefore narrower than provider activation: run the finite closed-gate egress probe on the exact real Windows endpoint and verify that real frames reach the RAW render client with no producer drops and acceptable underrun behavior. Only after that should provider enumeration/selection and public object-stream activation move forward.

Until complete application-to-headphones proof exists, the public provider continues to return `SPTLAUDCLNT_E_STREAM_IS_NOT_AVAILABLE`, preventing an unfinished provider from accepting spatial application audio and silently dropping it.

Omniphony does not treat already-binaural stereo as raw objects, and it does not reconstruct object metadata from a final binaural mix and call that native spatial ingress.

## What is implemented

| Layer | State |
| --- | --- |
| Canonical static scene | **Implemented:** 17-position 8.1.4.4 vocabulary |
| Source authority | **Implemented:** AUTHORED / DERIVED / EMPTY semantics |
| Stereo evidence mapping | **Implemented:** bounded stereo-derived spatial support |
| Current support shell | **Implemented:** 22-direction full-sphere rendering lattice |
| Binaural renderer | **Implemented:** measured HRTF / ITD path with distance and room support |
| Windows realtime runtime | **Implemented:** `omniphony_realtime.dll` |
| Windows stereo ingress | **Implemented:** protected stereo Current path |
| Windows authored 7.1 ingress | **Implemented and physically verified:** shared 7.1 client → stream SFX → stereo endpoint |
| Authored 7.1.4 processing | **Implemented and regression-tested** in the stream APO/native-bed path |
| Endpoint continuity / rollback | **Implemented for install/upgrade:** persistent endpoint identity, manual graph reset, and stereo rollback floor; automatic hotplug recovery remains Phase 9 |
| Headless Windows installer | **Implemented:** one installer, no virtual cable or resident audio host |
| Spatial provider capability probe | **Implemented in isolation:** `ISpatialAudioClient`, 17-role mask, object format, deterministic registration/snapshot tooling; real Windows enumeration/selection proof pending |
| Static spatial stream lifecycle | **Implemented behind a closed provider gate:** static object lifecycle + documented `VT_BLOB` activation marshalling |
| Static object → Current realtime path | **Implemented behind the gate:** fixed static-object ABI, dedicated worker, authored positions, safety lane, existing source-aware Current renderer |
| Provider C++ → realtime ABI bridge | **Implemented but not publicly activated:** absolute-path DLL loading, ABI validation, processor lifetime |
| COM quantum → Current composition | **Implemented and registry-free-smoked:** immutable role order, per-object volume/EOS snapshotting, composed COM-to-Current path |
| Current stereo → egress queue | **Implemented and registry-free-smoked:** complete 480-frame Current output enters the downstream SPSC queue without producer drops |
| RAW APO single-render bypass | **Implemented in source:** endpoint and stream APOs become zero-latency transparent stereo paths in Windows RAW processing mode |
| RAW physical-output capability probe | **Implemented in source:** read-only endpoint identity, RAW client properties, stereo float support, engine-period diagnostics |
| RAW physical-output lifecycle | **Implemented and compiled, deliberately unstarted in preflight:** exact endpoint, shared event-driven RAW stereo, endpoint-owned period selection, `IAudioRenderClient`, event handle |
| Spatial egress clock-domain queue | **Implemented, compiled, and smoke-tested:** fixed 480-frame producer, variable consumer periods, zero-filled underruns, non-blocking overflow rejection |
| Endpoint-event RAW output pump | **Implemented, compiled, and fail-closed-smoked without a physical endpoint:** pre-roll, endpoint-event cadence, padding-aware drain, Start/Stop/Reset lifecycle |
| Finite closed-gate physical egress diagnostic | **Implemented and compiled:** explicit endpoint, COM → Current → queue → RAW event pump; physical run pending |
| Provider package staging | **Implemented as an inert future-install primitive:** immutable content-addressed generations, exact file-set verification, full-package hashes, final-path smokes, 64-bit host guard, no provider registration or selection writes |
| Public Windows Spatial Audio object ingress | **In progress:** physical closed-gate egress proof, then real provider enumeration/selection/application proof remain before activation |
| Dynamic XYZ object ingress | **Future after static end-to-end proof** |
| Signed DriverStore deployment | **Optional future deployment route** |

## Source authority

The central rule is simple:

> **Preserve the richest source representation available and invent only what is missing.**

```text
stereo
→ preserve master + infer bounded spatial support

5.1 / 7.1 / height PCM
→ preserve authored channels and supplied geometry

static spatial objects
→ preserve fixed spatial roles and identity

dynamic spatial objects
→ preserve object identity, PCM, and continuous 3-D position

already-binaural material
→ avoid destructive double HRTF virtualization
```

`AUTHORED`, `DERIVED`, and `EMPTY` are provenance states, not cosmetic labels.

## Realtime architecture

The Windows APOs load `omniphony_realtime.dll` through a narrow ABI. Windows realtime callbacks do not run the allocating renderer graph directly. A bounded, preallocated callback-facing path exchanges PCM with a dedicated Current worker.

The static Spatial Audio ABI follows the same law. Its fixed stream topology is copied once at creation, planar object quanta move through preallocated rings, and the allocating source renderer stays on its worker. Directional object positions remain authored geometry, while LFE remains non-directional.

The internal COM-shaped static stream preserves that topology for its lifetime. At `EndUpdatingAudioObjects`, it snapshots active static-role buffers into the fixed planar order, applies object volume and partial end-of-stream semantics, and hands the quantum to a pre-opened transport. No DLL discovery or renderer construction happens on the update call.

RAW egress is deliberately different from the normal source-rendering paths: it is an identity transport for stereo that Current has already rendered. In RAW processing mode neither Omniphony APO is allowed to create a renderer worker, infer geometry, change the channel topology, or add renderer latency.

The physical endpoint owns the downstream clock. Omniphony keeps its fixed 480-frame Current quantum on the producer side and crosses into the endpoint's legal event period through a preallocated SPSC stereo queue. Producer overflow is non-blocking and observable; consumer underrun is explicitly zero-filled rather than exposing stale memory.

The closed-gate output pump performs no device discovery on its event path. Endpoint selection and initialization happen beforehand. Startup pre-rolls silence, then the endpoint event controls consumption. Each drain queries current padding and writes only the frames Windows says are available. A finite manual physical diagnostic uses an MMCSS `Pro Audio` event worker, but the public provider remains disconnected from this pump until physical proof exists.

The runtime includes:

- preallocated callback-facing rings;
- dedicated Current worker processing;
- time-aligned dry/fold-down safety lanes;
- non-finite sanitization;
- linked peak safety;
- explicit create/destroy lifecycle tests;
- static-object role/topology validation;
- preallocated spatial-egress clock adaptation;
- endpoint-event output lifecycle observability;
- manifest, import, and ABI checks in CI.

Realtime callbacks must not perform filesystem I/O, network activity, device discovery, or research-time analysis. The provider's C++ loader performs DLL discovery and ABI validation before processing begins rather than inside an object update callback.

## Validation

Engineering gates cover:

- canonical scene order and authority preservation;
- authored channel-mask identity;
- source identity stability;
- deterministic spatial placement;
- constant-power shell spread;
- HRTF / ITD behavior;
- transient and bass preservation;
- non-finite and peak safety;
- realtime ABI and lifecycle behavior;
- Windows APO registration and manifest contracts;
- endpoint continuity and rollback;
- shared-client multichannel initialization;
- exact two-channel physical output;
- spatial-provider capability and registry-free static-stream lifecycle contracts;
- static-object realtime ABI loading and worker handoff;
- composed COM-shaped static stream → realtime bridge → Current → egress queue transport;
- RAW-mode endpoint/stream APO bit transparency, zero latency, no Current load, and no 7.1 expansion;
- read-only RAW physical-output format/period preflight;
- inert event-driven RAW output initialization against one exact endpoint;
- 480-frame producer to variable-period consumer clock-domain adaptation, wrap, underrun, and overflow behavior;
- closed-gate endpoint pump compile/fail-closed lifecycle without opening a device in CI;
- finite explicit-endpoint egress probe compilation without running audible output in CI;
- content-addressed provider package staging without registry mutation.

Human listening remains the final gate for externalization, front/back discrimination, elevation, source body, envelopment, radial depth, center solidity, room naturalness, fatigue, groove, and bass integrity.

## Windows deployment

The Windows installer configures the selected physical render endpoint directly.

Normal use has:

- one installer executable;
- one UAC elevation;
- no virtual cable;
- no loopback host;
- no console;
- no resident audio-host application;
- a small preference/manual-recovery tray icon;
- rendering that continues if the tray UI is closed.

The current unsigned user-mode APO deployment uses Windows' unprotected AudioDG compatibility mode and records previous machine state for rollback and uninstall.

The future spatial-provider portion of setup is being shaped around immutable, content-addressed generations under the Omniphony install root. A candidate generation is copied to a temporary directory, the exact package file set and every SHA-256 are verified, capability/static-stream/realtime-bridge/clock-domain smokes run before and after the final-path move, and a manifest records the exact generation. Staging refuses a 32-bit PowerShell host on 64-bit Windows so later Program Files and registry-view behavior cannot silently diverge. The staged generation carries the read-only RAW capability probe and inert RAW output-sink probe so a future activation transaction can validate the real endpoint's stereo format, legal engine period, render client, and event ownership before it mutates provider state. This staging primitive performs **no provider registration and no provider selection**.

Activation preflight no longer requires the physical endpoint itself to accept a 480-frame shared-engine period. It verifies the endpoint's legal period, initializes and closes the event-driven sink without starting it, verifies the preallocated cadence adapter, and records whether direct 480-frame consumption happens to be legal as diagnostic information only.

The active output pump and finite audible egress diagnostic remain development evidence tools, not installer activation steps. They should not be staged into the ordinary product transaction until the exact physical-endpoint path has been run and measured successfully.

That gives later provider activation a safer transaction model: never overwrite an in-use COM DLL, never mutate a previously verified generation, keep the previous generation intact for rollback, switch registration only after the new generation has passed final-path checks and endpoint preflight, and restore prior provider state if activation verification fails. Omniphony must never leave Windows selected on a provider that can accept a stream but cannot render it.

A componentized signed DriverStore route remains available as a separate deployment research track without changing the renderer architecture.

## Repository map

```text
omniphony-renderer/renderer/
  portable DSP, HRTF, inference, scene, and source-rendering machinery

omniphony-renderer/orender_engine/
  headless renderer construction and execution boundary

omniphony-renderer/realtime_ffi/
  realtime ABI used by Windows host paths, including fixed static spatial objects

omniphony-renderer/windows_installer/endpoint_apo/
  Windows stream / endpoint APOs, installer, tray, and diagnostics

omniphony-renderer/windows_installer/spatial_provider_probe/
  bounded Windows Spatial Sound provider, static stream, realtime bridge,
  RAW output preflight/lifecycle, clock-domain queue, closed-gate output pump,
  physical egress diagnostic, immutable package staging, registration,
  and evidence experiments

layouts/
  canonical and internal rendering geometry

docs/
  source authority, Windows ingress, spatial scene, and validation contracts
```

## Build and tests

From `omniphony-renderer/`:

```sh
cargo test -p renderer
cargo test -p orender_engine --lib --tests
cargo test -p source_ffi --lib --tests
cargo test -p realtime_ffi
```

Focused CI additionally validates source-aware spatial behavior, the realtime Windows path, APO lifecycle contracts, endpoint tooling, spatial-provider contracts, closed-gate output primitives, and installer packaging.

## Definition of success

> **A finished source keeps its identity, weight, dynamics, clarity, and authored spatial truth while gaining a stable external world with convincing width, depth, height, distance, motion, source extent, and envelopment.**

For stereo, Omniphony should create a richer spatial presentation without pretending inferred geometry was authored.

For native surround, height, and object sources, Omniphony should become progressively less inferential and more authoritative, because the source has already supplied more of the world.

The long-term target is a transparent, inspectable, open spatial renderer that can sit at the Windows audio boundary, receive whatever spatial truth an application can provide, and perform the final headphone render itself.