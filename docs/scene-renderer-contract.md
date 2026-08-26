# Source, scene, channel/object, and renderer contract

This document owns the current semantic boundary between **source truth**, **presentation state**, **fixed channels**, **dynamic objects**, and **binaural rendering**.

It does not preserve migration phases, old incidents, implementation inventories, or superseded ABI plans. Code and tests own executable structure; Git owns chronology.

> **Rendering may transform a scene. It may not rewrite uncertainty into fake authorship.**

## 1. Three layers stay separate

```text
SOURCE TRUTH / EVIDENCE
what the source or host actually supplied
        ↓
PRESENTATION STATE
what additional DERIVED support is defensible
        ↓
RENDERING
how that state reaches two ears
```

A failure in one layer must not be hidden by another.

Examples:

- stereo can justify bounded support without containing literal rear metadata;
- an authored 7.1 bed remains authored 7.1 truth rather than being flattened and reconstructed;
- an already-binaural render does not receive a second HRTF stage simply because it is two channels.

## 2. Canonical static scene

The fixed semantic vocabulary is **8.1.4.4 / 17 anchors**:

```text
FL FR C LFE SL SR BL BR BC
TFL TFR TBL TBR
BFL BFR BBL BBR
```

Every anchor carries one provenance state:

```text
AUTHORED
DERIVED
EMPTY
```

The vocabulary is richer than many inputs. Empty positions remain empty unless a presentation mechanism deliberately creates bounded derived support.

Dynamic objects live beside the static frame and retain continuous geometry.

## 3. Fixed channels and dynamic objects are different source facts

A fixed channel is PCM with stable spatial meaning. A dynamic object is PCM whose position or other object state changes through source metadata.

```text
fixed channel
→ stable semantic label / role

dynamic object
→ stable object identity
+ PCM association
+ timed geometry / gain / extent / lifetime state
```

Do not fabricate object metadata for a fixed channel merely to enter an object renderer.

Do not demote a dynamic object to a fixed speaker lane merely to simplify routing.

A stream may contain fixed channels, dynamic objects, or both.

## 4. There is no global “spatial mode”

Object presence is a live source fact, not a rendering mode that reinterprets unrelated material.

```text
has dynamic objects
= observable property of this stream now

not

global product mode
```

Channel layout, object presence, and source semantics are stream-local. One stream changing representation must not silently rewrite another stream.

## 5. One label vocabulary

A fixed channel's semantic label is the canonical identity used across bridges, hosts, scene mapping, diagnostics, and UI.

Do not maintain parallel integer-id vocabularies, reverse-lookup tables, or format-specific aliases as separate truth when one canonical label can own the meaning.

Aliases may exist only as input parsing views over the canonical label table.

LFE remains semantically distinct from directional channels.

## 6. One render decision model

The source describes; the renderer decides.

For every fixed channel, the renderer may choose a declared presentation treatment such as:

```text
direct / one-hot where semantically appropriate
virtualized at a placement pose
host passthrough where a host contract permits it
```

For dynamic objects, supplied geometry remains authoritative.

A bridge/decoder must not pre-spatialize, invent positions, or assume an output layout to force a particular renderer path.

The renderer should build bounded cached routing state from declarations rather than re-deriving static mappings per audio frame.

## 7. Declaration state and timed state are different

Slow-changing declaration state may include:

```text
channel labels
object identities
PCM channel ↔ object associations
object names
source format semantics
```

Timed state may include:

```text
object position
object gain
object extent
fixed-channel gain automation where the source actually supplies it
lifetime / EOS
```

Declarations are cached and replaced on declared changes/reset. Timed events retain source timing and are not quantized merely for callback convenience.

## 8. Rich source truth outranks inference

```text
stereo
→ protected master + bounded DERIVED support

5.1 / 7.1 / height bed
→ matching AUTHORED fixed channels

static spatial roles
→ AUTHORED fixed roles

dynamic objects
→ continuous AUTHORED object geometry

Ambisonics / HOA
→ preserve the supplied field until the appropriate render boundary
```

Wrong:

```text
rich source
→ flatten to stereo
→ infer geometry that was already known
```

Richer source truth means less inference.

## 9. Stereo support remains sparse

Stereo-derived presentation does not need to populate the entire static vocabulary.

