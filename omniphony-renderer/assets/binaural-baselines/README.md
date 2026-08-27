# Binaural rendering controls

This directory contains the small set of renderer configurations that still own a current validation role.

The protected product presentation is `current-stereo-field.yaml`. Its exact blob identity is pinned by `../../current-listening-baseline.manifest`; ordinary cleanup must not change its sound-affecting bytes casually.

## Current owners

- `current-stereo-field.yaml` — protected stereo-support field used by the accepted Current renderer.
- `upstream-demo-reference.yaml` — upstream Omniphony demo-style perceptual ancestor for controlled comparison.
- `baseline-room.yaml` — room-assisted comparison control.
- `dry-binaural.yaml` — room-disabled isolation control.

The bundled `../demo/spatial-demo.wav` is the known-scene fixture for separating renderer behavior from stereo inference.

## Comparison law

Use one source route and loudness-match deliberately. Keep dimensions independent where possible:

```text
front / rear discrimination
height / radial distance
source stability / extent
envelopment / room presence
transient and center solidity
timbral fidelity / bass body
groove / microdetail / dynamics
```

A successful spatial presentation should enlarge the perceived world without making bypass restore bass, clarity, punch, tonal correctness, or musical coherence.

Historical listening sequences, superseded stereo prototypes, and retired host-routing diagnoses belong to Git history. Current product obligations belong in the repository contracts and executable baseline guard.
