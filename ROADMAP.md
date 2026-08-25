# Omniphony roadmap

This document contains only work that is not yet accepted, physically proven, productized, or complete.

Completed capability belongs in `README.md`, `AGENTS.md`, implementation history, tests, and listening history. Once a roadmap item is accepted or proven, remove it from this file rather than preserving it as a completed phase.

The product target remains one open spatial renderer that preserves the richest source representation available, performs one final binaural render, and invents only what the source does not already provide.

---

## 1. Prove the physical spatial-output path

Run the finite closed-gate spatial egress diagnostic on the exact physical Windows headphone endpoint.

Required evidence:

```text
COM-shaped authored static source reaches Omniphony
→ binaural stereo reaches the bounded egress queue
→ the real endpoint event clock drains audio through IAudioRenderClient
→ producer drops remain zero
→ underrun behavior is measured
→ provider registration remains untouched
→ public provider selection remains untouched
```

This proof must distinguish source-side correctness, renderer correctness, queue/cadence correctness, and physical endpoint playback. A successful synthetic or registry-free smoke is not a substitute for real endpoint evidence.

Gate:

> Authored static spatial content reaches the real headphone endpoint through Omniphony exactly once while the public Spatial Sound provider remains closed.

---

## 2. Prove Windows Spatial Sound provider enumeration and activation

After physical egress is proven independently, determine whether Windows can safely enumerate and activate Omniphony as a selectable Spatial Sound provider.

The provider-registration surface is undocumented and must remain experimental until physically verified. Do not generalize from registry shape, MSSOAL, Process Monitor observations, or successful COM construction alone.

Required work:

- register only Omniphony-owned provider state;
- verify appearance in the Windows Spatial sound selector;
- verify selection without damaging ordinary stereo playback;
- verify COM activation from the Windows provider path;
- verify failure leaves the previous provider state recoverable;
- keep public stream activation fail-closed until output transport is proven end to end;
- record every provider/selection mutation so uninstall and rollback can restore prior state exactly.

Gate:

> Windows can enumerate, select, activate, and safely deselect Omniphony without leaving the machine on a provider that cannot render.

---

## 3. Receive a real static Windows Spatial Audio object

Cross the boundary from internal/static test machinery to an object supplied through the actually selected Windows Spatial Audio provider path.

Preserve:

```text
static role identity
PCM
source authority
object volume
update timing
lifetime / EOS semantics
exact authored role position
```

Do not route real objects through stereo inference. Do not reconstruct metadata after another renderer has already collapsed the scene.

First gate:

> One real static object supplied above or below the listener reaches Omniphony as authored spatial truth and is rendered once to the real endpoint.

Then scale to the complete static vocabulary:

```text
horizontal: FL FR C LFE SL SR BL BR BC
upper:      TFL TFR TBL TBR
lower:      BFL BFR BBL BBR
```

Final static gate:

> The full 17-role static vocabulary survives Windows ingress, Omniphony rendering, cadence adaptation, and physical egress without role substitution, duplicate spatialization, or inferred replacement.

---

## 4. Receive dynamic XYZ objects

Add true dynamic-object capacity only when identity and continuous geometry can be preserved through the complete Windows path.

Required semantics:

```text
stable object identity
audio buffer
continuous x / y / z
volume
lifetime
motion trajectory
update timing
other authoritative host metadata
```

Do not snap dynamic objects to the static 17-role frame. Static roles and continuous objects remain parallel source representations.

Gate:

> A real moving object crosses arbitrary 3-D space while its identity, audio, position, and motion continuity survive into the final Omniphony render.

---

## 5. Prove a real spatial application or game end to end

Use a real application that submits Windows Spatial Audio objects and prove the complete product path without hooks, injection, anti-cheat-sensitive interception, or reconstruction from already-binaural stereo.

The evidence must distinguish:

```text
application produced spatial objects
→ Windows selected Omniphony
→ Omniphony received the authored objects
→ object PCM and geometry reached the renderer
→ Omniphony performed the single final binaural render
→ the physical headphone endpoint received that result
```

Test at least:

- static horizontal placement;
- elevation;
- front/back placement;
- moving dynamic sources when available;
- LFE/non-directional behavior where applicable;
- application pause/resume and stream restart;
- coexistence with ordinary non-spatial applications.

