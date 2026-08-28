# Upstream reference shelf

This file is Omniphony's **project-local return map** for external GitHub repositories worth revisiting.

It is deliberately not a dependency lockfile, vendor catalog, or second architecture. Exact build/runtime dependencies remain owned by code, manifests, workflows, and patches. Durable product law remains in the focused contracts under `docs/`.

The shelf records only repositories that can save a future deep dive or materially change a design decision:

```text
question
→ return to the smallest relevant upstream
→ inspect current revision if the revisit trigger fired
→ bring the finding back through Omniphony's own contracts/tests
```

A repository is implementation or research evidence only within its stated authority boundary. Vendor ownership, popularity, or fidelity claims do not promote it into Omniphony truth.

## Windows source and host boundary

| Repository | Inspected revision | Inspected surface | Return for | Not authority for | Revisit when |
| --- | --- | --- | --- | --- | --- |
| [`MicrosoftDocs/sdk-api`](https://github.com/MicrosoftDocs/sdk-api) | [`4502fff176b3b56beddb6a63c9f980377b11ba9b`](https://github.com/MicrosoftDocs/sdk-api/commit/4502fff176b3b56beddb6a63c9f980377b11ba9b) | ISpatialAudioObject SetPosition; ISpatialAudioObjectBase SetEndOfStream | Windows dynamic-object coordinates; same-pass PCM and metadata semantics; partial final-buffer EOS semantics | binaural DSP design; perceptual tuning | Microsoft changes Spatial Audio object or lifetime semantics; Omniphony changes Windows ingress |
| [`ThreeDeeJay/MSSOAL`](https://github.com/ThreeDeeJay/MSSOAL) | [`6b639f833b3e4204c03eb586fd904e71433da5fe`](https://github.com/ThreeDeeJay/MSSOAL/commit/6b639f833b3e4204c03eb586fd904e71433da5fe) | README.md; src/SpatialAudioStream.h; dynamic object update flow | ISpatialAudioClient replacement architecture; processing-pass call order; provider integration precedent | Microsoft API specification; reference-quality perceptual rendering | dynamic buffering or stutter handling changes; Omniphony changes Windows provider architecture |
| [`geocausa/SP11X1e-audio`](https://github.com/geocausa/SP11X1e-audio) | [`a1d51ecc7416a905acdad50d31600fff7f28ac1c`](https://github.com/geocausa/SP11X1e-audio/commit/a1d51ecc7416a905acdad50d31600fff7f28ac1c) | tools/windows/sp11_spatial_object_oracle.c; recent repository history | small native Spatial Audio oracle; object API call ordering; independent device and routing evidence | portable renderer architecture; general HRTF authority | the Windows oracle changes; new Windows parity evidence lands; Omniphony host negotiation changes |
| [`smourier/DirectN`](https://github.com/smourier/DirectN) | [`ac0f6979a34e919d8c2e0331265b218b917251eb`](https://github.com/smourier/DirectN/commit/ac0f6979a34e919d8c2e0331265b218b917251eb) | README.md; Spatial Audio interop surface | Windows COM names and types; source-native API binding cross-checks | audio-renderer behavior; perceptual or realtime authority | Windows interop metadata changes; Omniphony adds managed Windows tooling |

## Metadata, scene, and format semantics

| Repository | Inspected revision | Inspected surface | Return for | Not authority for | Revisit when |
| --- | --- | --- | --- | --- | --- |
| [`DolbyLaboratories/pmd_tool`](https://github.com/DolbyLaboratories/pmd_tool) | [`ac0fb7b3434e5e39a194d3744188b1dfdb4fad23`](https://github.com/DolbyLaboratories/pmd_tool/commit/ac0fb7b3434e5e39a194d3744188b1dfdb4fad23) | README.md; S-ADM reader and ingester | ADM and S-ADM to PMD conversion; professional object metadata ingestion; bounded metadata memory ownership | consumer Atmos renderer internals; binaural rendering law | Dolby publishes a new PMD or S-ADM release; Omniphony adds ADM-family ingress |
| [`DolbyLaboratories/sadm-tools`](https://github.com/DolbyLaboratories/sadm-tools) | [`8c8fa2d0660154a6c2caed28870851bddf2f09f2`](https://github.com/DolbyLaboratories/sadm-tools/commit/8c8fa2d0660154a6c2caed28870851bddf2f09f2) | README.md; S-ADM and MGA test material | S-ADM framing and transport; MGA mux and demux semantics; metadata timeline tests | binaural DSP; consumer Atmos behavior | S-ADM or MGA tooling changes; Omniphony adopts live ADM-family object ingress |
| [`DolbyLaboratories/universal_transcoder`](https://github.com/DolbyLaboratories/universal_transcoder) | [`72c739ebad1a5b9e2edcd37a293c576b64992e11`](https://github.com/DolbyLaboratories/universal_transcoder/commit/72c739ebad1a5b9e2edcd37a293c576b64992e11) | README.md; optimization.py; paper examples | format-independent spatial transcoding; psychoacoustic cost functions; irregular layout conversion | permission to flatten stronger source truth; headphone HRTF tuning | USAT implementation changes; Omniphony needs principled authored-format conversion |
| [`DolbyLaboratories/Blender-MPEG-I-Immersive-Audio-Authoring-Add-on`](https://github.com/DolbyLaboratories/Blender-MPEG-I-Immersive-Audio-Authoring-Add-on) | [`1280c8a7106f57b64cb33d228ff4dbaece42e00d`](https://github.com/DolbyLaboratories/Blender-MPEG-I-Immersive-Audio-Authoring-Add-on/commit/1280c8a7106f57b64cb33d228ff4dbaece42e00d) | README.md; audio scene, listener space and listener trajectory features | rich authored scene representation; listener trajectory concepts; MPEG-I scene visualization and authoring | realtime binaural rendering; Windows ingress semantics | Omniphony adds listener translation or MPEG-I import; Dolby updates the MPEG-I authoring model |
| [`ebu/ebu-adm-toolbox`](https://github.com/ebu/ebu-adm-toolbox) | [`526ec71f6d93354f7f8b1668afa930fb2eddabef`](https://github.com/ebu/ebu-adm-toolbox/commit/526ec71f6d93354f7f8b1668afa930fb2eddabef) | README.md; processing graph; block timing conformance example | ADM validation and profile conversion; audio plus metadata processing graphs; block timing conformance | headphone rendering quality; Windows host behavior | EBU ADM profiles or toolbox change; Omniphony builds ADM validation or conversion |
| [`Fraunhofer-IIS/mpeghdec`](https://github.com/Fraunhofer-IIS/mpeghdec) | [`4448b69738da2fa5f2f2f2b0ce29eea32509e046`](https://github.com/Fraunhofer-IIS/mpeghdec/commit/4448b69738da2fa5f2f2f2b0ce29eea32509e046) | README.md; decoder public API; ISO/IEC 23008-3:2026 support | NGA object and scene decoding; portable immersive decoder API design; cross-platform source-semantics reference | patent clearance; Omniphony binaural tuning | MPEG-H standard or decoder revision changes; Omniphony adds MPEG-H ingress |

## Binaural and spatial rendering

| Repository | Inspected revision | Inspected surface | Return for | Not authority for | Revisit when |
| --- | --- | --- | --- | --- | --- |
| [`ValveSoftware/steam-audio`](https://github.com/ValveSoftware/steam-audio) | [`480dd64f513cc8a6437e7d5b9eb0d3f1d30c2fac`](https://github.com/ValveSoftware/steam-audio/commit/480dd64f513cc8a6437e7d5b9eb0d3f1d30c2fac) | simulation documentation; direct source and distance architecture; open PR 567; open PR 563 | separation of distance, air absorption, HRTF and reflections; unchanged-state suppression; render topology versus transport topology checks | Omniphony tuning constants; Windows Spatial Audio API authority | core source or reflection architecture changes; PR 567 or PR 563 changes state; Omniphony materially raises object counts |
| [`kcat/openal-soft`](https://github.com/kcat/openal-soft) | [`3f94a50884e2ae4963092fead7d299127e97e5d5`](https://github.com/kcat/openal-soft/commit/3f94a50884e2ae4963092fead7d299127e97e5d5) | alc/alu.cpp; August 2026 nonblocking mixer commit series | realtime-safe HRTF call graphs; distance and panning precedent; nonblocking mixer audits | Windows Spatial Audio API authority; Omniphony perceptual tuning | HRTF or distance processing changes; new realtime-safety work lands; Omniphony adds a mechanical realtime audit |
| [`ebu/bear`](https://github.com/ebu/bear) | [`6127e897b941211051c2ad135ee09b00be2e6ae0`](https://github.com/ebu/bear/commit/6127e897b941211051c2ad135ee09b00be2e6ae0) | README.md; BRIR interpolation controller; public C++ API | ADM-to-binaural architecture; BRIR interpolation; embeddable realtime renderer API | Dolby-specific metadata behavior; Omniphony stereo tuning | BEAR or Tech 3396 implementation changes; Omniphony adds ADM binaural conformance work |
| [`IRT-Open-Source/binaural_nga_renderer`](https://github.com/IRT-Open-Source/binaural_nga_renderer) | [`56489e82979fbb1db023af2b5b36322caf992b36`](https://github.com/IRT-Open-Source/binaural_nga_renderer/commit/56489e82979fbb1db023af2b5b36322caf992b36) | README.md; virtual-loudspeaker and block-duration options | optimized virtual-loudspeaker binaural NGA rendering; quality versus cost tradeoffs; strict metadata handling | current realtime best practice; direct-object HRTF authority | a maintained successor appears; Omniphony compares direct-object and virtual-loudspeaker architectures |
| [`leomccormack/Spatial_Audio_Framework`](https://github.com/leomccormack/Spatial_Audio_Framework) | [`18fd5aba46e20787b51f28f7197a68506c965c07`](https://github.com/leomccormack/Spatial_Audio_Framework/commit/18fd5aba46e20787b51f28f7197a68506c965c07) | README.md; saf_hrir.c | HRTF and ITD processing; VBAP and Ambisonics; room simulation and spatial DSP primitives | product host integration; Omniphony perceptual acceptance | SAF changes HRTF or near-field algorithms; Omniphony needs a new spatial DSP primitive |
| [`leomccormack/SPARTA`](https://github.com/leomccormack/SPARTA) | [`5c728f72e434d3e2ac78c517aeb17f767060b271`](https://github.com/leomccormack/SPARTA/commit/5c728f72e434d3e2ac78c517aeb17f767060b271) | README.md; BinauraliserNF; 6DoFconv feature surface | near-field binauralization; SOFA and HRTF integration; time-varying convolution and head tracking | platform standard authority; Omniphony tuning | BinauraliserNF or 6DoFconv changes; Omniphony begins measured range-dependent HRTF work |
| [`hoene/libmysofa`](https://github.com/hoene/libmysofa) | [`90531bdb0b485dd36e8a14b3e37ce1c47c54d669`](https://github.com/hoene/libmysofa/commit/90531bdb0b485dd36e8a14b3e37ce1c47c54d669) | README.md; SOFA loading and neighbor-search API | AES69 SOFA ingestion; HRTF interpolation; radius-aware neighbor search | complete binaural renderer architecture; near-field perceptual model | Omniphony adds range-indexed SOFA support; libmysofa changes radial interpolation behavior |
| [`AppliedAcousticsChalmers/ReTiSAR`](https://github.com/AppliedAcousticsChalmers/ReTiSAR) | [`c1d752c68e091a726f9477537c095b7730fe5c5e`](https://github.com/AppliedAcousticsChalmers/ReTiSAR/commit/c1d752c68e091a726f9477537c095b7730fe5c5e) | README.md; _convolver.py; crossfade architecture | realtime filter switching; block convolution; artifact avoidance during transfer-function changes | low-level Rust callback safety; distance perception law | Omniphony revises HRTF transition architecture; ReTiSAR gains materially new motion handling |

## 6DoF and room reconstruction

| Repository | Inspected revision | Inspected surface | Return for | Not authority for | Revisit when |
| --- | --- | --- | --- | --- | --- |
| [`facebookresearch/6DoF-Auraliser`](https://github.com/facebookresearch/6DoF-Auraliser) | [`aa79a270da431b28e31f24e396a7b517a2df8d20`](https://github.com/facebookresearch/6DoF-Auraliser/commit/aa79a270da431b28e31f24e396a7b517a2df8d20) | README.md; SOFA and 6DoF architecture | listener translation and rotation; 6DoF source-receiver geometry; directional plus ambient rendering | active maintained product precedent; native Windows object ingress | Omniphony adds listener translation; a maintained successor appears |
| [`facebookresearch/BinauralSDM`](https://github.com/facebookresearch/BinauralSDM) | [`154834ab850753afc5fb878b017a9edbd3c19cf5`](https://github.com/facebookresearch/BinauralSDM/commit/154834ab850753afc5fb878b017a9edbd3c19cf5) | README.md; early versus late room-response decomposition | directional early reflections; BRIR generation; mixing-time split between early and late energy | live object rendering; Windows host architecture | Omniphony changes early or late room ownership; BinauralSDM adds new spatial manipulation methods |

## Objective audio validation

| Repository | Inspected revision | Inspected surface | Return for | Not authority for | Revisit when |
| --- | --- | --- | --- | --- | --- |
| [`DolbyLaboratories/SATS-software-audio-test-suite`](https://github.com/DolbyLaboratories/SATS-software-audio-test-suite) | [`1e3355c282b08fc5439776516b618633e3bea015`](https://github.com/DolbyLaboratories/SATS-software-audio-test-suite/commit/1e3355c282b08fc5439776516b618633e3bea015) | README.md; common test framework and reference-signal design | frequency response and THD+N validation; dynamic range and spectral validation; automated pass or fail audio regression design | spatial perception validation; current Dolby product behavior | Omniphony expands objective audio QA; Dolby revives or supersedes SATS |

## Re-entry law

- Prefer the pinned revision above when reconstructing the evidence that informed an existing decision.
- Refresh upstream before using it for a new claim when its revisit trigger has fired or the receiving Omniphony boundary changed.
- Preserve negative findings. A useful repository can still be the wrong authority for a neighboring question.
- Do not copy an upstream abstraction merely because it is mature. Translate only the mechanism that survives Omniphony's source-authority, realtime, fidelity, and physical-listening gates.
- Helix may retain a few cross-project lessons discovered here, but **this repository owns the detailed Omniphony upstream shelf**.
