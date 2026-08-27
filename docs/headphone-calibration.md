# Listener and headphone calibration contract

Omniphony's binaural renderer does not end when an HRTF has been convolved with the scene.

The actual reproduction chain is:

```text
inferred auditory scene
        ↓
binaural rendering
        ↓
listener HRTF / BRIR assumptions
        ↓
headphone driver + enclosure
        ↓
driver ↔ pinna / ear interaction
        ↓
listener
```

A spatial renderer that ignores the last three layers can localize correctly in a mathematical model while sounding colored, inside-the-head, bass-light, sharp, blurred, or inconsistent across headphones.

This document defines the calibration boundary for the Windows product. It is influenced especially by ASH Toolset's separation of HRTF/BRIR, headphone correction, driver-to-ear interaction, room target, low-frequency integration and headroom, and by Airwave's device-specific set-and-forget product model.

The architecture must remain useful even if particular calibration algorithms change.

---

## 1. Calibration is layered, not one magic FIR

Keep these concepts distinct until a deliberate measured response collapses them:

```text
LISTENER GEOMETRY / HRTF
which directional filtering resembles this listener?

HEADPHONE RESPONSE
what coloration does this physical headphone introduce?

DRIVER ↔ EAR INTERACTION
how does the headphone couple to this listener's pinna/ear region?

ROOM / PRESENTATION TARGET
what environmental response is the renderer intentionally presenting?

LOW-FREQUENCY INTEGRATION
which parts of a measured room/HRTF response are trustworthy and desirable in bass?

SAFETY HEADROOM
how much gain margin is required to avoid clipping after all filters sum?
```

Do not hide all six inside a knob called `spatial_profile`.

A user-facing preset can be simple while the internal state remains explicit.

---

## 2. Two legitimate calibration modes

### Component calibration

The system knows separate pieces:

```text
HRTF
+
headphone correction
+
optional driver-ear correction
+
room parameters
+
headroom
```

Advantages:

- inspectable;
- independently replaceable;
- easier to diagnose coloration;
- can move one listener profile across several headphones;
- can move one headphone profile across several generic HRTFs.

### End-to-end measured BRIR

A measurement may intentionally capture a complete loudspeaker/room/listener/headphone target chain.

In that case, do not double-apply components already represented by the measurement.

The profile must declare what it contains.

Example metadata:

```text
contains_listener_hrtf = true
contains_room = true
contains_headphone_response = false
contains_driver_ear_interaction = false
contains_bulk_itd = true
```

The important law is provenance, not one preferred mode.

---

## 3. Profile model

A future device profile should resemble:

```text
ListenerProfile
  id
  hrtf_source
  hrtf_coordinate_convention
  hrtf_sample_rate
  head_radius_m
  orientation_correction
  confidence

HeadphoneProfile
  device_identity
  model_identity
  correction_filter
  correction_target
  driver_ear_filter
  low_frequency_policy
  max_positive_gain_db
  confidence

PresentationProfile
  direct_level
  early_room
  late_room
  room_target
  distance_policy

SafetyProfile
  predicted_peak_gain_db
  preamp_db
  limiter_policy = none-by-default
```

The normal listener should not need to edit these structures manually.

---

## 4. Per-device persistence

The product shell should remember calibration by physical output device.

Inspired by Airwave's per-output profile model:

```text
Windows output A
→ profile A

Windows output B
→ profile B
```

Switching headphones/DAC endpoints should restore the appropriate HRTF/headphone calibration automatically where device identity is stable enough.

A device profile should be able to choose independently:

- HRTF/listener profile;
- headphone correction profile;
- presentation profile;
- bypass state.

Do not make a user reselect an HRTF every boot.

---

## 5. HRTF source selection

Omniphony already supports several conceptual HRTF families:

- embedded measured generic HRTF;
- synthetic model;
- parametric pinna/PRTF models;
- SOFA data.

