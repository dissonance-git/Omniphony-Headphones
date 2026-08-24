# Output: Omniphony

`foo_out_omniphony` is the foobar2000 frontend for the portable Omniphony
renderer family.

The user-visible selector is exactly:

```text
Output: Omniphony
```

The initial implementation forces the Foobar stream to 48 kHz stereo, renders
ordinary stereo through the same `omniphony_realtime.dll` Current path used by
Omniphony for Windows, and opens the current default physical endpoint as a
shared WASAPI RAW stream. RAW is an internal single-render guarantee: the
installed Omniphony SFX remains transparent for this already-rendered stream.

This output is a replaceable frontend, not another renderer. The next bounded
integration is an in-process source-session service so Retro VGM Compiler's VGM
and SPC inputs can provide causal source scenes to FullSphere while ordinary
Foobar inputs continue through stereo Current.

Do not install an artifact merely because this project compiled. Required proof
before listening delivery includes:

- exact visible-name contract;
- x64 Foobar SDK build;
- sibling realtime-DLL ABI and Current startup smoke;
- shared RAW client initialization against the selected machine;
- source-aware already-rendered bypass before VGM/SPC FullSphere is enabled;
- seek, track-change, pause, device-loss and fallback tests.

