# omniphony-renderer

This directory is the portable implementation workspace behind Omniphony.

Start with the repository [README](../README.md) for product identity and [AGENTS](../AGENTS.md) for governing change law. This file is only an implementation map.

## Core path

```text
source truth / stereo evidence
→ canonical scene
→ presentation/render geometry
→ binaural renderer
→ stereo headphones
```

The scene and render lattice are different layers. Authored channels/objects keep their source authority; stereo may create only bounded `DERIVED` support.

## Main owners

| Path | Responsibility |
| --- | --- |
| `scene_contract/` | host-neutral authored scene, timing, geometry, stable source slots |
| `renderer/` | portable scene rendering, HRTF/ITD, room, stereo inference/presentation |
| `orender_engine/` | headless product composition and Current music support |
| `realtime_ffi/` | narrow native-host realtime PCM/scene ABI |
| `source_ffi/`, `bed_ffi/`, `orender_ffi/` | embedding and known-source interfaces |
| `dsp_fixtures/` | independent deterministic DSP measurement/regression |
| `windows_host/`, `windows_installer/` | Windows-specific host and deployment work |
| `audio_input/`, `audio_output/`, `host_audio/` | portable/inherited host-audio support |

Windows-specific concepts stay out of portable scene semantics.

## Current renderer contracts

- [source and scene](../docs/scene-renderer-contract.md)
- [music presentation](../docs/music-presentation-contract.md)
- [binaural renderer](../docs/binaural-renderer.md)
- [realtime/control](../docs/realtime-control-contract.md)
- [Windows host](../docs/omniphony-for-windows.md)

## Focused validation

The workspace currently targets Rust 1.88.0.

```sh
cargo test -p scene_contract
cargo test -p renderer
cargo test -p orender_engine --lib --tests
cargo test -p realtime_ffi
cargo test -p source_ffi --lib --tests
```

The Current scene geometry gate is:

```sh
cargo test -p orender_engine --test current_scene_geometry
```

Windows APO, installer, packaging, dependency, and provider validation is owned by repository-level `../.github/workflows/`.

## Build/demo references

- [quickstart](QUICKSTART.md)
- [build](BUILD.md)
- [Windows build](BUILDING_WINDOWS.md)
- [bridge API](BRIDGE_API.md)
- [OSC protocol](OSC_PROTOCOL.md)

These are implementation/contributor references. Product law stays in the repository root and `docs/`; historical host instructions and completed experiment narratives belong to Git history.