Central/foundation musical information may remain primarily in the protected stereo master rather than being fabricated as discrete `C` or `LFE` channels. Rear, height, and lower support remain `DERIVED` when created from stereo evidence.

The exact currently populated support lanes and internal shell weights are implementation state and belong in code/tests, not this contract.

## 10. Semantic scene and rendering lattice are distinct

```text
17 semantic anchors
+ continuous objects
        ↓
renderer mapping / interpolation
        ↓
internal support/render lattice
        ↓
HRTF / ITD / distance / room
        ↓
stereo headphones
```

An internal render lattice may be denser than the semantic scene. It is not an authored input format and must not leak outward as fake source channels.

A continuous directional field may be represented internally by first-order or higher-order coefficients and projected to the ears from a dense measured-HRTF sampling or another demonstrably equivalent continuous transfer. A small set of virtual loudspeaker/cardinal axes is an implementation approximation, not scene truth, and must not impose an audible directional lattice on an otherwise continuous field.

Discrete HRTF sampling directions remain renderer samples. They do not become authored anchors or source roles.

## 11. Presentation entity vocabulary

Useful higher-level entities remain distinct from channel provenance:

### `FrontalAnchor`
Material whose relocation would destabilize musical focus, center of gravity, or groove floor.

### `DirectObject`
Persistent source-like material for which spatially specific presentation is justified.

### `BroadSource`
Coherent source-like material with meaningful extent or insufficient evidence for a point representation.

### `DiffuseField`
Musical or ambient energy better represented as a distribution.

### `RoomField`
Environmental energy such as reflections and late reverberation.

Keep:

```text
DirectObject
≠ BroadSource
≠ DiffuseField
≠ RoomField
```

A source's center, extent, and environmental field are separate dimensions.

## 12. Channel-derived object extraction is presentation, not source truth

A channel-based presentation stage may extract directional or phantom evidence into derived objects only when:

- source energy bookkeeping remains bounded and explainable;
- the residual channel bed remains valid;
- extracted objects are explicitly `DERIVED`;
- true source objects are not passed through the extractor;
- latency/alignment is handled for all affected lanes;
- callback partitioning does not redefine the result.

The exact extraction algorithm, FFT size, sector count, and tunable parameters are implementation details owned by code/tests.

## 13. Direct, early, and late rendering jobs differ

```text
DIRECT
→ source direction / identity / HRTF / ITD / distance

EARLY FIELD
→ directional reflection timing and externalization cues

LATE FIELD
→ bounded closure / envelopment / decay
→ preserve continuous field structure through binaural projection
```

Do not use late reverberation as a universal substitute for direct width, source extent, distance, or rear placement.

A late field may use a compact coefficient representation internally, but its binaural projection must preserve the intended directional continuity rather than making the listener hear the decoder's support axes. Additional externalization should preferentially improve early-field geometry before increasing late-field energy or decay.

## 14. Bass and groove law

Frequency alone must not imply object identity.

A diffuse low-frequency region may deserve protection without becoming a fake compact bass object. A melodic bass line may require contour and agency. Groove material may be judged primarily by timing and pressure.

Spatial scale may not dissolve the groove floor.

## 15. Motion and head tracking

Authored object motion remains source truth. Presentation smoothing may make transitions artifact-free but may not freeze or invent authored trajectories.

Head tracking modifies listener-relative rendering, not source authority. World-stable head-tracked rendering is a renderer/listener transform layered after source semantics.

## 16. Validation lanes

Keep validation decomposed:

### Known-scene lane

```text
known authored geometry
→ scene
→ renderer
→ headphones
```

Tests renderer behavior without stereo inference uncertainty.

### Stereo-presentation lane

```text
controlled stereo
→ evidence
→ bounded DERIVED support
→ renderer
```

Tests inference/presentation permission.

### Rich-input lane

```text
known fixed channels / objects / field
→ preserve AUTHORED semantics
→ renderer
```

Tests source authority and ingress.

### Transition lane

```text
declaration/timed-state changes
→ bounded transition
→ no click / stale identity / callback-shaped semantics
```

## 17. Contract acceptance

A source/scene/rendering change is acceptable only if it preserves:

- canonical provenance;
- fixed-channel/object distinction;
- stable object identity;
- source timing;
- stream-local semantics;
- one renderer decision model;
- bounded realtime behavior;
- no fake authorship;
- no premature downmix when richer truth is available;
- no second virtualization of already-binaural material.
