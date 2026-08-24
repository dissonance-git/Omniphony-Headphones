# Personalized reverb component roadmap

**Status:** deferred future component. Do not implement until the current front-stage, tray-control, output-level, and Noire X enhancement work has reached a stable listening baseline.

## Intent

Build a custom music reverb for the primary listening setup whose job is not to replace Omniphony's spatial renderer, Current's own acoustic support, or the headphone EQ.

The target is the perceptual end state:

> headphones disappear; the listener's head sits inside a continuous acoustic bubble; the band feels physically present around the listener rather than reproduced inside the cups.

This component is creative ambience processing. It must remain conceptually and operationally separate from source-authority spatialization.

## Current listening reference

The useful upstream foobar2000 reference chain currently includes:

```text
Vocal Exciter
→ Reverb DSP
→ Advanced Limiter
→ Windows / Omniphony
```

Current reverb reference settings from the accepted listening setup:

- Dry Time: `90%`
- Wet Time: `15%`
- Damping: `55%`
- Room Size: `40%`
- Room Width: `100%`

Current Vocal Exciter reference:

- Amount: `0.150`

These values are not sacred. They are the listening baseline against which a future Omniphony reverb must be compared.

## Product boundary

The future component should appear as one independently bypassable effect, for example:

```text
Personalized Reverb: On / Off
```

It must not be hidden inside:

- Current stereo inference;
- the binaural HRTF renderer;
- Noire X headphone EQ;
- Noire X Enhancement;
- the final safety limiter;
- authored multichannel/object source mapping.

A/B must therefore be possible without changing any of those other systems.

## Signal-path law

Do not add a generic reverberator after final binaural rendering unless listening and measurement prove that it preserves localization. Post-binaural L/R reverberation can blur the HRTF/interaural structure that Omniphony has already built.

Preferred first architecture for ordinary stereo music:

```text
finished stereo master
→ optional personalized creative reverb
→ Current evidence / protected-master spatial presentation
→ one Omniphony binaural render
→ headphone profile / enhancement
→ output trim
→ peak guard
```

This lets the creative room become legitimate stereo evidence that Current may spatially interpret, while retaining one spatialization pass.

For authored multichannel, objects, HOA, or already-binaural material, do not automatically reuse the stereo creative-reverb path. Those source types need a separate decision because richer source truth must not be collapsed or re-spatialized merely to apply a music effect.

## Desired character

The new reverb should maximize:

1. **Externalization**: sound should detach from the cups and occupy an apparent room around the head.
2. **Strong front presence**: the room must not pull the perceptual center behind the listener. Vocals and primary musical body should still feel physically forward.
3. **Continuous wrap**: front, sides, rear, and height should connect into one space rather than separate effect zones.
4. **Early-reflection realism**: prioritize useful directional early structure over simply increasing a late diffuse tail.
5. **Large apparent space without wash**: preserve attacks, groove, vocal intelligibility, bass articulation, and center stability.
6. **Wide but not phasey**: avoid decorrelation tricks that make width impressive while weakening localization or mono compatibility.
7. **Frequency-aware decay**: low frequencies should feel massive and room-filling without becoming slow or muddy; high-frequency decay should be smooth rather than metallic or splashy.
8. **No loudness trickery**: compare at matched output level so preference is not caused merely by gain.

## Candidate architecture to investigate later

Do not lock an algorithm yet. The future research/build pass should compare mature open-source reverbs and literature before selecting the smallest sufficient substrate.

Likely useful pieces to evaluate:

```text
input
├─ direct / protected path
├─ directional early-reflection network
└─ restrained late field
       ↓
frequency-dependent damping / decay
       ↓
controlled stereo/binaural coherence
       ↓
wet mix
```

Possible implementation families to quarry rather than blindly copy:

- image-source or explicitly directional early reflections;
- high-quality FDN reverbs;
- modulated delay-network reverbs where modulation is slow enough not to damage pitch/transients;
- allpass/Schroeder structures only where they survive listening against more modern networks;
- convolution/BRIR-derived room structure where CPU, latency, and personalization justify it;
- hybrid early-convolution + algorithmic-late designs.

The research pass must include mature GitHub implementations and peer-reviewed externalization/reverberation literature before sound-changing code is accepted.

## Controls

Keep the user-facing surface small. Initial target:

```text
Personalized Reverb    On / Off
Room                   Personal
```

Only expose more controls if repeated listening shows they map to genuinely independent perceptual dimensions. Internal parameters may be richer than the tray UI.

A later advanced editor could expose a few meaningful controls such as:

- Space / apparent room scale
- Wetness
- Decay
- Damping
- Front / wrap balance

Do not expose implementation parameters merely because they exist.

## Validation

Before replacing the current foobar reverb, compare at matched loudness using the same music excerpts.

Listening questions:

- Do headphones disappear more completely?
- Does the singer remain convincingly in front?
- Does the room continue behind and above without the rear becoming the center of gravity?
- Does bass feel physically embedded in the same room rather than separately boosted?
- Are kick/snare attacks intact?
- Is vocal timbre unchanged apart from believable room interaction?
- Does the effect still work on dense masters, sparse acoustic recordings, live music, VGM, and older recordings?
- Does bypass reveal a real spatial/externalization loss rather than merely less loudness?

Measurement guardrails should include peak/RMS delta, spectrum, interaural coherence, ILD/IPD change, impulse/decay behavior, latency, CPU, allocations, and mono compatibility where relevant.

## Dependency order

Do this later, in this order:

1. stabilize stronger frontal Current presentation;
2. finish the tray control surface;
3. stabilize the `+1.5 dB` output-level option;
4. stabilize the single-switch Noire X Enhancement layer;
5. freeze a new accepted listening baseline;
6. research mature GitHub reverbs + literature;
7. prototype personalized reverb outside the production path;
8. A/B against the current foobar reverb settings;
9. only then integrate it as an independent component.

The existing foobar reverb remains valid listening infrastructure until a replacement wins physically.
