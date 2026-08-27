# Quickstart

This is the shortest implementation-level path through `omniphony-renderer`. Product installation and Windows system-audio behavior are documented at the repository root, not here.

## Hear the bundled renderer demo

From a fresh checkout:

```sh
cd omniphony-renderer
./scripts/demo.sh
```

The demo uses the bundled reference bridge and fixture. It is a renderer demonstration, not the normal OS-wide product route.

Other demo outputs, when available on the host:

```sh
./scripts/demo.sh speakers
./scripts/demo.sh file
```

## Build

```sh
cargo build --release
```

The Cargo manifests are the feature/dependency authority. Do not rely on old prose feature inventories when they disagree with `Cargo.toml`.

## Focused tests

```sh
cargo test -p scene_contract
cargo test -p renderer
cargo test -p orender_engine --lib --tests
cargo test -p realtime_ffi
cargo test -p source_ffi --lib --tests
```

For the protected Current scene shape:

```sh
cargo test -p orender_engine --test current_scene_geometry
```

Windows APO, installer, provider, dependency, and packaging checks are owned by repository-level `../.github/workflows/`.

## Reference bridge

The bundled `reference_bridge/` reads known PCM/WAV material and is the smallest decoder/bridge example. The bridge contract is in [`BRIDGE_API.md`](BRIDGE_API.md).

A bridge supplies source PCM and metadata. It must not invent a renderer-specific output layout merely to enter Omniphony.

## Binaural rendering

The portable renderer contract is [`../docs/binaural-renderer.md`](../docs/binaural-renderer.md).

Source/scene semantics are [`../docs/scene-renderer-contract.md`](../docs/scene-renderer-contract.md). The Windows system-wide host is [`../docs/omniphony-for-windows.md`](../docs/omniphony-for-windows.md).

## OSC/control

The detailed implementation reference is [`OSC_PROTOCOL.md`](OSC_PROTOCOL.md). The durable control semantics are owned by [`../docs/osc-control-contract.md`](../docs/osc-control-contract.md).

## Contribution entry

Use:

1. [`../README.md`](../README.md) for product identity;
2. [`../AGENTS.md`](../AGENTS.md) for repository/change law;
3. [`../ROADMAP.md`](../ROADMAP.md) only for unresolved gates;
4. [`../CONTRIBUTING.md`](../CONTRIBUTING.md) for validation and contribution procedure.

Historical host/build instructions belong to Git history once Cargo/workflows own the executable truth.
