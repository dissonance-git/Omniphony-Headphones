# Binaural renderer contract

Omniphony's headphone path is an independent render path, not a speaker downmix.

For a spatialized source, the current conceptual chain is:

```text
scene position
→ listener/head-relative direction
→ interpolated HRTF/HRIR
→ direction- or metric-source-appropriate per-ear ITD
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
direction-only or finite-point-source per-ear ITD
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
- late-field send relationship;
- for lanes whose source contract declares listener-relative metric XYZ, ear-specific acoustic parallax, finite-point-source interaural arrival timing, plus a bounded distance-dependent interaural level relationship.

Authored metric XYZ is already in metres and therefore bypasses the renderer's generic scene `unit_scale_m`. Inferred/presentation coordinates keep using that scene scale. For authored metric near-field objects, interaural arrival timing can add a finite-point-source rigid-sphere ray correction: a visible ear receives the straight source-to-ear path, while an occluded ear receives the tangent path plus the shortest surface arc. The renderer subtracts that ray model's own far-field limit and adds only the remaining distance-dependent term to Omniphony's established direction-only ITD. Only the left/right path difference is rendered, so absolute source distance does not add transport latency; at long range the correction vanishes and Current's existing azimuth/elevation behavior is recovered exactly. Near-field ear level is derived only from the left/right propagation-distance ratio and is equal-power normalized, so it can strengthen the near-ear cue without introducing a hidden overall `1/d` source attenuation. The extra ratio is bounded and converges smoothly to unity with distance. Omniphony does not invent a near-field spectral equalizer when the selected HRTF set does not contain range-indexed measurements.

This invariant has regression coverage: with room, reverb and air absorption disabled, symmetric front sources at different distances retain the same direct broadband level, and authored metric coordinates remain unchanged when the generic scene scale changes.

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

## 4. Direction-only and finite-source ITD

The direct path keeps two geometrically compatible ITD projections under one timing owner.

Ordinary and inferred positions use the established direction-only plane-wave Woodworth model from source direction and effective head radius. Authored metric near-field positions may add the finite-distance residual from a rigid-sphere point-source ray model, because their listener-relative XYZ supplies the source distance needed to decide whether each ear is directly visible or reached by a tangent-plus-surface path.

The finite-source route is not a second HRTF or a near-field spectral model. It changes only bulk interaural arrival timing. The full 3-D ray model is evaluated both at the real finite distance and at its far-field limit; only their difference is added to Current's existing ITD. This preserves the established far-field elevation behavior instead of silently replacing it with a different 3-D approximation, returns no common propagation delay, and makes the candidate collapse exactly back to Current as distance grows. It remains independently switchable from near-field HRTF parallax and ILD, and is off by default until engineering validation plus clean-route physical listening earn promotion.

The model is validated end-to-end using a symmetric synthetic HRTF provider so measured HRTF asymmetry cannot masquerade as an engine ITD bug.

Current tests cover:

- centre source ≈ zero ITD;
- antisymmetry around the median plane;
- growing absolute ITD toward the interaural axis;
- approximate agreement with the direction-only analytic model;
- finite-point-source correction convergence to the established Current model at long range across azimuth and elevation;
- stronger lateral ITD for a source within reach;
- 3-D median-plane symmetry and invalid inside-head geometry.

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

The same continuous signal should not produce different acoustic or control trajectories merely because one backend calls the renderer with 40 samples and another with 1024.

### Late-room state

The FDN carries its modulation scheduler across `process_block` boundaries. Its modulation target horizon is measured in processed samples rather than “once per caller block.”

A regression test renders the same signal with several block partitions and requires identical output.

### Metadata and mute gain

`ChannelState::slew_gain` is the single authority for metadata/mute slew state and rate. It advances at a constant sample rate, not a per-callback rate, and exposes the block's start/rate/end segment. If the target is reached partway through a callback, the remainder of that callback holds the target instead of dilating the final gain change to the block boundary.

The direct binaural handoff carries that derived segment into the headphone renderer; it does not create a second gain state machine. A production-boundary regression uses the transparent direct/LFE headphone route to render one continuous fade under radically different callback partitions, including a callback that straddles the exact slew endpoint, and requires the same sample trajectory.

### Authored source motion

Authored metric objects reuse the canonical `ChannelRampState`; the binaural path does not own a second motion state machine.

For a timed position update, the spatial renderer projects the same ramp to the source-time boundary reached by the current processing pass and forwards both that metric endpoint and the number of authored interpolation samples consumed. The binaural stage then uses that duration for its distance/direction transition:

- near-field interaural level cues follow the half-open authored span and preserve equal power;
- ITD delay targets use the authored motion duration rather than callback duration;
- air-absorption and late-send targets follow the same radial span;
- HRIR changes crossfade toward the same-pass endpoint, with the existing bounded HRIR-length cap preserving realtime cost;
- a zero-length authored interpolation remains an explicit jump;
- no source-position update leaves ordinary head-motion smoothing behavior unchanged.

This removes the former one-callback lag in the direct authored-object path while keeping callback partitioning a transport concern. The exact high-resolution HRTF interpolation strategy may evolve, but it must continue consuming this same source-time trajectory rather than inventing another motion clock.

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

## 12. Remaining binaural bridges

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