Listener/HRTF selection is a future optional feature, not part of Current baseline cleanup. The accepted reference path keeps its existing embedded measured HRTF while sound-preserving cleanup proceeds; no HRTF trial or custom measurement is required for normal playback.

When that optional feature is deliberately reopened, a short perceptual selection task should prioritize dimensions generic HRTFs often get wrong:

```text
front vs back
height / elevation
externalization
spectral naturalness
lateral precision
distance plausibility
```

The user should choose what sounds naturally located, not answer technical questions about pinna notches.

### Calibration trial rule

Do not compare HRTFs at uncontrolled loudness.

Each candidate must be level matched closely enough that louder is not mistaken for more externalized or detailed.

---

## 6. Direction-convention validation

SOFA/HRIR data can disagree in:

- azimuth zero direction;
- positive azimuth handedness;
- elevation convention;
- coordinate axes;
- source/listener orientation;
- sample-rate assumptions.

ASH Toolset's explicit direction-misalignment correction is a useful warning.

Every imported dataset should pass an automatic directional sanity fixture before becoming selectable.

Minimum tests:

```text
front
left
right
rear
above/front where coverage permits
```

Check:

- expected ITD sign;
- expected ILD sign where appropriate;
- finite/non-silent filters;
- direct-arrival timing contract;
- smooth neighborhood interpolation;
- no obvious left/right swap;
- no front axis rotated by 90/180 degrees.

If the convention cannot be determined safely, mark the dataset unresolved rather than silently guessing.

---

## 7. Headphone correction

Headphone correction is a **translation layer**, not part of auditory scene inference.

Its purpose is to reduce headphone-specific coloration so the binaural cues reach the listener more consistently.

It must be possible to bypass and measure independently.

### Required invariants

A correction profile must expose:

- target response identity;
- sample rate;
- maximum boost;
- required preamp/headroom;
- source/provenance;
- whether it is minimum phase, linear phase, IIR or mixed;
- expected latency/group delay.

### Do not over-correct

Fine high-frequency measurement notches can be placement-sensitive and listener-specific.

Correction should prefer robust broad structure over blindly inverting every narrow measurement feature.

A spatial product should not trade localization stability for brittle EQ precision.

---

## 8. Driver-to-ear interaction

The physical headphone modifies the acoustic load around the outer ear.

This means:

```text
free-field HRTF
+
headphone frequency-response correction
```

is not guaranteed to reproduce the same transfer function the HRTF measurement assumed.

A future driver-ear correction layer may compensate systematic coupling effects separately from general headphone tonal EQ.

This is an experimental lane, not yet a hard requirement for the first build.

Validation question:

> Does this layer improve externalization/front-back/elevation without introducing more spectral unnaturalness than it removes?

---

## 9. Bass integration

Room/HRTF measurements can have poor or undesirable low-frequency behavior because of:

- measurement noise;
- limited loudspeaker extension;
- room modes;
- normalization choices;
- headphone correction interaction.

Low-frequency integration should therefore be explicit.

Possible policy:

```text
very low frequencies
→ clean headphone/base target

transition region
→ smooth crossfade

higher frequencies
→ full direction/room-specific binaural response
```

This is philosophically aligned with the renderer's existing bass law:

> do not buy spatial dimension by destabilizing the groove floor.

The crossover must be smooth in magnitude and phase and should be validated for transient/bass timing.

---

## 10. Headroom and gain prediction

Spatial convolution + room response + headphone correction can create positive gain even when each stage seems individually safe.

Before realtime playback, calculate a conservative peak-gain estimate where practical.

```text
combined filter / render configuration
→ predicted peak gain
→ required preamp margin
```

The default safety strategy should be **prevention**, not a hidden limiter.

A limiter changes transients and dynamics and therefore conflicts with the fidelity contract unless explicitly justified.

Recommended state:

```text
predicted_peak_gain_db
preamp_db
overload_margin_db
```

