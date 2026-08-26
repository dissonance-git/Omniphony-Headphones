# Realtime game-music source contract

This document owns the current source-authority and presentation semantics for already-separated causal game-music sources supplied by Retro VGM Compiler or equivalent frontends.

It does not preserve research surveys, platform histories, ABI migration phases, old tuning constants, or source-specific experiment diaries. Exact ABI/version/layout implementation lives in code/tests. Git owns chronology.

> **The frontend reconstructs the musical source truth. Omniphony presents that truth in the larger spatial world.**

## 1. Two simultaneous obligations

```text
SOURCE TRUTH
preserve what the game / driver / chip / DSP actually did

PRESENTATION FREEDOM
use modern spatial dimensions only where the source did not author them
```

The frontend owns reconstruction truth. Omniphony owns presentation.

Do not make modern presentation decisions masquerade as historical/source authorship.

## 2. Source-object count is not scene-lane count

Recovered source topology remains itself.

Examples:

```text
YM2612       → complete audible FM channels
YM2151       → complete audible FM channels
Genesis PSG  → tone voices + noise
SNES S-DSP   → dry voices + one shared wet field where independently proven
```

Therefore:

```text
source-object count
!= canonical 8.1.4.4 lane count
!= internal render-lattice direction count
```

Do not manufacture fixed speaker-channel PCM merely to fit the scene vocabulary.

## 3. AUTHORED / DERIVED / EMPTY

```text
AUTHORED
preserved from source / driver / device / format

DERIVED
chosen by modern Omniphony presentation

EMPTY
no trustworthy source fact exists for that dimension
```

Examples of authored evidence include source identity, exact event timing, native left/right routing where hardware supplies it, shared-effect send state, and genuinely supplied spatial coordinates.

Examples of derived presentation include unauthored depth, height, rear placement, source extent, diffuse treatment, and modern room support.

## 4. NativeRouting and FullSphere are presentation policies

Both policies use the same source objects and same renderer semantics.

### `NativeRouting`

Preserve source-native route/identity while closing creative rear, height, extra depth, and derived extent not present in the source.

### `FullSphere`

Preserve the same authored source facts while allowing stable `DERIVED` azimuth/depth/elevation/distance/extent where presentation evidence permits it.

The difference is presentation freedom, not source reconstruction truth and not a hidden renderer swap.

## 5. Stable identity matters more than hardware slot

Physical chip/DSP slot is not always presentation identity.

Prefer:

```text
persistent musical part
otherwise bounded source identity
```

for spatial continuity.

If an unrelated source reuses a hardware slot, it must not inherit the previous source's pose trajectory. If a persistent part genuinely migrates between slots, continuity may survive when the frontend can prove that identity.

## 6. FM operator boundary

For ordinary FM synthesis, a complete audible channel is the default spatial source.

```text
FM operator
!= independent spatial object by default
```

Operators participate in one synthesis network through algorithms, modulation, feedback, and shared channel output.

Likewise:

```text
better whole-chip rendering
!= proven independent additive stems
```

Shared mixer/DAC/clamp/coupling paths require explicit decomposition evidence before independent enhanced lanes are admitted as exact source truth.

## 7. Dry source, shared wet, and Omniphony room are distinct

Keep:

```text
dry / localizable source
!= source-native shared effect return
!= Omniphony presentation room
```

A source-native shared return remains one shared field. Do not fabricate one wet stem per dry source merely because the dry sources are separated.

Its field center, source-native stereo relation, extent, rear bias, height, and modern presentation strength are separate concepts.

## 8. SNES S-DSP contract

Where capture proves it, preserve:

```text
8 dry S-DSP voices
+ signed per-voice L/R route
+ per-voice echo-send state
+ final shared post-EVOL echo L/R
```

The final echo is one shared stereo feedback field, not eight fabricated wet stems.

An echo-rich source may need less generic Omniphony room support because source-native wet energy already carries envelopment.

## 9. Source extent is a presentation dimension

Source center and source extent are independent.

```text
source center
+ [width, depth, height] extent
→ renderer
```

Increasing extent must redistribute presentation rather than act as an implicit gain control.

NativeRouting closes derived extent when the source provides no authored extent. FullSphere may open bounded derived extent.

The exact spread lattice, interpolation states, and coefficients are implementation details owned by code/tests.

## 10. Musical evidence constrains presentation

Useful derived evidence may include:

- foundation/foreground/support tendency;
- source density;
- energy concentration;
- low-band share;
- transient density;
- shared-effect share;
- coarse spectral overlap among dry sources.

These are presentation constraints, not source metadata.

Do not infer genre, composer, cue name, or semantic role merely to spatialize.

## 11. Spectral overlap is not masking truth

A coarse spectral-overlap measure may bound aggression when sources occupy similar broad spectral regions.

```text
more crowding
→ tighter derived extent / diffuseness
→ less extra room
```

Do not call a coarse overlap statistic psychoacoustic masking probability.

Do not use source extent as a substitute for a future explicit separation/panning mechanism.

## 12. Causality

Adaptive presentation for block/time N may depend only on source state available before N.

```text
completed past audio
→ observer update
→ bounded presentation budget
→ future audio
```

Current audio must not choose its own earlier presentation state through non-causal lookahead unless a different explicit offline/lookahead product contract is used.

Adaptive expansion should generally be slower than protective contraction so brief arrangement gaps do not make the scene pump.

Exact smoothing constants belong in code/tests.

## 13. Ordered intra-block events remain ordered

A source event occurring at a specific frame offset remains an event at that frame offset.

Derived presentation may ramp afterward for perceptual smoothness, but authored timing is not quantized to callback boundaries for convenience.

## 14. Reset and seek

Track change, seek, decoder restart, or source-generation change clears stream-lifetime state that could leak the previous source into the new one, including:

- renderer/binaural history as applicable;
- presentation identity history;
- observer/adaptation state;
- spectral-profile state;
- derived presentation budget.

A new source begins from declared neutral presentation state unless authoritative state says otherwise.

## 15. Ownership

```text
source frontend
  reconstruction truth
  exact timing
  source / part identity
  native route / send evidence
  causal source observations
        ↓
Omniphony
  presentation policy
  canonical semantic scene
  source center / extent presentation
  renderer geometry
  binaural HRTF / ITD
  distance / room / externalization
```

The frontend must not pre-render a competing spatial world.

Omniphony must not decide which emulator/source reconstruction is more truthful.

## 16. Validation obligations

Minimum live invariants include:

```text
AUTHORITY
source-native route / timing / identity survive
DERIVED geometry never becomes authored

POLICY
NativeRouting closes unauthored creative dimensions
FullSphere opens deterministic bounded presentation
policy changes do not require source reconstruction

EXTENT
extent changes presentation without silently moving source center
extent is not a volume control

SHARED WET
one source-native shared field remains one shared field
shared wet remains distinct from Omniphony room

ADAPTATION
current audio cannot choose its own prior budget
callback partitioning does not materially redefine adaptation
more crowding cannot increase aggressive diffuseness/room by accident
reset returns adaptive state to neutral
failed rendering does not advance authoritative source identity

SOURCE OBJECTS
FM operators are not spatial objects by default
whole-chip fidelity does not imply exact independent stems
```

## 17. Perceptual target

> **Different soundtracks should remain recognizably different while each gains as much stable width, depth, height, source body, and envelopment as its own arrangement can support without sacrificing impact, clarity, timbre, transients, source truth, or musical hierarchy.**
