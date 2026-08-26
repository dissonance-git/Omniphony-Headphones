# Contributing to Omniphony

Omniphony is a free and open-source spatial audio renderer for headphones. Contributions are welcome across DSP, realtime systems, Windows audio integration, source-scene handling, testing, documentation, and portability.

## Read before changing code

Use the repository's canonical owners rather than relying on duplicated summaries:

1. [`README.md`](README.md) for product identity, stable architecture, and source-authority model;
2. [`AGENTS.md`](AGENTS.md) for governing development, evidence, realtime, and listening law;
3. [`ROADMAP.md`](ROADMAP.md) when your work targets unresolved current work;
4. the smallest relevant contract under [`docs/`](docs/);
5. the exact implementation and tests you intend to change.

The repository follows this design rule:

> **Store authority. Derive views. Preserve evidence. Promote slowly.**

Do not copy architecture, status, or implementation inventories into contributor prose when a canonical owner or exact repository query already exists.

## Contribution shape

Prefer focused changes that answer one clear question and include the smallest coherent implementation plus the evidence needed to support it.

A good contribution makes it easy to answer:

```text
What changed?
Why does it belong in Omniphony?
Which source representations are affected?
Which invariant protects against regression?
How was it validated?
What remains unproven?
```

Avoid combining unrelated renderer tuning, host plumbing, repository restructuring, and calibration changes unless they truly cannot be separated.

Large architectural changes should identify the missing obligation, the canonical owner, the baseline, the candidate mechanism, validation, migration/rollback, and which weaker or duplicate machinery is retired.

## Core boundaries

Do not weaken the durable contracts in `README.md` and `AGENTS.md`.

In particular:

- preserve authored source identity and geometry;
- never relabel inferred geometry as authored;
- keep the portable renderer independent of Windows host concepts;
- keep realtime callback work bounded and deterministic;
- keep already-binaural material from being blindly virtualized twice;
- keep one final binaural render;
- protect stereo master identity, bass, transients, center stability, dynamics, timbre, and motion;
- keep public/default tuning distinct from listener-specific calibration unless broader evidence earns promotion.

For detailed scene and Windows semantics, follow the relevant focused contracts under `docs/` rather than duplicating them here.

## Testing

The renderer workspace currently requires Rust 1.88.0 or newer.

From `omniphony-renderer/`, useful focused commands include:

```sh
cargo test -p renderer
cargo test -p renderer --test source_shell_spread_energy
cargo test -p orender_engine --lib --tests
cargo test -p orender_engine --test source_shared_wet_extent
cargo test -p source_ffi --lib --tests
cargo test -p source_ffi --test runtime_spatial_mode
cargo test -p realtime_ffi
```

Windows host changes should also pass the relevant APO build, COM/lifecycle, manifest, realtime ABI, installer, endpoint/client-format, rollback, and packaging checks in CI.

CI failures are evidence. Do not make a gate green by weakening the requirement unless the requirement itself has been shown to be wrong.

## Audible DSP changes

For an audible change, record at minimum:

```text
intended percept
source types affected
comparison baseline
mechanism changed
objective behavior changed
fidelity cost or risk
physical listening result
keep / revise / revert decision
```

Useful objective checks include null/residual tests where identity is expected, peak/RMS and headroom, frequency response, ITD/interaural behavior, transient timing, bass coherence, state-switch continuity, block-size invariance, non-finite handling, and source/channel/object provenance.

Human listening remains required for perceptual questions such as externalization, front/back discrimination, elevation, radial depth, source extent, image stability, envelopment, room naturalness, direct-source solidity, bass/groove integrity, timbre, fatigue, and preference.

Listening should be loudness-aware and route-clean. Do not draw conclusions while duplicate physical paths or multiple headphone virtualizers are active unintentionally.

Accepted/rejected perceptual evidence belongs in [`docs/listening-history.md`](docs/listening-history.md). Unresolved work belongs in [`ROADMAP.md`](ROADMAP.md).

## Documentation and evidence

Public-facing documentation should describe stable product behavior, contracts, and supported capabilities. Current project status belongs in `ROADMAP.md`; historical listening evidence belongs in `docs/listening-history.md`; implementation truth belongs in code/tests/CI.

Keep evidence states distinct:

```text
source builds
≠ tests pass
≠ host negotiation succeeds
≠ endpoint association succeeds
≠ a real application supplies the expected representation
≠ physical playback succeeds
≠ physical listening confirms the intended percept
```

Do not promote a capability beyond the strongest evidence actually obtained.

Machine-specific debugging transcripts, temporary hypotheses, one-off game configurations, and dated experiment narratives belong in focused evidence/research material rather than the root README or contributor guide.

## Upstream and third-party work

Omniphony is derived from the original [`mgth/Omniphony`](https://github.com/mgth/Omniphony) project. Preserve upstream attribution and licensing. See [`NOTICE.md`](NOTICE.md).

External projects, papers, datasets, and proprietary spatial renderers may be useful references or comparison targets, but they are not automatic dependency choices. Check licensing and redistribution implications before adding code, data, HRTFs, models, or other assets.

## Submitting changes

Before publication, re-fetch current `main`, inspect the exact targets, preserve concurrent work, and run validation proportionate to the change. After publication, verify the resulting commit and report build/tests, CI, measurements, and physical listening as separate evidence states.
