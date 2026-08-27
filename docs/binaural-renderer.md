# Binaural renderer contract

Omniphony's headphone path is an independent render path, not a speaker downmix.

For a spatialized source, the current conceptual chain is:

```text
scene position
→ listener/head-relative direction
→ interpolated HRTF/HRIR
→ analytic per-ear ITD
→ stateful convolution
→ optional directional early reflections
→ optional late room field
→ stereo L/R
```

The target is not merely correct left/right panning. It is a stable externalized auditory world that can support front, rear, lateral, height, depth, broad-source and field impressions without sacrificing the recording's fidelity.

This document owns the current portable binaural-renderer behavior and invariants. Product identity stays in `../README.md`; source/scene semantics stay in `scene-renderer-contract.md`; listener calibration stays in `headphone-calibration.md`.

---

## 1. Direct path

For each spatialized input channel, Omniphony derives listener-relative:

```text
azimuth
elevation
distance
```

The direct path then uses:

```text
interpolated per-ear HRIR
+
analytic per-ear ITD
+
continuous convolution state
```

### Authored level is preserved

The direct binaural path does **not** apply a generic inverse-distance `1/d` attenuation law to object level.

That is deliberate.

For authored/spatial material, source/object gain is already meaningful state. Distance is used to drive **distance cues**, not to silently rewrite the mix's direct level.

Current distance-related cues include:

- air absorption;
- room/reflection geometry;
- late-field send relationship.

This invariant has a regression test: with room, reverb and air absorption disabled, the same front source at different distances must retain the same direct broadband level.

---

## 2. HRTF / HRIR providers

Current conceptual provider families include:

- embedded measured SAF KEMAR;
- synthetic analytic baseline;
- parametric pinna model;
- PRTF structural model;
- optional SOFA data.

The provider is a rendering/calibration input. It does not define auditory-object identity.

### Direction interpolation

`HrirSet` interpolates across directional samples rather than snapping a moving source between isolated measurements.

The important acceptance property is smooth filter evolution with direction.

For irregular future HRTF datasets, native triangulated-sphere interpolation remains a future research option. It is not part of Current baseline cleanup; because it can change front/back/elevation spectral cues, it must be treated as a separately listening-gated sound change before replacing the accepted path.

---

## 3. Bulk arrival timing versus HRTF spectral phase

Omniphony separates:

```text
bulk / direct-arrival interaural delay
```

from

```text
direction-dependent HRTF spectral phase
```

This distinction is load-bearing.

Measured left/right HRIRs can retain different spectral/group-delay structure even after the first meaningful arrival is aligned. A non-zero left/right cross-correlation lag is therefore **not automatically residual bulk ITD**.

The measured-HRIR validation contract is:

> direct arrivals must be aligned closely enough that Omniphony can add analytic ITD separately without double-counting bulk propagation delay.

It is **not**:

> force two spectrally different ear filters to have a zero cross-correlation lag.

The active validation test checks direct-arrival alignment rather than flattening legitimate HRTF phase structure.

---

## 4. Analytic ITD

The direct path uses an analytic head model to derive left/right delay from source direction and effective head radius.

The model is validated end-to-end using a symmetric synthetic HRTF provider so measured HRTF asymmetry cannot masquerade as an engine ITD bug.

Current tests cover:

- centre source ≈ zero ITD;
- antisymmetry around the median plane;
- growing absolute ITD toward the interaural axis;
- approximate agreement with the analytic model.

ITD validation should remain independent from HRTF-choice validation.

---

## 5. Moving HRTFs

Changing source direction changes the HRIR kernel.

A geometrically smooth trajectory can still click, buzz or comb if filter state changes discontinuously.

The current `EarConvolver` therefore retains signal history and crossfades old/new filter outputs during kernel changes rather than simply dropping in a new FIR.

Mid-transition retargeting is also handled as state rather than forcing a discontinuous restart.

This property must survive any future move to FFT/partitioned convolution.

---

## 6. Asynchronous HRTF switching

Building or loading a new HRTF grid may allocate, resample, parse SOFA data or otherwise perform control-plane work that does not belong on the realtime audio thread.

Current design:

```text
user/control request
→ background HRTF build
→ source-tagged completed grid
→ audio thread accepts only if tag still matches latest request
→ atomic active-grid swap
```

The source tag matters because a slow obsolete build can finish after a newer request.

Without request identity:

```text
request B
request C
B finishes late
→ B could incorrectly become active
```

The current renderer rejects that stale completion.

Profile/calibration switching should reuse the same law.

---

## 7. Early room

Omniphony has two bounded early-room uses.

For ordinary known-scene/object rendering, the generic room path uses six first-order shoebox image sources. Each reflection carries relative geometric propagation delay, directional per-ear ITD, and broadband interaural level difference. The common propagation delay is relative to the direct path, so the early room does not add blanket latency to the direct object.

The protected stereo-music presentation uses a richer but still bounded measured-HRTF early field. First-order support reflections are routed through a fixed clustered set of measured-HRTF direction buses. Front and top-front lanes additionally redistribute a bounded share of their existing early tap power into physically derived second-order shoebox image paths.

Those promoted order-2 paths keep their actual image directions through four dedicated measured-HRTF precision buses instead of collapsing onto the final-wall direction. Their delay, wall tone, and distance loss remain path-specific. The total front early tap-power budget is conserved, so stronger frontal externalization is not purchased by adding room energy.

Below 300 Hz, the music early field preserves reflection timing/envelope while collapsing directional ITD into a coherent return. Above that boundary, measured HRTF structure carries the directional spectral information. The rear, first-order field, late enclosure, direct master, and coherent foundation remain separate owners.

