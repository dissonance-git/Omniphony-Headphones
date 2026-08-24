# omniphony-renderer

`omniphony-renderer` is the portable DSP and scene-rendering workspace inside this Omniphony fork.

Windows is the current live host, but the engine is not a Windows-only design.

```text
source truth / stereo evidence
        ↓
canonical scene state
        ↓
Current spatial shell
        ↓
binaural renderer
        ↓
listener / headphone correction
        ↓
stereo headphones
```

For the product overview, start at [`../README.md`](../README.md).

---

## Current product contract

The Current stereo path is built around a **17-lane canonical 8.1.4.4 scene**:

```text
L R C LFE Ls Rs Lb Rb Cb Tfl Tfr Tbl Tbr Bfl Bfr Bbl Bbr
```

Stereo analysis currently populates only evidence-backed lanes:

```text
L R Ls Rs Lb Rb Tfl Tfr Tbl Tbr
```

The remaining canonical lanes stay EMPTY:

```text
C LFE Cb Bfl Bfr Bbl Bbr
```

The canonical scene then feeds the **22-direction System-H-derived Current shell**, followed by cascaded binaural rendering to stereo.

```text
stereo evidence
→ canonical 8.1.4.4 scene
→ 22-direction Current shell
→ HRTF / ITD / room rendering
→ stereo
```

The scene and the shell are deliberately different layers. The 17-lane scene is semantic source/presentation state. The 22-direction shell is internal render geometry.

---

## Source authority

The core must preserve the strongest truth available at its input boundary.

```text
stereo
→ preserve master + derive bounded support

5.1 / 7.1
→ preserve authored channels when a host exposes them

height beds
→ preserve supplied height

objects
→ preserve supplied continuous positions

Ambisonics / HOA
→ preserve the field representation

already-binaural material
→ do not apply a second HRTF renderer blindly
```

Use provenance explicitly:

```text
AUTHORED
DERIVED
EMPTY
```

A rich scene vocabulary is not permission to invent authorship.

---

## Workspace ownership

### `renderer`

Portable DSP core. Owns HRTF/ITD rendering, scene and stereo inference, music-field construction, speaker geometry, VBAP support, room machinery and related validation helpers.

### `orender_engine`

Headless construction layer for product rendering. `current_music_support` is the product-facing Current bridge from the canonical scene through the 22-direction shell into cascaded binaural output.

### `realtime_ffi`

Narrow realtime ABI used by the Windows APO and spatial-provider experiments. It owns the stereo Current worker seam plus authored native-bed and static-object realtime ingress. Richer authored geometry bypasses stereo inference rather than being collapsed into the stereo path.

### `dsp_fixtures`

Deterministic measurement and regression ruler. It should remain independent enough that broken DSP cannot certify itself.

### `reference_bridge`, `bed_ffi`, `source_ffi`, `orender_ffi`

Embedding and known-source laboratory surfaces. They are useful for rich-input and deterministic tests, but they do not define the normal Windows product path.

### `audio_input`, `audio_output`, `host_audio`

Host/platform machinery inherited from the broader renderer. Useful pieces remain, but the Windows product now uses the endpoint APO rather than the old loopback host as its normal system-audio boundary.

### `windows_installer/endpoint_apo`

Development Windows EFX APO, diagnostics, installer integration and the production component-package scaffold.

---

## Realtime law

Audible behavior belongs to one logical sample timeline, not to the host callback partition.

Changing callback or block size must not create a different:

- gain trajectory;
- source trajectory;
- HRTF trajectory;
- transient response;
- room transition;
- inferred scene organization.

The renderer now carries dedicated regression work for callback-invariant binaural motion and Current-shell behavior. New realtime state must follow the same law.

The Windows AudioDG callback also must remain bounded. Heavy Current processing lives behind preallocated transfer structures and a dedicated worker, with aligned dry fallback rather than blocking the audio engine.

---

## Binaural path

At a high level:

```text
scene / shell state
→ listener-relative direction
→ HRTF interpolation
→ ITD
→ stateful convolution
→ directional early field
→ bounded late closure
→ stereo
```

Important retained mechanisms include:

- measured and parametric HRTF providers;
- optional SOFA support;
- direction interpolation and motion smoothing;
- stale asynchronous HRTF rebuild rejection;
- measured-HRIR direct-arrival validation;
- analytic ITD handling;
- distance and air behavior;
- directional early reflections;
- late room machinery;
- block/callback invariance gates.

See [`BINAURAL.md`](BINAURAL.md).

---

## Current music path

The product does not replace the finished stereo master with a wet binaural reconstruction.

```text
protected master
+
coherent foundation
+
evidence-derived spatial support
→ peak-safe stereo output
```

The protected master remains direct. Spatial support is additive and bounded.

This matters because externalization, localization, timbre and musical impact are separate obligations. More room energy is not automatically better externalization, and a wider image is not automatically a more faithful one.

---

## Validation priorities

Engineering tests should isolate at least four layers:

```text
SOURCE TRUTH
Did AUTHORED / DERIVED / EMPTY survive correctly?

SCENE GEOMETRY
Did the canonical 17-lane scene remain intact?

RENDER GEOMETRY
Did Current expand through the intended 22-direction shell?

BINAURAL OUTPUT
Did the result remain stable, finite, stereo and perceptually plausible?
```

Current focused gate:

```sh
cargo test -p orender_engine --test current_scene_geometry
```

That test locks:

- `MUSIC_FIELD_CHANNELS == 17`;
- exact canonical lane order;
- EMPTY-lane preservation for stereo-derived Current;
- 22-direction Current shell count;
- final binaural stereo output.

Other important axes include ITD/ILD, diffuse-field behavior, timbral coloration, transient preservation, bass timing, block-size invariance, motion continuity, non-finite handling and headroom.

---

## Research alignment

Peer-reviewed binaural research reinforces the way this workspace separates obligations:

- HRIR time alignment and diffuse-field constraints can improve coloration, localization and externalization together rather than treating them as unrelated metrics.
- Reverberation-related binaural cues can be especially important for frontal externalization.
- Interaural coherence is strongly associated with externalization in several listening studies.
- World-stable head-tracked motion can materially improve externalization, particularly for frontal and rear sources.

Representative references are listed in the root README. They guide tests and frontiers, not automatic parameter choices.

---

## Current Windows boundary

The normal format-changing stream SFX accepts stereo and authored multichannel float32 while the physical headphone endpoint remains stereo. Stereo enters protected-master Current. Authored beds retain their channel mask and positions and bypass stereo inference. Authored 7.1 is physically verified and authored 7.1.4 processing is regression-tested.

```text
stereo client
→ protected stereo Current
→ stereo endpoint

5.1 / 7.1 / height bed
→ authored channel identities
→ source-authoritative Current render
→ stereo endpoint
```

The richer Windows Spatial Audio provider path remains gated while static-object transport and RAW single-render egress are proven end to end. Its authored object geometry is another ingress into the same portable scene/renderer contract, not a second renderer and not a serial stage after the stream SFX.

---

## Build and focused test

The workspace currently targets Rust `1.88.0`.

From this directory:

```sh
cargo fmt --all
cargo test -p renderer
cargo test -p orender_engine --test current_scene_geometry
cargo test -p realtime_ffi
```

Windows build, APO, packaging and dependency audits live in the repository-level `.github/workflows/` directory.

---

## Design rule

> **Use the inherited Omniphony renderer for spatial jobs it already owns. Add fork-specific machinery only for source preservation, evidence, bounded presentation control, product integration and validation.**

The renderer is the engine. The evidence layer decides what it is allowed to ask the engine to do.