Gate:

> A real spatial title can use Omniphony as its selected Windows headphone renderer with no second virtualization pass.

---

## 6. Establish perceptual parity and superiority on equivalent authored scenes

Once real object ingress exists, compare Omniphony against established headphone spatial renderers using source-equivalent scenes rather than unequal stereo reconstructions.

Primary dimensions:

- left/right accuracy;
- front/back discrimination;
- elevation certainty;
- moving-source continuity;
- frontal externalization;
- radial distance;
- source extent;
- center solidity;
- transient localization;
- envelopment without directional smear;
- timbre;
- bass integrity;
- impact and groove;
- long-session naturalness.

Testing law:

- level-match comparisons when loudness could bias preference;
- separate localization from tonal preference;
- separate source geometry from room/externalization effects;
- preserve a winning baseline before each sound-changing experiment;
- use literature plus mature open implementations before every substantive sound change;
- physical listening remains the promotion authority.

Gate:

> Remaining deficits can be attributed to the binaural renderer itself because source geometry and source authority are controlled.

---

## 7. Complete frontal externalization, distance, and room behavior

Finish the transition from strong frontal occupancy to convincing frontal distance and acoustic depth.

Immediate work:

- physically A/B the current front-depth candidate against the accepted frontal-occupancy baseline;
- keep it only if the front moves outward without becoming wetter, softer, echoey, spectrally colored, or less precise;
- measure whether directional early-field changes alter transient localization or center stability;
- refine front-specific early-reflection timing and geometry only when listening evidence identifies a remaining deficit.

Then separate the larger perceptual problems:

### Far-field externalization

Investigate bounded combinations of:

- early-reflection structure;
- direct-to-early energy relationship;
- binaural coherence;
- direction-dependent room evidence;
- HRTF interaction;
- restrained late-field support.

Do not buy externalization with indiscriminate late reverb or synthetic width.

### Radial distance

Treat perceived distance as distinct from front/back direction and room size.

Future work should investigate:

- near-field HRTF / ILD behavior;
- direct-to-reverberant evidence;
- air absorption where physically justified;
- distance-dependent source extent;
- near-field versus far-field parameterization.

### Source extent

Keep apparent source size separate from room size and diffuseness. A larger source must not automatically become a blurrier source.

Gate:

> Frontal sources occupy stable external space with useful distance variation while direct musical structure, bass pressure, transients, center solidity, and side/rear envelopment remain intact.

---

## 8. Turn HRTF flexibility into listener personalization

Move from available HRTF mechanisms to a coherent listener-facing selection and personalization system.

Future work:

1. define objective and perceptual comparison scenes for HRTF choice;
2. support repeatable A/B selection between datasets without changing unrelated DSP;
3. determine whether a small set of generic HRTFs can cover meaningful listener variation;
4. map morphology or compact listener measurements to useful HRTF/PRTF parameters where research supports it;
5. provide a clean workflow for measured individualized SOFA data;
6. preserve a strong generic default so personalization is optional;
7. keep headphone EQ, hearing-asymmetry compensation, and HRTF choice as separate profile layers.

Gate:

> A listener can select or supply a better-fitting HRTF with measurable/localizable benefit and without turning personalization into a mandatory calibration ritual.

---

## 9. Add trustworthy already-binaural source policy

Automatic mixed-source handling must distinguish ordinary stereo from stereo that already contains a spatial headphone render.

Channel count alone is not sufficient.

Future work:

- identify provenance-bearing signals available from hosts and media containers;
- prefer explicit metadata/session provenance over waveform guessing;
- define behavior for known already-binaural media;
- validate any permitted non-spatial correction separately from spatial processing;
- investigate signal-based detection only if it reaches a confidence level that makes false double-rendering acceptably rare;
- expose a deterministic override when automatic classification is uncertain.

Target policy:

```text
ordinary stereo
→ Current inference / enhancement

known already-binaural stereo
→ spatial bypass
→ optional validated non-spatial output correction only

unknown stereo
→ conservative deterministic policy
```

Gate:

> Omniphony does not blindly apply a second HRTF render to content already rendered for headphones.

---

## 10. Productize optional head tracking

Turn renderer-level head-pose capability into a reliable optional product feature rather than a lab input.

