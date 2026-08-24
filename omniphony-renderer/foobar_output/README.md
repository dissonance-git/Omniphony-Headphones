# Output: Omniphony

`foo_out_omniphony` is the foobar2000 frontend for the portable Omniphony
renderer family.

The user-visible selector is exactly:

```text
Output: Omniphony
```

The ordinary-stereo path forces the Foobar stream to 48 kHz stereo, renders
ordinary stereo through the same `omniphony_realtime.dll` Current path used by
Omniphony for Windows, and opens the current default physical endpoint as a
shared WASAPI RAW stream. RAW is an internal single-render guarantee: the
installed Omniphony SFX remains transparent for this already-rendered stream.

## Recovered-source session

The output component also owns a process-local, versioned source-session ABI for
trusted game-music decoders such as Retro VGM Compiler. A VGM or SPC input may
publish:

```text
causal source PCM
+ source identity / native route / timing evidence
+ ordered intra-block evidence changes
+ past-derived scene mix budget
+ the protected 48 kHz reference stereo block
```

The output renders that source scene through sibling `omniphony_source.dll`
FullSphere and queues the resulting binaural stereo. On the actual Foobar output
callback it substitutes that rendered block only if the stereo delivered by
Foobar still matches the protected decoder reference. This makes the routing
law explicit:

```text
ordinary stereo
→ Current
→ one binaural render

recovered VGM / SPC source scene
→ source-session FullSphere
→ one binaural render

recovered-source metadata + modified/intervening stereo
→ source-session match rejected
→ actual delivered stereo enters Current
→ one binaural render
```

A DSP, resampler, callback-boundary change, queue underrun, ABI mismatch, source
render failure, or stale session therefore cannot silently bypass the Foobar
signal path or produce a second spatialization pass. Source-session packets are
bounded and may be consumed across different Foobar callback partitions, but a
reference mismatch clears the queued source render and fails closed to Current.

The source-session layer knows nothing about chip families. Genesis FM/DAC/PSG,
SNES S-DSP, and future VGM chips remain decoder/source-model responsibilities;
they all converge on the same generic Omniphony source ABI after their own source
truth and additivity contracts are proven.

Do not install an artifact merely because this project compiled. Required proof
before listening delivery includes:

- exact visible-name contract;
- x64 Foobar SDK build;
- sibling realtime-DLL ABI and Current startup smoke;
- sibling source-DLL ABI 0.4 and FullSphere regression tests;
- shared RAW client initialization against the selected machine;
- source-session exact-reference bypass and ordinary-stereo fallback;
- seek, track-change, pause, callback repartition, device-loss and fallback tests;
- physical listening validation after the engineering gates are green.
