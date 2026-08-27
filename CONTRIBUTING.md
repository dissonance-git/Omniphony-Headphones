# Contributing to Omniphony

Omniphony is a free and open-source spatial audio renderer for headphones. Contributions are welcome across DSP, realtime systems, Windows audio integration, source-scene handling, testing, documentation, and portability.

## Read before changing code

Use canonical owners rather than duplicated summaries:

1. [`README.md`](README.md) for product identity and stable architecture;
2. [`AGENTS.md`](AGENTS.md) for governing development/evidence law;
3. [`ROADMAP.md`](ROADMAP.md) when work targets an unresolved gate;
4. the smallest relevant living contract under [`docs/`](docs/);
5. the exact implementation and tests you intend to change.

Connector-only agents should also read [`.agents/github-connector.json`](.agents/github-connector.json) and [`.agents/skills/github-workspace/SKILL.md`](.agents/skills/github-workspace/SKILL.md). Search is discovery; exact refs/blobs establish repository truth.

> **Store authority. Derive views. Preserve evidence. Promote slowly.**

In this repository, retired experiments and chronology belong to Git history. Do not create research ledgers, listening histories, status reports, migration diaries, or frozen frontier snapshots. Fold the surviving decision into a living contract, test, code path, or current roadmap item.

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
What redundant surface can now be removed?
```

Avoid combining unrelated renderer tuning, host plumbing, repository restructuring, and calibration changes unless they truly cannot be separated.

Large architectural changes should identify the missing obligation, canonical owner, candidate mechanism, validation, migration/rollback, and which weaker or duplicate machinery is retired.

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

## Testing

The renderer workspace currently requires Rust 1.88.0 or newer.

From `omniphony-renderer/`, useful focused commands include:

```sh
cargo test -p renderer
cargo test -p orender_engine --lib --tests
cargo test -p source_ffi --lib --tests
cargo test -p realtime_ffi
```

Windows host changes should also pass the relevant APO build, COM/lifecycle, manifest, realtime ABI, installer, endpoint/client-format, rollback, and packaging checks in CI.

CI failures are evidence. Do not make a gate green by weakening the requirement unless the requirement itself has been shown to be wrong.

## Audible DSP changes

For an audible change, evaluate at minimum:

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

Listening should be loudness-aware and route-clean. Do not draw conclusions while duplicate physical paths or multiple headphone virtualizers are unintentionally active.

After the decision:

```text
retained stable rule
→ living contract and/or regression test

unresolved consequence
→ ROADMAP.md

retired comparison narrative
→ Git history only
```

## Documentation and evidence

Working-tree documentation describes current product law, current unresolved work, or a focused current technical contract. Implementation truth belongs in code/tests/CI.

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

Do not promote a capability beyond the strongest evidence obtained.

Do not preserve machine-specific debugging transcripts, dated experiment narratives, old implementation inventories, or completed validation reports in `docs/`. Git already preserves the exact repository state that produced them.

## Upstream and third-party work

Omniphony is derived from the original [`mgth/Omniphony`](https://github.com/mgth/Omniphony) project. Preserve upstream attribution and licensing. See [`NOTICE.md`](NOTICE.md).

External projects, papers, datasets, and proprietary spatial renderers may be useful references or comparison targets, but they are not automatic dependency choices. Check licensing and redistribution implications before adding code, data, HRTFs, models, or other assets.

## Submitting changes

Before publication, re-fetch current `main`, inspect the exact targets, preserve concurrent work, and run validation proportionate to the change. After publication, verify the resulting commit and report build/tests, CI, measurements, and physical listening as separate evidence states. Remove superseded documentation rather than memorializing it.
