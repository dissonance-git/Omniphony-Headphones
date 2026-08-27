# Omniphony for Windows

This document is the canonical **Windows product, ingress, egress, lifecycle, and installation contract** for Omniphony.

It does not own current implementation status, experiment history, registry research notes, temporary development transports, or completed diagnostics. Current unresolved gates live in `../ROADMAP.md`; executable behavior lives in code/tests/CI; Git owns chronology.

> **Windows is the reference host, not the product boundary. The Omniphony renderer and source-authority laws must survive every host.**

## 1. Product boundary

Omniphony for Windows is a system-wide spatial renderer for headphones.

The intended user experience is deliberately small:

```text
install once
→ select/retain physical headphone endpoint
→ Omniphony renders system audio headlessly
→ optional tray/preferences control surface
→ headphones
```

Normal product operation should require:

- one installer path;
- no user-facing virtual cable requirement;
- no loopback host that must remain open;
- no console/foreground audio-host window;
- no resident UI process carrying the audio stream;
- rendering that continues when the UI closes;
- a final binaural headphone render even when richer source geometry exists upstream;
- preservation of the selected endpoint's verified baseline mix geometry rather than forcing a project-chosen channel count merely to prove a route;
- ordinary Current processing that does not depend on Windows Sonic or another Spatial Sound provider being selected.

Windows is the first and current reference host, not the architecture of Omniphony. It is where endpoint behavior, native spatial ingress, physical listening, recovery, installer lifecycle, and the reference product experience are being perfected first because it is the actively used development environment.

That priority is sequencing, not platform lock-in. The accepted renderer, canonical scene contract, source-authority rules, stereo presentation law, and final binaural stage are intended to be reused by future macOS, Linux, and Android hosts. Those ports may require different system-audio and spatial-ingress adapters, but they must not create separate Omniphony scene models or alternate product identities.

## 2. Host/core ownership

Windows owns:

```text
device/session discovery
Windows audio APIs
endpoint association
source ingress
format translation
clock/cadence adaptation
lifecycle and recovery
transparent physical egress
installation / update / rollback / uninstall
Windows UI/service integration
```

The portable core owns:

```text
host-neutral scene contract
source scene
source authority
fixed-channel/object semantics
metric geometry and source timing
bounded stable source identity
presentation state
spatial rendering
binaural output
```

Windows adapters lower native role/object metadata into the host-neutral scene contract before renderer DSP. Compatibility entry points may wrap that canonical path, but they may not own a second worker model, fallback mixer, lifecycle model, scene vocabulary, or object renderer.

The Windows adapter is the reference implementation of this boundary, not the definition of it. A future Core Audio, Linux audio-stack, or Android adapter must be able to supply the same canonical scene without importing WASAPI, WDK, COM, registry, APO, or Windows Spatial Sound concepts into portable code. When another platform exposes different source metadata, preserve the strongest trustworthy representation that platform actually supplies and adapt it into the same source-authority model.

Do not move endpoint identities, registry state, WASAPI concepts, COM lifetime rules, provider registration, tray/service state, or installer state into portable renderer semantics.

## 3. One renderer across Windows source types

```text
stereo
→ preserve finished master
→ infer only missing support

authored PCM bed
→ preserve supplied roles
→ infer less

Windows static spatial roles
→ preserve supplied role + PCM

Windows dynamic objects
→ preserve identity + PCM + continuous geometry

all trustworthy forms
→ canonical Omniphony scene
→ one final binaural render
→ headphones
```

Already-binaural stereo is different from raw spatial input. When trustworthy provenance says another renderer already produced the final headphone presentation, Omniphony must not blindly virtualize it again.

The conventional PCM Current path is an Omniphony path in its own right. It must remain functional with Windows Spatial Sound disabled. A selected Windows Sonic or other provider is a separate host state and may not be treated as a hidden dependency of ordinary Current rendering.

## 4. Windows source representations remain distinct

Conventional shared-mode PCM and Windows Spatial Audio are different ingress representations into the same portable scene.

### Conventional PCM

Preserve every trustworthy authored channel identity supplied by the host. Missing roles remain missing. Do not synthesize absent roles and label them authored.

### Static Spatial Audio roles

The complete fixed semantic vocabulary is **8.1.4.4 / 17 roles**:

```text
horizontal
FL FR C LFE SL SR BL BR BC

upper
TFL TFR TBL TBR

lower
BFL BFR BBL BBR
```

The frame is a semantic vocabulary, not a claim that every source contains all roles.

### Dynamic objects

Dynamic objects remain continuous objects beside the fixed frame. Preserve:

- stable object identity;
- PCM association;
- continuous metric XYZ where supplied;
- authored radial distance implied by that metric geometry;
- object gain/volume;
- exact update timing after conversion to source sample time;
- lifetime/EOS;
- other authoritative host metadata that belongs to the source contract.

Persistent dynamic IDs use bounded stable sparse lanes so one object appearing or ending does not reset unrelated object history. Host update-quantum boundaries are transport boundaries, not trajectory semantics. Position smoothing/interpolation must be defined on source sample time.

Static-only compatibility ingress and combined static+dynamic ingress must converge into the same canonical object-rendering ownership before DSP. The static compatibility surface must not grow independent scene, worker, fallback, or rendering behavior.

Do not snap moving objects to static anchors merely for implementation convenience.

## 5. Authority states

Every static role has one source-authority state:

```text
AUTHORED  source/host supplied this signal or position
DERIVED   Omniphony inferred bounded presentation support
EMPTY     no trustworthy signal is assigned
```

LFE remains semantically distinct from directional sources.

The endpoint's Windows mix geometry does not reduce source authority upstream. A multichannel `GetMixFormat` result is not by itself evidence that Omniphony failed to produce a binaural headphone render.

## 6. Conventional PCM and Spatial Audio are separate host seams

A conventional SFX/APO path may carry stereo and authored multichannel PCM. That does **not** prove raw `ISpatialAudioClient` object interception.

A claimed Spatial Audio provider/object path must prove that authored static/dynamic source state reaches Omniphony **before** another headphone renderer collapses it to binaural stereo.

Opening `ISpatialAudioClient`, compiling a provider, observing registry keys, constructing COM objects, or initializing a probe is not equivalent to receiving another application's authored objects.

Do not reconstruct object positions from already-rendered binaural audio and call them native objects.

If Windows exposes no supported richer scene seam, preserve that as a boundary rather than fabricating one through process injection, hooks, or a user-visible capture device.

## 7. One-render law

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
→ Omniphony spatial processing again
→ headphones
```

For the conventional APO route, successful steady state must not leave both the stream SFX and the stereo EFX processing the same graph. The promoted stream SFX is the ordinary processing route; the EFX is a rollback floor for supported stereo graphs and must be absent from the promoted steady state.

This one-render law applies equally to ordinary PCM and a future Spatial Sound provider.

## 8. Provider egress must bypass Omniphony's ordinary ingress processing

If a Windows Spatial Sound provider has already produced final binaural stereo, that output must not re-enter Omniphony's ordinary stream SFX/stereo inference path.

A RAW shared stereo stream is a preferred Windows candidate because Windows documents RAW processing as bypassing ordinary SFX processing, but RAW capability is not assumed on every endpoint.

A provider egress is acceptable only after the exact physical endpoint proves the chosen path is transparent to Omniphony's own ordinary spatial processing.

For RAW specifically, treat these as separate evidence states:

```text
endpoint reports RAW support
≠ RAW stereo client initializes
≠ final provider audio reaches physical endpoint
≠ one-render path is physically verified
```

If RAW cannot be created, do not silently route final binaural output through the normal Omniphony SFX. Any fallback must independently prove transparent egress or coordinate a safe explicit bypass.

Exclusive-mode ownership is not the default fallback merely because it can avoid SFX; it may interfere with ordinary system audio and must earn its product semantics separately.

## 9. Already-binaural policy

Three cases remain distinct:

```text
raw authored scene reaches Omniphony
→ Omniphony performs final spatial render

another renderer already produced binaural stereo
→ Omniphony spatial bypass
→ only separately validated non-spatial correction may remain

unknown stereo
→ conservative deterministic policy
```

Stereo channel count alone is not sufficient provenance.

## 10. Realtime and cadence law

Windows realtime callbacks remain bounded and deterministic and obey `realtime-control-contract.md`.

The host owns adaptation between:

```text
Windows callback / endpoint cadence
renderer processing quantum
worker queue cadence
physical output cadence
```

The physical endpoint must not be forced into an internal renderer block size or channel-count assumption merely because it is convenient upstream.

Overflow, underrun, recovery, mute, drop, and backpressure behavior must be bounded and observable. Continuity-critical processed audio must not be silently discarded.

## 11. Single physical route

A trustworthy listening or product path has exactly one audible route to the physical headphones.

```text
source
→ Omniphony
→ physical endpoint
```

not:

```text
source ─────────────→ physical endpoint
   └→ Omniphony ───→ physical endpoint
