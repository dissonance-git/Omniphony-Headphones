# Baseline 1 spectral frontier

> **Evidence status:** preserved historical spectral-frontier snapshot.
>
> This file records the measurements, hypotheses, mechanism stack, rollback ladder, and listening questions of the Baseline-1/post-baseline investigation. It does not own the current project frontier, current defaults, or current implementation state. Current unresolved work is owned by `../ROADMAP.md`; executable state is derived from code/tests/CI; later listening evidence is preserved in `listening-history.md`.

Baseline 1 was recorded as the canonical listening reference for this investigation:

```text
03dac8bb454444b47353c39f65b58ce82617d731
```

The post-baseline branch deliberately pushed beyond that reference while preserving a rollback ladder. The motivating perceptual defect was intermittent **piercing / fatigue on bright transients**, especially cymbals and already-aggressive mixes.

The working hypothesis moved beyond "the treble EQ is too high" toward a renderer-colour / coherence problem that manifested most audibly in the upper spectrum.

## Measured SAF/KEMAR diffuse fingerprint

`renderer/tests/hrtf_diffuse_spectrum.rs` measured the cos(elevation)-weighted direction-averaged power response of the interpolated embedded SAF/KEMAR HRTF grid.

Relative to 1 kHz, the recorded profile was:

```text
500 Hz     -0.57 dB
1 kHz       0.00 dB
2 kHz      +1.74 dB
3 kHz      +4.23 dB
4 kHz      +7.27 dB
5 kHz      +7.35 dB
6 kHz      +7.02 dB
8 kHz      +5.29 dB
10 kHz     +7.53 dB
12 kHz     +4.32 dB
14 kHz     +4.06 dB
16 kHz     +3.88 dB
```

Sampled span: **8.10 dB**.

This was not evidence that the KEMAR HRTF should be flattened globally. Directional pinna structure is useful localization information. It did show that broadband HRTF energy normalization and frequency-dependent diffuse-field normalization are different jobs.

In the protected-master topology, the finished stereo recording was already present full-band. The additive spatial branch could therefore impose common HRTF colour again. That made partial support-only compensation a plausible experiment without EQing or replacing the master.

## External evidence used

### MPEG-H virtual-loudspeaker binaural rendering

Hyeong-Joo Moon and Young-Cheol Park, **Quality Enhancement of MPEG-H 3DA Binaural Rendering Using a Spectral Compensation Technique** (Electronics, 2022, DOI `10.3390/electronics11091491`) reported spectral artifacts in virtual-loudspeaker binaural downmix and subjective improvement from frequency-dependent compensation.

The open `ittiam-systems/libmpegh` decoder was also used structurally because it separates direct and diffuse BRIR/filter contributions.

### Diffuse-field HRTF equalization

Thomas McKenzie, Damian Murphy and Gavin Kearney, **Diffuse-Field Equalisation of Binaural Ambisonic Rendering** (Applied Sciences, 2018, DOI `10.3390/app8101956`) was used as evidence for direction-independent diffuse-field equalization while preserving directional residual structure.

Spatial Audio Framework's `diffuseFieldEqualiseHRTFs` implementation provided an open implementation reference for weighted mean-squared HRTF magnitude normalization.

### HRTF gain normalization

Valve Steam Audio was used as independent evidence that HRTF gain management can be a renderer responsibility distinct from programme EQ.

### Coherence and transient preservation

Jonathan B. Moore and Adam J. Hill, **Dynamic Diffuse Signal Processing for Sound Reinforcement and Reproduction** (JAES, 2018, DOI `10.17743/JAES.2018.0054`) informed the caution against broad decorrelation of direct musical structure.

## Recorded post-baseline mechanism stack

### Cascaded renderer

```text
derived support
→ virtual-speaker renderer
→ virtual room
→ SAF/KEMAR HRTF + ITD
→ binaural support
```

Direct binaural remained a reference path. The cascaded path was retained in this investigation because listening found it more continuous and bubble-like.

### Larger frontier geometry

The snapshot recorded:

```text
metric scale                 7.25 m / ADM unit
speaker effect-space width   15.5 m
front reach                  13.0 m
rear reach                   10.5 m
upper reach                  14.5 m
TFL/TFR z                    1.65
TBL/TBR z                    1.50
side x                       ±1.15
source spread floor          0.09
source spread max            0.36
phantom extraction           0.28 broadband
reflection room              17 × 27 × 15.5 m
reflection level             0.38
late field                   0.035 / 0.17 s / 28 ms
```

The associated design hypothesis was that scale should come primarily from geometry, timing, HRTF/ITD, early-field structure, and source extent rather than a louder late tail.

### Reflection spectral realism

The post-baseline reflection experiment added broad high-frequency loss to reflection-only paths based on generic wall HF retention and additional propagation distance, while preserving low/mid timing and distance structure.

### High-band coherence cleanup

A correlated stereo-mid shortcut into top-front support was disabled above 5 kHz to test whether the combination of direct center transient plus correlated HRTF-rendered overhead energy contributed to harshness.

### Partial SAF diffuse-field compensation

`renderer/src/binaural/diffuse_compensation.rs` recorded this first partial inverse:

```text
4.8 kHz broad peak   -3.40 dB
10 kHz broad peak    -3.00 dB
12 kHz high shelf    -1.20 dB
```

It was deliberately partial rather than a full inverse so directional HRTF residuals could remain available for localization.

At that time, the generic cascade kept compensation off by default and the music configuration explicitly opted into the SAF-specific profile.

### Reclaimed playback level

The snapshot recorded a host fixed gain change from `0.72` to `0.90`, about +1.94 dB of whole-program level, with ON and OFF using the same static gain and no content-dependent loudness stage.

## Recorded rollback ladder

```text
03dac8bb  Baseline 1
0471501e  larger geometry + reflection HF realism
17ad1f20  fixed output level 0.72 → 0.90
e360c7fc  remove correlated >5 kHz top-front mid copy
cc186861  SAF diffuse-spectrum measurement
42fba777  partial SAF diffuse compensation implementation
7acde068  explicit SAF-only cascade compensation gating
```

This ladder is historical evidence, not a current version/status table.

## Listening questions recorded for that frontier

1. Was whole-program volume usable without nearly maxing the amplifier?
2. Was the sphere clearly larger than Baseline 1?
3. Were cymbals and bright transients less needle-like?
4. Did correction sound calmer rather than merely darker?
5. Was height retained after the high-band top-front shortcut was removed?
6. Did bass pressure, kick weight, and drum body remain unchanged?
7. Did panned percussion and tom rolls remain mobile?
8. Was any new blur, hallway colour, or late-field fog introduced?
9. Did the higher fixed gain cause clipping on dense masters?

## Reopenable next hypotheses from that investigation

If the same piercing failure mode reappears, the historical next candidates were:

1. measure the complete cascaded support transfer function rather than only isolated HRTF diffuse response;
2. derive a smoother minimum-phase/FIR or frequency-domain compensation from the measured cascade;
3. test transient-preserving decorrelation only on diffuse/reflection residue;
4. investigate frequency-dependent virtual-speaker spread;
5. only after renderer spectral stability, compare HRTF/SOFA selection or personalization.

The protected master remained outside those experiments.
