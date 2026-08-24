# Omniphony native Windows APO path

This directory contains the native Windows host for Omniphony.

It attaches directly to the selected physical render endpoint. It does **not** create an Omniphony playback device, require a virtual cable, or keep an audio-host application running.

## Product role

The Windows host exists to make Omniphony a system-wide spatial renderer rather than an application-specific effect.

```text
Windows source audio
stereo / 5.1 / 7.1 / richer source representations
        ↓
Omniphony Windows ingress
        ↓
source-authoritative Omniphony scene
        ↓
Current spatial renderer
        ↓
binaural stereo
        ↓
physical headphone endpoint
```

Stereo and surround are not separate products. Stereo uses the same renderer with more inference because the source contains less explicit geometry. Authored surround uses less inference because channel position is already known.

## Deployment contract

The steady-state Windows path is the format-changing stream SFX:

```text
OmniphonySetup.exe
        ↓
one UAC elevation
        ↓
recover / verify the selected physical endpoint
        ↓
install OmniphonyAPO.dll + OmniphonyStreamAPO.dll + omniphony_realtime.dll
        ↓
establish stereo Current EFX rollback floor
        ↓
register and attach Omniphony stream SFX
        ↓
remove temporary EFX after native-surround acceptance
        ↓
restart Windows Audio
        ↓
verify multichannel shared-client initialization
while endpoint remains stereo
        ↓
Current renders headlessly
```

The unsigned user-mode APO route is the supported quick-install path. The componentized/signed DriverStore machinery under `production/` remains an optional future deployment route.

## Accepted Windows surround baseline

The host has physically verified the following topology:

```text
authored Windows client stream
48 kHz / float32 / 7.1
        ↓
OmniphonyStreamAPO.dll
        ↓
native-bed authored source path
        ↓
omniphony_realtime.dll
        ↓
canonical 8.1.4.4-capable source scene
        ↓
Current 22-direction support shell
        ↓
cascaded binaural / measured HRTF
        ↓
listener correction + linked peak safety
        ↓
48 kHz / 32-bit / stereo physical endpoint
```

The endpoint remaining stereo is intentional. A stereo `GetMixFormat` result does not mean the SFX failed to receive richer source input. Acceptance tests the client boundary directly by asking Windows to support and initialize an exact 48 kHz float32 7.1 shared stream while the physical endpoint remains two-channel.

Expected accepted-state evidence includes:

```text
SHARED_7_1_FORMAT_SUPPORTED
SHARED_7_1_INITIALIZE_OK
NATIVE_SURROUND_CLIENT_FORMAT_OK INPUT_CHANNELS=8 ENDPOINT_CHANNELS=2
NATIVE_SURROUND_SFX 1
NATIVE_SURROUND_EFX 0
```

## Installed layout

```text
C:\Program Files\Omniphony\
├─ APO\
│  ├─ OmniphonyAPO.dll
│  ├─ OmniphonyStreamAPO.dll
│  └─ omniphony_realtime.dll
├─ support\
│  ├─ Install-OmniphonyAPO.ps1
│  ├─ Install-OmniphonyWindows.ps1
│  ├─ Uninstall-OmniphonyAPO.ps1
│  ├─ Uninstall-OmniphonyWindows.ps1
│  ├─ OmniphonyApoCtl.exe
│  ├─ OmniphonyMixProbe.exe
│  ├─ OmniphonyEndpointCtl.exe
│  ├─ OmniphonySpatialProbe.exe
│  ├─ OmniphonySpatialProviderProbe.exe
│  └─ OmniphonyTray.ps1
├─ LICENSE
└─ Inno Setup uninstaller files
```

Legacy virtual-device and loopback-host files are migration history and are removed during upgrade.

## APO roles

Omniphony retains two Windows APOs with different responsibilities.

### `OmniphonyStreamAPO.dll`

This is the promoted steady-state path after successful installation.

It:

- implements `IAudioProcessingObjectPreferredFormatSupport`;
- can prefer 7.1 input for a stereo-rendering headphone endpoint;
- preserves the stereo Current path when the client is stereo;
- routes authored multichannel beds through the native-bed realtime ABI;
- accepts differing input/output channel counts;
- reduces richer source input to stereo before the physical endpoint;
- keeps the physical endpoint at its normal two-channel format.

Stable stream APO CLSID:

```text
{07D403D9-8A98-43EF-8C28-8651756D83BE}
```

### `OmniphonyAPO.dll`

This is the stereo Current EFX and rollback floor.

It:

- processes supported stereo float32 graphs;
- provides recovery if native-surround promotion fails;
- is attached during installation to establish a known-good floor;
- is removed after the stream SFX passes real client-boundary acceptance.

Stable endpoint APO CLSID:

```text
{A9333BFE-39C1-40FD-B4B0-ECC591410B47}
```

Steady-state invariant after successful native-surround installation:

```text
SFX = OmniphonyStreamAPO
EFX = absent
```

Current must not run in both APOs simultaneously.

## Source authority

The canonical scene is the 17-position 8.1.4.4 vocabulary. The 22-direction Current shell is downstream rendering geometry, not a replacement scene model.