```

Duplicate delayed physical routes can create combing, hollow tone, echo, and false renderer conclusions.

Bypass must also be route-clean: no stale wet tail, duplicate dry path, old forwarding path, or simultaneous Omniphony SFX+EFX route may remain audible after the state change.

## 12. Lifecycle and recovery

Power cycling, temporary endpoint absence, default-device changes, audio-service restart, suspend/resume, and format/period changes are host lifecycle events.

They are not permission to erase installation state or source authority.

A genuinely new endpoint identity may require reattachment. Temporary absence should remain recoverable.

Host recovery must not make source semantics depend on callback accidents or stale device generations.

## 13. Installation, activation, and rollback are transactions

Before mutating a working audio graph, retain enough prior state to recover ordinary audio if the richer path fails.

Required properties:

- fail closed before claiming an unproven richer ingress;
- never leave Windows selected on a provider/path that cannot render;
- preserve previous known-good state until replacement is verified;
- snapshot and preserve the selected endpoint's baseline mix geometry rather than requiring a universal stereo floor;
- use immutable/versioned deployment generations rather than overwriting in-use binaries;
- keep machine-specific mutations attributable to Omniphony;
- restore Omniphony-owned mutations on rollback/uninstall;
- never remove/replace the user's physical audio driver during ordinary uninstall;
- provider selection/deselection and uninstall recovery must be safe even after partial failure.

Signing, DriverStore packaging, or deployment mechanics may change. They must not create a second renderer architecture or weaken rollback law.

A client-format probe and an endpoint-mix probe answer different questions. A diagnostic that refuses to test a richer client format solely because `GetMixFormat` is multichannel is not evidence that the SFX or renderer is inactive; it is an insufficient diagnostic for that endpoint geometry.

## 14. UI and profile law

The normal UI is a control surface, not an audio transport.

Closing tray/preferences must not stop rendering.

Public/default tuning and listener-specific tuning are separate authority layers. Hardware EQ, hearing-asymmetry compensation, individualized HRTF selection, comfort level, and other personal settings belong in explicit calibration/profile state unless separately generalized and validated.

A successful personal configuration is evidence for personalization, not automatic evidence for a public default.

## 15. Conventional APO steady-state evidence

The conventional Windows APO route has several distinct proof layers.

A healthy promoted graph should satisfy the structural invariant:

```text
stream SFX attached
+ stereo EFX absent
+ system effects enabled
+ selected physical endpoint retained
```

Registry attachment alone does not prove the Windows audio engine instantiated the processing path. While real playback is active, stronger runtime evidence is:

```text
AudioDG process exists
→ OmniphonyStreamAPO.dll is loaded in that AudioDG process
→ omniphony_realtime.dll is loaded in that AudioDG process
```

Module presence proves live instantiation/loading, not mathematically that every source sample took the intended transform. Sample-path tests, route-clean physical playback, and listening remain separate evidence layers.

Endpoint geometry is independent evidence. A valid graph may coexist with a stereo or multichannel Windows mix format. Health checks must report the actual geometry and compare it with the preserved baseline rather than declaring any non-stereo mix intrinsically broken.

Ordinary Current rendering must also remain effective with Windows Sonic disabled. Similar listening with Sonic enabled and disabled may support the conclusion that Current is not relying on Sonic, but it does not prove native Windows Spatial Audio object interception or provider integration.

## 16. Capability evidence

Keep these states distinct:

```text
source compiles
≠ tests pass
≠ Windows API negotiates
≠ endpoint/client path initializes
≠ SFX registry attachment succeeds
≠ AudioDG instantiates the Stream APO
≠ AudioDG loads the realtime renderer DLL
≠ the intended source representation enters that graph
≠ the intended samples are transformed exactly once
≠ provider enumerates/selects
≠ real application supplies claimed authored representation
≠ Omniphony receives that representation
≠ physical endpoint receives one-render output
≠ physical listening confirms the percept
```

Do not promote Windows capability beyond the strongest boundary actually crossed.

Current unresolved proof obligations belong in `../ROADMAP.md`. Once resolved, fold only the durable product consequence here or into executable tests/code. Do not create a provider research archive.

## 17. Stable Windows reference target

The Windows host should grow upward in source authority without changing Omniphony's identity:

```text
stereo
→ authored surround / height
→ static spatial roles
→ dynamic spatial objects
```

Each richer representation should preserve more source truth, require less inference, converge on the same portable renderer, and reach the headphones through exactly one final binaural render.

Once Windows has proven these semantics end to end, later platform hosts should reuse that portable core and repeat only the platform-specific ingress, egress, lifecycle, packaging, and application-compatibility proofs. A Windows-specific workaround is not a portable feature unless its durable semantic consequence can be expressed without Windows concepts.
