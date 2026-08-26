# Music presentation contract

This document owns the **current stereo/music presentation obligations** for Omniphony.

It does not preserve experiment chronology, old profiles, dated listening notes, candidate parameter values, or superseded tuning. Git history owns those. Exact implementation values live in code/config/tests.

> **Give the finished recording a stronger external world without weakening the recording itself.**

## 1. The mastered recording remains authoritative

For ordinary stereo music, the finished master remains explicitly present and owns musical identity.

Protect:

- center of gravity;
- bass pressure and body;
- kick impact;
- transient ownership;
- groove and microtiming;
- vocal/instrument focus;
- dynamics;
- tonal hierarchy;
- important stereo relationships;
- authored pan/motion;
- exact musical timing.

> **OFF may collapse the world. It may not bring the rhythm section back to life.**

A presentation that gains width, height, or envelopment by damaging those invariants fails.

## 2. Stereo evidence is permission, not recovered authorship

Stereo may expose useful evidence through:

```text
L/R level and pan relation
complex mid/side structure
phase / coherence
persistence
transient behavior
directness / diffuseness
frequency region
trajectory stability
```

That evidence can justify bounded presentation support. It does not prove hidden rear, height, distance, source identity, or object metadata.

```text
stereo evidence
→ confidence / permission
→ DERIVED presentation

not

stereo evidence
→ recovered authored 3-D scene
```

Frequency is never a placement command by itself. High frequency does not mean “above”; low frequency does not mean “below.” Semantic or learned labels likewise do not directly command placement.

## 3. Current presentation topology

The living stereo architecture is conceptually:

```text
FINISHED STEREO MASTER
        │
        ├──────────────→ protected direct master
        │
        ├──────────────→ coherent foundation
        │
        └→ bounded stereo evidence
                 ↓
          DERIVED support scene
                 ↓
          Omniphony renderer
                 ↓
      directional early field
                 ↓
        restrained late field
                 ↓
master + foundation + spatial support
                 ↓
        deterministic output safety
                 ↓
             headphones
```

The exact band edges, gains, shell coefficients, HRTF preprocessing, room parameters, and safety constants are implementation state. Do not duplicate them here.

The durable topology is what matters:

- master remains structurally present;
- bass/foundation does not have to be reconstructed from the spatial branch;
- inferred support is additive and bounded;
- source/scene semantics remain separate from renderer geometry;
- early and late environmental jobs remain distinct;
- output safety must not become a content-dependent remix.

## 4. Current retained music mechanisms

The default music path may use these classes of mechanism because they satisfy current product obligations:

- coherent low-frequency/body foundation separate from spatial support;
- analysis-only stereo evidence extraction;
- a derived full-sphere support field;
- measured-HRTF binaural rendering;
- directional measured-HRTF early reflections;
- bounded transient-sensitive excitation of the existing early field while leaving the direct event untouched;
- a restrained first-order late enclosure whose directional field reaches the ears through a continuous measured-HRTF projection rather than exposing a sparse cardinal-axis HRTF lattice;
- support-side spectral management where needed to avoid duplicating common renderer coloration;
- fixed output makeup/headroom policy plus stereo-linked peak safety;
- realtime continuity guards.

These are one product presentation, not a menu of historical profiles. A replacement mechanism must retire the weaker mechanism if it wins.

The current default Windows music presentation is the protected perceptual baseline. Its accepted directional early field, restrained late field, coherent foundation, bass-specific finishing, center authority, and deterministic safety behavior stay fixed while new capabilities are developed around it. A new audible mechanism enters the default path only after controlled engineering validation and clean-route physical listening show that it preserves or improves the protected musical invariants.

The late field earns retention by improving closure and envelopment without requiring more wet energy. Once late closure is smooth and unobtrusive, further externalization should be sought first through early-field geometry and directional resolution rather than by increasing late level, decay, or diffuse duplication.

## 5. Direct, broad, diffuse, and room roles stay distinct

Keep:

```text
FrontalAnchor
DirectObject
BroadSource
DiffuseField
RoomField
```

distinct enough that one job cannot silently substitute for another.

In particular:

```text
rear direct support
≠ rear reflection
≠ diffuse rear field

source extent
≠ reverb amount

distance
≠ gain reduction alone
```

