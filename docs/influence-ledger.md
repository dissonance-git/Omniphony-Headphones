# External influence ledger

> **Evidence status:** durable external-research ledger.
>
> This file preserves sources, transferred lessons, rejected transplants, and parked mechanisms. It is **not** a roadmap, implementation-status page, or product-architecture owner.
>
> - durable product identity / architecture → `../README.md`;
> - governing promotion law → `../AGENTS.md`;
> - unresolved current work / frontier → `../ROADMAP.md`;
> - executable state → code, tests, and CI;
> - physical listening evidence → `listening-history.md`.

The purpose of this ledger is to keep useful external evidence reopenable after the immediate experiment has passed.

## Promotion rule

```text
external source / influence
→ concrete mechanism or lesson
→ relevance to an observed obligation
→ bounded experiment
→ objective validation + listening where audible
→ retain / narrow / reject
```

Parking is not rejection. A reference does not become architecture merely by being interesting.

The upstream Omniphony renderer remains the spatial foundation. External work should normally improve how Omniphony feeds, preserves, validates, or selectively extends that core rather than vote to replace it.

## Renderer and binaural architecture references

### Upstream Omniphony

Sources:
- https://github.com/mgth/Omniphony
- https://omniphony.mgth.fr/
- upstream `BINAURAL.md`
- upstream bundled demo configuration

Retained lessons:
- the renderer is the inheritance; Studio is supervision/control;
- binaural output is a distinct renderer path;
- upstream owns load-bearing HRTF/HRIR, ITD, geometry, object/bed, reflection, room, head-pose, and binaural machinery;
- bridges are the intended decode/format seam;
- known spatial content is a strong fit for the upstream engine.

The upstream demo was treated as a known-spatial control, not automatically as the correct treatment for finished stereo music.

### 3D Tune-In Toolkit

Sources:
- **3D Tune-In Toolkit: An open-source library for real-time binaural spatialisation**, PLoS ONE, 2019
- https://doi.org/10.1371/journal.pone.0211899
- https://github.com/3DTune-In/3dti_AudioToolkit

Retained lesson: direct/anechoic identity-bearing rendering and reverberant/environment rendering can remain separate responsibilities. ITD handling, HRIR interpolation, transitions, and room behavior need explicit ownership.

Status: high-value architectural influence, not replacement renderer.

### Google Open Binaural Renderer

Source: https://github.com/google/obr

Retained lesson:

```text
direct
→ short / precise / identity-bearing

ambient + reverberant
→ may use longer temporal support
```

Do not copy response lengths literally; preserve the asymmetry of responsibilities.

### Valve Steam Audio

Source: https://github.com/ValveSoftware/steam-audio

Useful evidence:
- stateful per-source binaural processing;
- HRTF interpolation quality/performance tradeoffs;
- SOFA HRTFs;
- direct/reflection/late separation;
- Ambisonic diffuse/full-sphere fields;
- SIMD-aware realtime engineering;
- HRTF gain/normalization as renderer responsibility.

Boundary: reference mechanisms and tests, not a graft over Omniphony.

### Resonance Audio

Source: https://github.com/resonance-audio/resonance-audio

Useful evidence:
- per-source binaural state;
- smooth HRTF transitions;
- Ambisonic field representation;
- room simulation separable from direct rendering;
- thin host adapters around a stable engine.

Status: archived reference, not dependency target.

### Meta XR Audio SDK samples

Sources:
- https://github.com/oculus-samples/Unity-MetaXRAudioSDK
- https://github.com/oculus-samples/Unreal-MetaXRAudioSDK

Primary transfer: perceptual test taxonomy for room acoustics, source directivity, spatialization amount, and host integration.

### Cavern

Source: https://github.com/VoidXH/Cavern

Useful convergence:
- direction + distance headphone rendering;
- channel/object formats;
- surround-to-3D conversion;
- calibration/measurement;
- low-latency operation;
- listener/source abstractions.

Status: benchmark, not replacement.

### IEM Plug-in Suite

Sources:
- https://github.com/tu-studio/IEMPluginSuite
- https://plugins.iem.at/

Useful transfer: Ambisonics can be a portable representation for a **derived field** when that representation earns its complexity. Explicit encoder/field/decoder separation is valuable.

Boundary: do not insert Ambisonics merely because it is spatially elegant.

## Convolution, HRTF, and filter evidence

### Stereo Convolution DSP / 2×2 convolution matrix

Lineage:
- foobar `foo_dsp_stereoconv`
- Hydrogenaudio documentation
- multichannel FIR/convolution literature

Useful abstraction:

```text
yL = L * HLL + R * HRL
yR = L * HLR + R * HRR
```

This makes diagonal identity-bearing transfer and cross-ear support explicit. The durable insight is that the finished stereo solution can remain structurally present rather than being destroyed and recreated as two virtual speakers.