Expose clipping risk in diagnostics.

---

## 11. Calibration versus presentation

Do not let calibration absorb creative spatial policy.

```text
CALIBRATION
makes the reproduction chain more faithful / consistent

PRESENTATION
chooses how the inferred scene is spatially expressed
```

Examples:

- headphone EQ belongs to calibration;
- listener HRTF choice belongs to calibration;
- whether a secondary stable object is placed rear-lateral belongs to presentation;
- early-room strength belongs primarily to presentation;
- correcting an HRTF's reversed azimuth belongs to calibration.

This separation is necessary for meaningful A/B tests.

---

## 12. Realtime architecture

Rich calibration belongs on the control plane.

```text
OFFLINE / CONTROL THREAD
import SOFA
validate coordinates
resample filters
construct correction
estimate gain/headroom
build convolution partitions
publish immutable profile

REALTIME AUDIO THREAD
read immutable active profile
apply bounded stateful DSP
no file I/O
no SOFA parsing
no optimizer
no large allocation
```

Profile changes should use the same principle as the current asynchronous HRIR rebuild path:

```text
build away from audio thread
→ tag result with request identity
→ atomically publish only if still current
→ crossfade audible state safely
```

---

## 13. Convolution strategy

The current short HRIR path does not automatically need FFT convolution.

For long BRIR/headphone filters, benchmark at least:

```text
direct FIR
uniform partitioned FFT
head/tail two-stage FFT
```

The `neodsp/fft-convolver` architecture suggests a useful candidate:

```text
short early head
→ small partition / low latency

long room/filter tail
→ larger partitions / lower CPU
```

The implementation must remain independent of host callback block size.

Choose from measurements, not fashion.

---

## 14. Required calibration fixtures

### Filter identity

With all correction layers disabled:

```text
input
→ calibration stage
→ output
```

must null to numerical tolerance.

### Sample-rate round trip

For supported rates, resampling/import must preserve documented timing and frequency behavior.

### Headroom

Known worst-case impulses and correlated signals must not exceed the predicted peak bound beyond tolerance.

### Direction sanity

Imported HRTFs must pass left/right/front/back convention tests.

### Interpolation continuity

Moving through neighboring HRTF samples must not produce filter jumps/clicks.

### Profile switch

Switching listener/headphone profiles while audio runs must not install stale state or create a discontinuity.

### Block-size invariance

Equivalent continuous audio partitioned into different callback sizes must produce equivalent calibrated output apart from explicitly documented latency buffering.

---

## 15. Product phases

### Phase C0 — explicit generic baseline

- generic measured HRTF;
- no headphone EQ by default;
- clear headroom measurement;
- reproducible binaural baseline.

### Phase C1 — headphone profile

- import/select headphone correction;
- persistent per-device selection;
- automatic safety preamp;
- bypassable independent A/B.

### Phase C2 — listener/HRTF selection (future optional)

- small perceptual HRTF candidate test;
- SOFA import;
- automatic coordinate sanity validation;
- per-device/listener persistence.

### Phase C3 — driver-ear and bass integration research

- controlled driver-ear correction experiments;
- low-frequency response integration;
- externalization/localization versus timbral-naturalness evaluation.

### Phase C4 — measured end-to-end profiles

- optional BRIR/full-chain measurements;
- component provenance metadata;
- long-IR convolution optimization;
- automatic conflict prevention so represented stages are not applied twice.

---

## 16. Acceptance law

Calibration succeeds when better transducers allow Omniphony's spatial and musical information to scale upward rather than exposing more artifacts.

That means the desired relationship is:

```text
better headphone / DAC chain
→ clearer access to the same scene
→ stronger localization / externalization where the renderer supports it
→ more preserved microdetail and dynamics

NOT

better headphone
→ more obvious DSP coloration / phase smear / reverb artifacts
```

The calibration architecture exists to make that scaling possible.