A literal full HRTF convolution for every image of every support source is still intentionally avoided. Bounded clustering is the realtime compromise; any future replacement must preserve the accepted frontal boundary and musical invariants before it can replace this baseline.

---

## 8. Late room field

Late reverberation is represented separately from direct objects and early reflections.

Current FDN properties include:

- eight delay lines;
- mutually orthogonal / zero-sum ear output patterns;
- high-frequency damping in the feedback path;
- slow mutually detuned delay modulation;
- low-frequency interaural coherence shaping;
- higher-frequency decorrelated returns;
- adjustable RT60;
- adjustable predelay;
- distance-related send behavior.

This is a **presentation room field**.

It is not the same thing as a musical `DiffuseField` inferred from the source recording.

```text
room field
≠ diffuse musical content
```

The latter still needs a first-class spherical/extended direct-render representation.

---

## 9. Sample-time invariance

Host callback boundaries are transport artifacts, not acoustic events.

The same continuous signal should not produce a different room merely because one backend calls the renderer with 40 samples and another with 1024.

The FDN therefore carries its modulation scheduler across `process_block` boundaries.

Its modulation target horizon is measured in processed samples rather than “once per caller block.”

A regression test renders the same signal with several block partitions and requires identical output.

### Zero predelay

`predelay_ms = 0` is also a true zero-delay state.

The older implementation silently clamped zero to one sample. That behavior was removed.

---

## 10. Air absorption

When enabled, distance can drive a high-frequency rolloff representing propagation loss in air.

This is a cue layer, not a direct-level law.

It should remain independently bypassable so distance perception and coloration can be tested separately.

---

## 11. Non-spatialized low-frequency/direct channels

Inherited authored scenes can mark a channel as direct/non-spatialized, commonly for LFE behavior.

That path bypasses HRTF/ITD/room spatialization and feeds the ears symmetrically at constant power.

For the future stereo-music inference path, low-frequency handling should evolve from a channel-format exception into the more general scene law already emerging in `scene_inference`:

```text
low-frequency content receives strong protection from aggressive reassignment
```

but

```text
low frequency alone does not prove one coherent frontal object
```

Diffuse low-frequency energy and coherent groove/foundation evidence must stay distinguishable.

---

## 12. Current missing binaural bridges

### Sample-accurate gain trajectory

The inherited `ChannelState` can generate sample-accurate gain ramps, but the current binaural handoff still effectively applies too much state at block granularity.

Fix requirement:

```text
one authoritative ChannelState gain trajectory
→ consumed per sample by binaural path
```

Do not create a second independent binaural gain state machine.

### Sample-accurate position trajectory

The same issue exists for object motion: the current binaural path receives a block-level position state and then smooths HRTF changes.

Filter crossfading prevents discontinuities, but it does not substitute for the authored/source trajectory itself.

The next hot-path refactor should carry the actual position ramp into binaural processing.

### Broad-source extent

The inherited scene model already contains object size/extent, but the headphone path currently collapses too much of that state to a point.

`BroadSource` therefore needs a binaural rendering strategy that consumes the existing extent state rather than inventing a parallel width knob.

### Musical diffuse field

The room FDN is not the direct renderer for `DiffuseField` content.

A spherical field basis, likely Ambisonic or experimentally equivalent, is a candidate internal representation.

---

## 13. Listener/headphone calibration

HRTF selection is only one layer of reproduction calibration.

The mature architecture should keep separate:

```text
listener HRTF
headphone response
driver ↔ ear interaction
room / BRIR target
low-frequency integration
safety headroom
```

See [`headphone-calibration.md`](headphone-calibration.md).

A better headphone should expose more of the scene and recording, not more DSP artifacts.

---

## 14. Convolution strategy

The current short direct HRIR path does not automatically benefit from FFT convolution.

For longer BRIRs, headphone filters or more complex reflection responses, benchmark:

```text
direct FIR
uniform partitioned FFT
head/tail two-stage FFT
```

A useful candidate architecture for long responses is:

```text
short perceptually critical head
→ tiny low-latency partitions

long room/filter tail
→ larger efficient partitions
```

Any replacement must preserve:

- realtime safety;
- arbitrary host-buffer handling;
- reset/discontinuity semantics;
- moving-filter continuity;
- sample-time invariance.

---

## 15. Validation lanes

### Known scene → binaural

Use deterministic authored positions to measure renderer behavior without stereo-inference uncertainty.

Key tests:

- ITD sign/magnitude/antisymmetry;
- HRTF interpolation continuity;
- moving-filter continuity;
- direct-level distance invariance;
- early-reflection delay/ear timing;
- FDN decay/coherence;
- block-size invariance;
- clipping/headroom;
- spectral coloration;
- front/back/elevation listening.

### Stereo scene → binaural

Only after the known-scene renderer is trustworthy should scene-inference errors be mixed into end-to-end listening judgments.

---

## 16. Realtime rule

The audio thread must not become the calibration thread.

Keep off the realtime path:

- SOFA file I/O;
- large HRTF/grid construction;
- headphone-profile optimization;
- corpus/model inference that is not explicitly budgeted for realtime;
- large allocation;
- unbounded logging;
- blocking locks.

Build state elsewhere and publish bounded immutable/realtime-safe state to the renderer.

---

## 17. Product acceptance rule

The binaural renderer passes when it can make the headphones feel less like the apparent acoustic source **without** making bypass restore:

- clarity;
- transient precision;
- bass definition;
- timbral naturalness;
- dynamics;
- musical hierarchy.

Externalization bought with smear is not externalization worth shipping.