For conventional authored 7.1:

```text
FL FR C LFE SL SR BL BR = AUTHORED
BC TFL TFR TBL TBR BFL BFR BBL BBR = EMPTY unless separately earned
```

For stereo, Current retains evidence-bounded derived spatial support while protecting the finished master.

For authored 7.1.4, the stream APO/native-bed path is regression-tested with twelve input channels.

The ideal full fixed Windows spatial vocabulary remains **8.1.4.4 / 17 positions**. Dynamic spatial objects are richer still because they carry continuous XYZ positions rather than fixed speaker anchors.

Raw 8.1.4.4 object ingress and dynamic-object interception are separate host capabilities and are not implied by the conventional SFX baseline.

## Headless UI contract

The notification-area icon is the normal preference surface.

The tray does not host or transport audio. Closing it does not stop Current because processing remains inside the Windows APO path.

## Unsigned AudioDG compatibility mode

The current installer uses the Windows compatibility path required by these unsigned user-mode APOs:

```text
HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Audio
DisableProtectedAudioDG = 1
```

The installer snapshots the previous value before changing it. Rollback and uninstall restore that saved state.

Equalizer APO is **not** a runtime dependency. Omniphony only uses a similar broad unsigned-APO deployment tradeoff.

## Install transaction

`OmniphonySetup.exe` performs the complete normal installation. The user should not need to run the PowerShell helpers manually.

During install or upgrade it:

1. validates the realtime renderer before endpoint mutation;
2. stops obsolete Omniphony host/tray instances;
3. repairs known Omniphony global APO registration when recovering an existing endpoint;
4. resolves the current render endpoint and persists its stable identity;
5. snapshots endpoint state and the previous AudioDG protection value;
6. establishes and proves the stereo Current EFX rollback floor;
7. registers the native stream APO;
8. waits for the exact physical endpoint to be ACTIVE;
9. cleans interrupted older SFX state;
10. attaches the Omniphony stream SFX;
11. removes the temporary stereo EFX before graph restart;
12. restarts the Windows audio graph;
13. verifies the physical endpoint remains stereo;
14. proves an exact 48 kHz / float32 / 7.1 shared client format is supported;
15. proves that 7.1 shared client can initialize;
16. keeps the SFX only after those facts are true;
17. otherwise restores and verifies the stereo Current EFX;
18. starts the tray icon after successful setup.

A failed preflight must not deregister a previously working global APO while an endpoint still references it. Rollback success is based on verified restored state, not merely attempted cleanup commands.

## Endpoint continuity

A physical DAC being powered off, unplugged, or temporarily absent must not be treated as uninstalling Omniphony.

Omniphony persists the verified endpoint identity. Recovery can repair project-owned global APO registration, reassert a known endpoint when appropriate, restart the Windows audio graph, and require the exact endpoint to become ACTIVE before FX mutation continues.

Normal power cycling of the same endpoint preserves installation state. The current tray exposes a manual Windows Audio graph reset after surprise removal; automatic hotplug recovery remains future hardening. A genuinely new endpoint identity after a driver or topology change may require reattachment.

## Fixed-latency safety lane

The Current realtime path reports a fixed **40 ms / 1920-frame** host delay at 48 kHz. The same timeline is maintained for a delayed-dry safety lane. Worker underruns substitute the matching delayed dry frame rather than jumping forward in time. Late Current frames are discarded before Current resumes.

## Diagnostics

The normal product is the EXE installer, but retained support helpers can diagnose a machine when needed:

```powershell
OmniphonyApoCtl.exe status
OmniphonyMixProbe.exe "<endpoint-name>"
OmniphonyMixProbe.exe --shared-7.1 "<endpoint-name>"
```

The test payload also contains two read-only Spatial Audio research probes:

```powershell
OmniphonySpatialProbe.exe
OmniphonySpatialProviderProbe.exe
```

`OmniphonySpatialProbe.exe` interrogates the active endpoint's public `ISpatialAudioClient` capability: static-object mask/positions, dynamic-object capacity, and supported object format. It does not open another application's stream.

`OmniphonySpatialProviderProbe.exe` observes installed spatial-provider registry surfaces without writing them. Provider-registry output remains experimental evidence because Microsoft does not document that registry surface as a public third-party provider contract.

## Optional signed DriverStore route

`production/` retains the componentized Windows APO work: target capture, generated extension INF, DriverStore component package, catalog/signing hooks, transactional install/rollback, and protected-AudioDG probes.

This is a deployment alternative, not a different renderer architecture.

## Evidence states

Keep engineering evidence distinct:

```text
APO source builds
≠ Current DSP contracts pass
≠ realtime ABI tests pass
≠ COM activation succeeds
≠ endpoint association succeeds
≠ SFX registry attachment succeeds
≠ exact 7.1 shared format reports supported
≠ exact 7.1 shared client Initialize succeeds
≠ an application actually populates the richer stream
≠ physical listening confirms the result
```

Raw Windows Spatial Audio object ingress adds another evidence layer beyond conventional shared-client PCM.