Future work:

- define supported sensor transports and discovery;
- establish latency and update-rate budgets;
- validate recentering and coordinate conventions across devices;
- handle sensor dropout and reconnection without scene jumps;
- provide smoothing that does not create perceptible lag;
- test stationary room anchoring against head-locked music preference;
- keep tracking optional per listener and content type;
- ensure tracking never mutates source authority or authored motion.

Gate:

> Head tracking can stabilize a virtual acoustic scene under real head motion without audible discontinuities, excessive latency, or making non-tracked listening second-class.

---

## 11. Harden Omniphony for Windows as a product

Complete reliability, safety, compatibility, installation, and recovery work needed for unattended daily use.

Required areas:

### Audio lifecycle

- endpoint hotplug and DAC power cycling;
- default-device changes;
- sample-rate and format changes;
- endpoint-period variation;
- stream restart and application relaunch;
- suspend/resume;
- service restart;
- queue overflow/underrun behavior under load;
- realtime worker starvation and recovery;
- static/dynamic object lifecycle abuse cases.

### Spatial-provider lifecycle

- safe provider selection and deselection;
- transactional activation;
- rollback after failed activation;
- repair and upgrade from previous generations;
- immutable content-addressed provider generations;
- retirement of in-use COM binaries without in-place replacement;
- stale Omniphony provider-key detection without touching unrelated providers;
- active/staged/previous generation manifests with exact hashes;
- clean uninstall that restores any provider state Omniphony changed.

### Security and deployment

- minimize opaque/elevated installer behavior;
- preserve transparent service/control operations;
- investigate signing and reputation strategy;
- verify release artifacts by exact source revision and hashes;
- maintain deterministic anti-stale packaging discipline.

### Compatibility

Build an application matrix covering:

- ordinary stereo music players;
- browsers;
- communication apps;
- authored 5.1 / 7.1 / height content;
- Windows Spatial Audio applications;
- games using static objects;
- games using dynamic objects;
- already-binaural media;
- exclusive/shared/RAW modes where relevant.

Gate:

> Omniphony can remain installed as ordinary system audio software without requiring the user to nurse the audio graph.

---

## 12. Publish a stable portable spatial-scene API

After the Windows object path and source-authority semantics are physically proven, expose the renderer through a stable host-neutral API.

The public scene contract should represent:

```text
authored static channels / roles
continuous objects
Ambisonics / HOA where supplied
stereo with bounded derived support
already-binaural bypass provenance
listener pose
listener/profile selection
output-format and latency contract
```

Windows concepts must remain in the Windows adapter rather than leaking into the portable core.

Future hosts may include:

- Linux audio systems;
- macOS;
- game engines;
- XR runtimes;
- media players;
- DAWs;
- research tools.

Gate:

> A non-Windows host can feed the same source-authority semantics into the same renderer without inheriting WASAPI, WDK, registry, tray, or Windows service machinery.

---

## 13. Public-release gate

A public release should follow demonstrated product behavior rather than source completeness alone.

Minimum release evidence:

```text
real Windows Spatial Sound provider selection
+ real static-object ingress
+ real dynamic-object ingress where advertised
+ one-render physical egress proof
+ real spatial application/game proof
+ ordinary stereo and authored PCM regression safety
+ already-binaural double-render protection policy
+ installer / upgrade / rollback / uninstall safety
+ compatibility matrix
+ controlled listening validation
+ reproducible release artifact identity
```

Do not advertise capabilities whose final physical/application boundary has not been crossed.

---

## Critical path

The shortest future path is:

```text
prove closed-gate physical spatial egress
        ↓
prove provider enumeration + safe activation
        ↓
receive one real static Windows spatial object
        ↓
receive the full 17-role static vocabulary
        ↓
receive continuous dynamic XYZ objects
        ↓
prove a real spatial application/game end to end
        ↓
run controlled renderer parity tests on equivalent source scenes
        ↓
finish frontal externalization / distance behavior
        ↓
productize HRTF personalization + optional head tracking
        ↓
complete already-binaural policy and Windows hardening
        ↓
publish a stable portable scene API
        ↓
public release
```

When any step is accepted or physically proven, remove it from this roadmap and advance the first remaining item to the top.