A bigger presentation is not automatically better if it converts direct musical information into haze.

## 6. Center and foundation have veto power

Center authority is an independent invariant. Width/rear expansion must not hollow, smear, or destabilize the phantom center.

Bass has several possible musical jobs:

```text
physical mass
groove anchor
melodic line
timbral color
interlock with drums / other parts
```

Do not collapse those jobs into one LFE boost or one spatial rule. Low-frequency spatial treatment must preserve timing, pressure, contour, and authored stereo motion.

## 7. Authored motion may not be frozen

A stable foundation may anchor energy, but authored pan and source motion remain part of the recording.

> **Energy may be anchored. Motion may not be frozen.**

Stereo motion that already exists in the master must survive any foundation, support, room, or safety path.

## 8. Externalization is not “more reverb”

Treat at least these as separate perceptual obligations:

```text
localization
externalization
radial distance
source extent
envelopment
timbre
motion consistency
musical fidelity
```

Directional early-field cues may support externalization. Late energy mainly supplies closure/envelopment and must remain restrained.

Do not buy front externalization with indiscriminate late reverb, synthetic width, copied wet direct material, or treble emphasis.

The relationship between direct and reflected binaural cues matters more than raw room energy.

## 9. Transients remain direct

A transient detector may modulate an already-existing environmental/support mechanism only when the direct transient remains untouched.

Valid shape:

```text
transient evidence
→ bounded temporary permission for early-field excitation

protected direct transient
→ unchanged
```

Invalid shape:

```text
transient detected
→ move / soften / replace / smear the direct event
```

Transient evidence is not instrument recognition.

## 10. Pre-authored-quality law

The presentation should feel:

```text
stable
finished
authored
coherent
```

not:

```text
live-remixed
section-reactive
wandering
showy
algorithmically restless
```

Stateful processing is allowed. Audible reinterpretation for its own sake is not.

A classifier changing its mind is not a musical event.

## 11. Uncertainty controls aggression

Use confidence to bound permission, not to declare truth.

```text
high confidence
→ more specific reversible presentation may be allowed

medium confidence
→ broader / safer / slower change

low confidence
→ preserve authoritative mix relationships
```

Prefer broad extent to unsupported precise placement. Prefer conservative fallback to theatrical motion.

## 12. Rich source truth outranks stereo inference

When authored multichannel, object, field, or continuous geometry is available, preserve it.

```text
stereo
→ bounded DERIVED support

authored bed
→ preserve supplied channels / positions

objects
→ preserve identity / geometry / timing
```

Do not flatten rich source truth to stereo and ask the stereo layer to rediscover it.

Channel layout and source semantics are stream-local. A surround game sounding beside stereo music must not change what the music is.

## 13. Optional analysis cannot own playback

Learned or heavy analysis may supply bounded advisory control only if ordinary playback remains complete without it.

```text
optional analysis available + valid
→ bounded modulation

optional analysis unavailable / late / stale / invalid
→ normal Omniphony presentation continues
```

A model, separator, cache, network service, or research stack must never become a required audio dependency unless the product contract is deliberately changed and the realtime law remains satisfied.

## 14. Bypass and comparison law

A perceptual comparison is valid only when the route is single and level-aware.

Reject conclusions contaminated by:

```text
duplicate physical paths
multiple headphone virtualizers
queued wet tails
phase/comb interference
uncontrolled loudness advantage
```

Matched-loudness bypass should ideally read as:

```text
ON  → same music, stronger world
OFF → world collapses, music remains intact
```

## 15. Promotion law

Every new artistic degree of freedom must earn itself.

```text
candidate mechanism
→ controlled engineering validation
→ clean-route physical listening
→ keep / revise / revert
```

If retained, fold the stable rule here or in a narrower living contract and encode objective invariants in tests where possible. Delete the experiment narrative. If unresolved, put only the unresolved gate in `../ROADMAP.md`.

Do not accumulate profile matrices or alternate product modes merely because several mechanisms were once compared.

## 16. Success condition

The target is not to make Omniphony seem intelligent.

The target is:

> **The music keeps its identity, weight, timing, dynamics, clarity, center, bass, and authored motion while the headphone presentation gains convincing external width, depth, height, distance, extent, and envelopment.**
