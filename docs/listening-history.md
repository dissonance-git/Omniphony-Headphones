# Listening history

This file preserves physical listening evidence from retired Omniphony comparison paths and from audible mechanisms as they enter or leave the single Current model.

These are **historical research controls**, not user-facing listening modes. The tray and launcher no longer expose a profile selector. Normal Windows playback uses one **Current model**.

## 2026-08-12 · profile comparison

The first tray-profile comparison produced a strong compression result:

- the non-PRTF variants were not clearly distinguishable in ordinary listening;
- the music still sounded good across those variants;
- the structural PRTF path was clearly distinguishable, but in the wrong direction: it sounded **tinnier and worse**;
- the hybrid direct-height path did not produce an obvious perceptual gain and therefore did not justify its added routing complexity.

This result did not prove that the underlying mechanisms are perceptually identical in general. It established that, under the tested system and listening conditions, those profile-level differences did not earn separate product modes.

## 2026-08-12 · measured-HRTF early reflections

A later build replaced the lightweight analytic first-order reflection panner with a bounded six-bus measured-HRTF early field while preserving the protected master and the rest of the successful music path.

Listening described this path as **a little better**, while explicitly noting that placebo could not be excluded.

The project therefore records the result conservatively:

> slight subjective preference; not a demonstrated perceptual law.

The path was adopted as the **Current model** because:

- it was not heard as worse;
- it represents a materially different and more physically meaningful early-field mechanism;
- its reflection energy is approximately power-matched rather than simply louder;
- it preserves the existing protected-master architecture;
- carrying many weakly distinguished product profiles no longer helps development.

The former tray label `Externalization` is retired. The mechanism is now simply part of the Current model.

## 2026-08-12 · transient-aware early-room excitation

The next build added lane-local transient evidence before the measured-HRTF early-reflection delay bank. Fast versus slow energy envelopes briefly increased only early-room excitation, with a +2.5 dB ceiling, while leaving the protected master, coherent foundation, primary support render and late room unchanged.

Physical listening reported that the sound was **better incrementally again**. No pumping, attack damage, bass loss or new fatigue was reported in that pass.

The result is therefore promoted into Current model as an application-specific retained mechanism:

> sharp musical events may briefly excite the already-existing early room more strongly, provided the direct event remains untouched.

This does **not** establish that the transient detector identifies drums or instruments. It only establishes that, in the tested listening system, the bounded transient-dependent early-room behavior improved the experience enough to retain.

## 2026-08-12 · front / center refinement candidate

The next listening candidate responds to a more specific observation:

- center/vocals can be a little less reverberant and slightly clearer;
- stereo-front material can move farther outward;
- front height can expand somewhat;
- side, rear and lower presentation already sound good and should stay fixed;
- bass and power are already right;
- total playback level can come down slightly.

The candidate therefore changes only:

```text
front L/R x position       +/-1.00 -> +/-1.15
top-front x position       +/-0.96 -> +/-1.10
top-front z position          2.15 -> 2.45
late-room level              0.020 -> 0.016
late-room RT60                0.14 -> 0.12 s
final fixed makeup          +3.5 dB -> +2.8 dB
```

Side, rear and lower evidence-source poses are unchanged. Bass/foundation tuning is unchanged. The retained transient-aware measured-HRTF early field is unchanged.

This candidate is **not yet a listening result**. It remains provisional until physical listening.

## 2026-08-23 · deep-sub pressure candidate

A later observation reopens only the low-frequency balance:

- deep bass should feel more bottomless;
- additional 80-240 Hz weight would be counterproductive if it reads as mud;
- proven kick, body, stereo motion, room behavior and spatial geometry should remain protected.

The candidate shifts existing low-frequency ownership downward rather than adding
a parallel bass path:

```text
renderer foundation shelf   85 Hz / +2.8 dB -> 60 Hz / +3.4 dB
renderer 110 Hz punch       unchanged at +1.6 dB
renderer 240 Hz body        unchanged at +1.2 dB
Noire X 32 Hz shelf         +4.0 dB -> +5.5 dB
Noire X profile preamp      -2.5 dB -> -4.0 dB
```

The renderer shift is calculated to add roughly 0.5 dB at 20-25 Hz while
reducing roughly 0.5-0.7 dB around 80-110 Hz. The Noire X change adds a further
bounded deep-sub preference tilt while reserving filter headroom. This is a
candidate, not a retained listening result; loudness-matched physical listening
and peak-guard activity remain required before promotion.

## Current model inherited from the comparison

The Current model retains:

- protected finished stereo master;
- coherent low-frequency/body foundation;
- analysis-only stereo evidence extraction;
- derived 7.1.4 support field;
- coherent elevation transfer;
- grid-aligned full-sphere shell;
- measured SAF/KEMAR binaural rendering;
- measured-HRTF six-bus first-order early field;
- lane-local transient-aware early-room excitation;
- short low-level late closure;
- support-only spectral compensation;
- fixed output makeup and stereo-linked peak safety;
- Windows realtime continuity guards.

Its first-order early field uses:

```text
support lanes
    ↓
lane-local transient evidence
    ↓
first-order image timing / wall filtering
    ↓
six wall-grouped buses
    ↓
measured SAF/KEMAR HRTF + ITD
    ↓
linear support sum
```

The primary engine's older analytic reflection bank is disabled on this path so the same early energy is not routed twice.

## Retired controls

### `control`

Earlier cascaded-binaural reference topology. Useful only for historical comparison.

### `all`

Previous Current model before measured-HRTF wall-bus reflections were promoted.

This path established much of the current successful sound but no longer represents normal playback.

### `hybrid`

Split the four height evidence lanes into a direct measured-HRTF path while leaving the surrounding eight lanes in the cascaded world.

Mechanical tests established exclusive routing and aligned first arrivals, but physical listening did not reveal a clear benefit over the then-current model. The extra runtime complexity was not promoted.

### `direct`

Rendered all evidence lanes through direct HRTF instead of the virtual-speaker cascade.

No clear listening advantage was established in the profile pass.

### `external`

The name was used twice during research.

The earlier version was a room-balance control and did not earn retention.

The later version introduced the measured-HRTF six-bus early field. That **mechanism** is now promoted into the Current model, but `external` is no longer a product/profile concept.

### `prtf`

Structural PRTF alternative to the measured KEMAR path.

Physical listening described it as **tinnier and worse**. This is a retained negative result: a different or more structural pinna model does not automatically improve elevation or externalization.

### `close`

Shorter-distance / smaller-room control. No clear listening benefit was established in the profile pass.

### `tracked`

Head-tracking-ready configuration. Without live head-motion input this was never a valid head-motion comparison, so no perceptual conclusion about world locking follows from the static profile test.

### `diffuse`

Deliberately stronger diffuse late-field control. It did not earn a separate listening mode.

## Promotion rule going forward

The project no longer keeps a broad tray matrix of speculative modes.

New audible mechanisms should normally enter as bounded research challengers and then either:

```text
beat / clearly improve the Current model
→ promote into Current model

fail to improve it
→ retain only the useful negative evidence

remain ambiguous
→ do not multiply product modes
```

Current unresolved perceptual work and frontier are owned by `ROADMAP.md`. This file preserves the listening evidence that can support or reopen those decisions.