This is an architectural lesson, not a requirement that the implementation be one fixed 2×2 FIR.

### Partitioned convolution literature

Primary reference: Frank Wefers, **Partitioned convolution algorithms for real-time auralization** (2015).

Retained lesson: partitioning is an implementation tool for trading latency and compute. Short early partitions and larger late partitions are a candidate when longer responses actually earn themselves.

### HiFi-LoFi / FFTConvolver

Source: https://github.com/HiFi-LoFi/FFTConvolver

Status: parked implementation reference for realtime uniform and two-stage partitioned convolution.

### MathAudio Headphone EQ / crossfeed

Sources:
- https://mathaudio.com/headphone-eq.htm
- https://mathaudio.com/download.htm

Useful lesson: headphone correction and spatial presentation are separate responsibilities; bounded cross-ear support can matter, but strong crossfeed can narrow stereo.

Status: mechanism reference only.

### MathAudio Room EQ

Sources:
- https://mathaudio.com/room-eq.htm
- https://mathaudio.com/why-room-eq.htm

Retained law:

> **Measurable invertibility is not permission to perform the inversion.**

Deep notches, inverse conditioning, pre-ringing, and transient behavior need bounded perceptual justification.

Rejected transfer: broad anti-FIR framing. The issue is inappropriate filter design/phase/transition/latency, not FIR as a category.

### FIR phase / pre-ringing literature

References:
- Johann Gaus, **Optimization of Phase Correction for Finite Impulse Response Filters**, JAES, 2026
- Li et al., **Evaluation of headphone phase equalization on sound reproduction**, Applied Acoustics, 2019
- Korhola & Karjalainen, **Perceptual Study and Auditory Analysis on Digital Crossover Filters**, JAES, 2008

Retained validation dimensions:

```text
magnitude error
phase / group delay
pre-response
ringing
transient smear
stereo-width error
interchannel mismatch
```

## Stereo analysis and presentation references

### Trifield / Michael Gerzon lineage

Sources:
- https://www.foobar2000.org/components/view/foo_dsp_trifield
- linked Hydrogenaudio documentation

Retained law:

> **Center authority is an independent spatial invariant.**

The useful transfer is center/stage stability, not blindly creating a literal headphone center object.

### LCC — Localization Cue Correction

Sources:
- https://www.foobar2000.org/components/view/foo_dsp_lcc
- https://github.com/MeteorStudioASU/lcc

LCC targets loudspeaker crosstalk, so its actual algorithm is not a headphone transplant.

Retained law:

> **Do not let the reproduction transform destroy useful interaural cues already present in the source.**

Status: ITD/ILD cue-preservation influence only.

### FreeSurround

Sources:
- https://www.foobar2000.org/components/view/foo_dsp_fsurround
- source lineage preserved in Real3D

Useful analysis idea: per-frequency amplitude and phase differences can produce candidate spatial evidence without semantic source separation.

Important negative evidence: reconstructed speaker-bed output could flatten/collapse the desired headphone presentation.

Retained distinction:

```text
FreeSurround-style evidence extraction
→ potentially useful

FreeSurround-style reconstructed bed
→ not automatically desirable output
```

### NRSC5-Fan Real3D-Surround-Upmixer

Source: https://github.com/NRSC5-Fan/Real3D-Surround-Upmixer-

Useful transfer: FreeSurround-derived evidence can be mapped into different output topologies. More synthetic output channels do not make the inferred bed more authoritative.

### NUGEN Halo Upmix

Source: https://nugenaudio.com/haloupmix/

Commercial benchmark, not code source.

High-value constraints:
- locational-cue analysis;
- coherent expansion without mandatory reverb/chorus/delay;
- center and LF management;
- source/downmix integrity.

Retained law:

> **Spatial expansion should be evaluated for reversibility/source preservation, not only for impressiveness.**

### Penteo

Source: https://www.perfectsurround.com/

Commercial benchmark, not code source.

Useful constraints:
- phase coherence;
- source recoverability;
- center stability;
- low-frequency stability;
- no mandatory artificial room.

### Airwindows Wider

Sources:
- https://www.airwindows.com/wider-vst/
- https://github.com/airwindows/airwindows

Useful lesson: very small M/S-domain and timing changes can alter depth without reconstructing discrete objects. Subtle mechanisms can outperform aggressive width processing.

### Goodhertz CanOpener Studio

Sources:
- https://goodhertz.com/canopener-studio/
- https://manuals.goodhertz.com/3.13/canopener-studio/

Useful lesson: crossfeed amount, apparent angle, timing, and spectral modeling are separable. Crossfeed is a bounded presentation mechanism, not the whole spatial world.

### Preferred headphone response for spatial vs stereo content

Reference: Isaac Engel, D. Alon, Kevin Scheumann, Jeff Crukley, Ravish Mehra, **On the Differences in Preferred Headphone Response for Spatial and Stereo Content**, JAES, 2022.

