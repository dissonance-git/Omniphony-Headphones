# Omniphony roadmap

This file is the canonical owner of **unresolved current work, acceptance gates, and project frontier**.

It owns no durable architecture, implementation inventory, completed phase, research archive, or chronology.

- durable product identity and stable architecture → `README.md`;
- governing development/listening law → `AGENTS.md`;
- Windows host/ingress/egress law → `docs/omniphony-for-windows.md`;
- source/scene/renderer semantics → `docs/scene-renderer-contract.md`;
- music presentation law → `docs/music-presentation-contract.md`;
- executable implementation state → code, tests, and CI;
- chronology and retired alternatives → Git history.

When a gate is crossed, remove it from this roadmap. Fold any surviving durable consequence into the appropriate living contract or executable regression. Do not move completed work into a history section or evidence ledger.

The current product target remains one open spatial renderer that preserves the richest trustworthy source representation, performs one final binaural render, and invents only what the source does not already provide.

---

## 1. Prove the physical spatial-output path

Run the finite closed-gate spatial egress diagnostic on the exact physical Windows headphone endpoint.

Required evidence:

```text
COM-shaped authored static source reaches Omniphony
→ binaural stereo reaches the bounded egress queue
→ the real endpoint event clock drains audio through IAudioRenderClient
→ producer drops remain zero
→ underrun behavior is measured
→ provider registration remains untouched
→ public provider selection remains untouched
```

**Gate:** authored static spatial content reaches the real headphone endpoint through Omniphony exactly once while the public Spatial Sound provider remains closed.

---

## 2. Prove Windows Spatial Sound provider enumeration and activation

After physical egress is proven independently, verify safe provider registration, enumeration, selection, COM activation, deselection, rollback, and uninstall recovery.

The provider-registration surface remains experimental until physically verified. Registry shape, third-party observations, or successful COM construction alone are insufficient.

**Gate:** Windows can enumerate, select, activate, and safely deselect Omniphony without leaving the machine on a provider that cannot render.

---

## 3. Receive real static Windows Spatial Audio objects

Cross from internal/static test machinery to objects supplied through the actually selected Windows Spatial Audio path.

Preserve static role identity, PCM, source authority, volume, update timing, lifetime/EOS semantics, and authored role position. Do not route real objects through stereo inference.

First prove one real static object above or below the listener. Then scale to the complete 17-role vocabulary:

```text
horizontal: FL FR C LFE SL SR BL BR BC
upper:      TFL TFR TBL TBR
lower:      BFL BFR BBL BBR
```

**Gate:** the full 17-role static vocabulary survives Windows ingress, Omniphony rendering, cadence adaptation, and physical egress without role substitution, duplicate spatialization, or inferred replacement.

---

## 4. Receive dynamic XYZ objects

Add true dynamic-object capacity only when identity and continuous geometry can be preserved end to end.

Required semantics include stable object identity, audio buffer, continuous XYZ, volume, lifetime, motion trajectory, and update timing. Do not snap dynamic objects to the static frame.

**Gate:** a real moving object crosses arbitrary 3-D space while identity, audio, position, and motion continuity survive into the final render.

---

## 5. Prove a real spatial application or game end to end

Use a real application that submits Windows Spatial Audio objects and prove:

```text
application produced spatial objects
→ Windows selected Omniphony
→ Omniphony received authored objects
→ PCM + geometry reached the renderer
→ Omniphony performed the single final binaural render
→ the physical endpoint received that result
```

Cover horizontal placement, elevation, front/back, motion when available, LFE/non-directional behavior, pause/resume/restart, and coexistence with ordinary applications.

**Gate:** a real spatial title can use Omniphony as its selected Windows headphone renderer with no second virtualization pass.

---

## 6. Establish perceptual parity and superiority on equivalent authored scenes

Once real object ingress exists, compare Omniphony against established headphone renderers using source-equivalent scenes.

Measure localization, front/back, elevation, motion continuity, frontal externalization, radial distance, source extent, center solidity, transient localization, envelopment, timbre, bass integrity, impact/groove, and long-session naturalness.

Level-match where needed, separate localization from tonal preference, separate source geometry from room/externalization effects, and preserve a winning baseline before each sound-changing experiment.

**Gate:** remaining deficits can be attributed to the binaural renderer itself because source geometry and source authority are controlled.

---

## 7. Complete frontal externalization, distance, and room behavior

The immediate perceptual problem is frontal depth/externalization, not more frontal quantity.

The physically accepted dense first-order measured-HRTF late-field projection is now integrated into `main`. Keep its late level, decay, predelay, protected-master relationship, and working Windows host route fixed while evaluating the next mechanism.

The current bounded candidate increases early-reflection HRTF directional resolution from six global final-wall averages to ten deterministic direction clusters. It preserves the existing image-source delay/tone banks, first-/second-order tap gains, transient law, total reflection-power bookkeeping, and late field. Clustering is initialized once from static source-wall geometry; the realtime loop only follows precomputed routes.

The ten-bus choice is intentional rather than a round-number expansion: on the current source-wall geometry it materially reduces weighted directional quantization error versus the six-wall baseline while using every cluster, whereas a twelve-bus trial added effectively redundant capacity. Focused early-field tests and the full source-aware spatial suite have passed. Physical listening remains the promotion gate; engineering success alone does not establish perceptual improvement.

Immediate sequence:

```text
accepted dense late field stays fixed
→ finish Windows/installer engineering validation of the ten-cluster early candidate
→ compare ten-cluster early field against the accepted dense-late baseline
→ require no loss of bass, transient ownership, center solidity, clarity, motion, or route reliability
→ keep / revise / revert from physical listening
→ only then revisit second-order share or transient excitation
```

Investigate bounded front-specific early-field structure, HRTF interaction, direct-to-early/reverberant evidence, binaural coherence, radial-distance cues, near-field behavior, and source extent without sacrificing directness, bass, transients, center solidity, or side/rear envelopment.

Do not buy externalization with indiscriminate late reverb, synthetic width, or another copied wet path.

**Gate:** the higher-resolution early field beats the accepted dense-late baseline in clean-route physical listening, preserves the protected musical invariants, and remains reliable on the real Windows endpoint. If it does not, revert or revise it rather than increasing wet energy to hide the deficit.

---

## 8. Turn HRTF flexibility into listener personalization

Create a coherent optional listener-facing HRTF workflow:

- repeatable A/B comparison without unrelated DSP changes;
- useful generic HRTF choices where evidence supports them;
- optional morphology/measurement mapping when validated;
- measured individualized SOFA support;
- a strong generic default;
- separate layers for HRTF, headphone EQ, and hearing-asymmetry correction.

**Gate:** a listener can select or supply a better-fitting HRTF with measurable/localizable benefit without mandatory calibration ritual.

---

## 9. Add trustworthy already-binaural source policy

Automatic mixed-source handling must distinguish ordinary stereo from stereo already containing a headphone spatial render.

Prefer provenance/session metadata over waveform guessing. Channel count alone is insufficient. Provide deterministic override when classification is uncertain.

Target policy:

```text
ordinary stereo → Omniphony stereo inference/enhancement
known already-binaural → spatial bypass + separately validated non-spatial correction only
unknown stereo → conservative deterministic policy
```

**Gate:** Omniphony does not blindly apply a second HRTF render to already-rendered headphone content.

---

## 10. Productize optional head tracking

Turn renderer-level head-pose capability into a reliable optional product feature. Define supported transports, latency/update budgets, recentering and coordinate conventions, dropout/reconnect behavior, smoothing, and tracked-vs-head-locked listening policy.

Tracking must never mutate source authority or authored motion.

**Gate:** head tracking stabilizes a virtual acoustic scene under real motion without discontinuities, excessive latency, or making non-tracked listening second-class.

---

## 11. Harden Omniphony for Windows as a product

Complete unattended-use reliability across:

- endpoint hotplug, DAC power cycling, default-device changes, format/period changes, suspend/resume, service restart;
- stream/object lifecycle abuse, queue overflow/underrun, worker starvation/recovery;
- provider selection/deselection, transactional activation, rollback, upgrade, repair, immutable generations, and clean uninstall;
- release artifact identity, signing/reputation strategy, and transparent elevated behavior;
- compatibility across stereo players, browsers, communications, authored multichannel, Spatial Audio apps/games, already-binaural media, and relevant processing modes;
- diagnostics that preserve and report the endpoint's actual baseline mix geometry rather than treating stereo as a universal health floor;
- separate proof of SFX registry attachment, AudioDG Stream-APO instantiation, realtime-renderer DLL loading, intended source ingress, and actual once-only sample transformation.

The current `--shared-7.1` health path must be hardened so a multichannel endpoint mix does not fail merely because the probe assumes `stereo-float32-48000` before testing the client boundary.

**Gate:** Omniphony can remain installed as ordinary system audio software without requiring the user to nurse the audio graph, and its diagnostics distinguish endpoint geometry from live APO/runtime activity instead of collapsing them into one pass/fail assumption.

---

## 12. Publish a stable portable spatial-scene API

After Windows object ingress and source-authority semantics are physically proven, expose a stable host-neutral API for authored static roles, continuous objects, Ambisonics/HOA where supplied, stereo-derived support, already-binaural provenance, listener pose/profile, and output/latency contracts.

Windows concepts must remain in the Windows adapter.

**Gate:** a non-Windows host can feed the same source-authority semantics into the same renderer without inheriting WASAPI, WDK, registry, tray, or service machinery.

---

## 13. Public-release gate

A public release follows demonstrated product behavior, not source completeness alone.

Minimum evidence:

```text
real Windows Spatial Sound provider selection
+ real static-object ingress
+ real dynamic-object ingress where advertised
+ one-render physical egress proof
+ real spatial application/game proof
+ stereo and authored-PCM regression safety
+ already-binaural protection policy
+ installer / upgrade / rollback / uninstall safety
+ compatibility matrix
+ controlled listening validation
+ reproducible release artifact identity
```

Do not advertise capabilities whose final physical/application boundary has not been crossed.

---

## Critical path

```text
prove closed-gate physical spatial egress
→ prove provider enumeration + safe activation
→ receive one real static object
→ receive full static vocabulary
→ receive dynamic XYZ objects
→ prove a real spatial application/game
→ controlled renderer comparisons
→ physically validate higher-resolution early-field direction clustering
→ finish frontal externalization / distance
→ HRTF personalization + optional head tracking
→ already-binaural policy + Windows hardening
→ stable portable scene API
→ public release
```

When a gate is crossed, delete it here. If it established a durable law, fold that law into the appropriate living contract or executable regression. Git history is the only project chronology.
