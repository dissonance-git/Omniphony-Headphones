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

Current bounded stereo listening candidate: preserve the true order-2 image direction of front/top-front early reflections through four dedicated measured-HRTF buses while conserving the accepted front early tap-power budget and sub-300 Hz coherent return. Keep it outside the protected default until physical A/B either promotes it or deletes it.

**Gate:** remaining deficits can be attributed to the binaural renderer itself because source geometry and source authority are controlled.

---

## 7. Extend authored scenes beyond the protected baseline

The accepted default Windows music presentation is now protected. Do not reopen its early/late-field balance, bass foundation, transient behavior, center authority, or default route merely to add capability.

Build the next spatial gains first where the source has supplied more truth:

- consume the canonical sample-time object/block contract in every rich-source adapter, including real Windows dynamic-object ingress, seek/recovery, lifetime/EOS, and explicit discontinuity handling;
- preserve continuous metric XYZ and authored radial distance through adapter, scene, render-policy, and binaural boundaries without normalizing objects onto a unit shell;
- extend the portable authored-scene conformance lane with ADM/BW64-facing adapters and reference fixtures for object, direct-speaker, and HOA semantics so source interpretation can be tested independently of Windows provider availability;
- compare ADM-style timing and scene behavior against established reference renderers where practical, including fractional block boundaries and interpolation/jump semantics;
- evaluate physically motivated near-field cues such as acoustic parallax, ear-specific geometry, distance-dependent ILD, and bounded near/far spectral behavior, with smooth convergence toward far-field behavior rather than an audible mode boundary;
- keep radial-distance and near-field candidates independently switchable and out of the protected stereo-master path unless a separate stereo candidate later earns promotion;
- compare source-equivalent scenes against standards/reference renderers before attributing a deficit to binaural rendering.

Any sound-changing distance/HRTF candidate must remain independently switchable and revertible until engineering validation and physical listening both pass.

**Gate:** an authored moving object preserves identity, PCM, sample-time motion, XYZ and radial distance through the renderer while the protected default stereo baseline remains unchanged.

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

## 10. Plan optional head tracking for broader publication

Head tracking is not part of the protected reference listening path and is not required for ordinary Omniphony use.

Preserve the existing renderer-level head-pose capability and plan a future publication-facing integration layer with supported transports, latency/update budgets, recentering and coordinate conventions, dropout/reconnect behavior, smoothing, and tracked-vs-head-locked listening policy. Mature tracker ecosystems such as OpenTrack are preferred integration points over making Omniphony responsible for camera/IMU tracking itself.

Tracking must remain optional, must never mutate source authority or authored motion, and must not make non-tracked listening second-class.

**Gate:** before advertising head tracking publicly, a supported tracker path stabilizes a virtual acoustic scene under real motion without discontinuities or excessive latency. This gate does not block the current non-tracked product path.

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

Harden the existing host-neutral scene contract into a stable external host API only after Windows object ingress and source-authority semantics are physically proven.

The published API must cover authored static roles, continuous objects, exact source-time semantics, metric geometry/radial distance, bounded source identity, Ambisonics/HOA where supplied, stereo-derived support, already-binaural provenance, listener pose/profile, and output/latency contracts.

Windows concepts must remain in the Windows adapter, and renderer DSP internals must not become part of the public scene ABI merely because the current implementation consumes them.

**Gate:** a non-Windows host can feed the same source-authority semantics into the same renderer without inheriting WASAPI, WDK, registry, tray, service, or renderer-internal scene machinery.

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
→ receive dynamic XYZ objects with sample-time continuity
→ prove a real spatial application/game
→ offline authored-scene conformance + controlled renderer comparisons
→ authored radial-distance / near-field rendering
→ HRTF personalization
→ already-binaural policy + Windows hardening
→ stable portable scene API
→ public release
→ optional head-tracking integration when advertised
```

When a gate is crossed, delete it here. If it established a durable law, fold that law into the appropriate living contract or executable regression. Git history is the only project chronology.