Retained lesson: conventional stereo and authored binaural/spatial material are not necessarily the same reproduction problem.

## Room, coherence, and diffuse-field evidence

### Diffuse/coherence literature

Jonathan B. Moore and Adam J. Hill, **Dynamic Diffuse Signal Processing for Sound Reinforcement and Reproduction**, JAES, 2018, informed caution around highly coherent duplicated energy and the need to preserve transients when decorrelating.

### MPEG-H / diffuse-field compensation references

Hyeong-Joo Moon and Young-Cheol Park, **Quality Enhancement of MPEG-H 3DA Binaural Rendering Using a Spectral Compensation Technique** (Electronics, 2022) and the open `ittiam-systems/libmpegh` decoder were used as evidence that virtual-loudspeaker binaural paths can need spectral compensation and separation of direct/diffuse responsibilities.

Thomas McKenzie, Damian Murphy and Gavin Kearney, **Diffuse-Field Equalisation of Binaural Ambisonic Rendering** (Applied Sciences, 2018) plus Spatial Audio Framework's diffuse-field equalization implementation informed bounded common-response compensation experiments.

These sources support experiments; they do not authorize flattening directional HRTF structure wholesale.

## Host and platform references

### Current/legacy incumbent chain evidence

A historical reference chain included foobar processing, Hi-Fi Cable, Equalizer APO/HeSuVi, DTS Virtual:X, ASIO Bridge, FiiO, and Dan Clark Noire X.

Durable lessons:
- coherent acoustic volume matters;
- rear structure can be compelling;
- bass timing/weight and center authority matter;
- complex internals are acceptable, complex user ritual is not;
- migration should disable/replace one active function at a time before uninstalling the old tool.

This is retained historical evidence, not a current product topology.

### CamillaDSP

Source: https://github.com/HEnquist/camilladsp

Useful host pattern:

```text
capture
→ bounded handoff
→ processing
→ bounded handoff
→ playback
+ supervisor/control
```

Transferred lessons include explicit format negotiation, reconnect handling, clock management, optional resampling, realtime priority, and separation of host plumbing from DSP.

### wasapi-rs

Source: https://github.com/HEnquist/wasapi-rs

Useful host capabilities included render/capture, shared/exclusive operation, event/poll modes, application loopback, and session/device notifications.

Boundary: host plumbing only.

### HEnquist audio ecosystem

Relevant repositories include `camilladsp`, `wasapi-rs`, `audio_thread_priority`, and `audioadapter-rs`.

Retained theme: small host/infrastructure crates and explicit buffering are preferable to platform contamination of DSP semantics.

### ASIO2WASAPI

Source: https://github.com/levmin/ASIO2WASAPI

Useful only as interoperability evidence that specialist ASIO compatibility can remain a boundary concern rather than define the portable core.

### Dolby public repositories

Sources:
- https://github.com/orgs/DolbyLaboratories/repositories
- https://github.com/DolbyLaboratories/gst-home-audio

Useful architecture:

```text
parse / decode
→ object / flexible rendering
→ perceptual post-processing
→ output
```

Boundary: proprietary Dolby rendering libraries are not open implementation sources.

## Product laws distilled from the evidence

These are the durable conclusions that survived multiple independent references and Omniphony's own negative/positive evidence:

1. **Upstream renderer stays the heart.** Improve source/presentation/validation before replacing the renderer.
2. **Finished stereo remains structurally present.** Default spatial support should not delete the authored master and recreate it as guessed objects.
3. **Analysis is not rendering.** Evidence extraction does not make an inferred bed authored truth.
4. **Center authority is protected independently.**
5. **Existing interaural cues are protected.**
6. **Reversibility/downmix quality is a useful regression dimension.**
7. **Environment is optional support, not spatiality itself.**
8. **Convolution is infrastructure, not a sound signature.**
9. **Crossfeed is a bounded mechanism, not a product mode.**
10. **Bass/foundation has veto power over spatial impressiveness.**
11. **Richer source truth reduces inference.**
12. **Independent source layouts should coexist without a global channel mode.**
13. **Platform host and portable renderer remain separate.**
14. **The user interface should collapse complexity rather than expose the research graph.**

## Historical experiment retained for provenance

One early experiment derived low-level side/rear support from stereo difference evidence while keeping the original stereo master latency-aligned and authoritative. The experiment used roughly 14% support, excluded low bass from that support branch, and avoided default room processing.

Its purpose was to test this architecture:

```text
original stereo master
+ bounded derived support
→ headphones
```

rather than:

```text
stereo
→ giant inferred speaker bed
→ treat as authored truth
```

That experiment is retained because it helped establish the protected-master/source-authority direction. It is **not** the current next-build plan. Current experiments and gates belong only in `../ROADMAP.md`.
