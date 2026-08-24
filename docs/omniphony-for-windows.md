# Omniphony for Windows

This document defines the Windows product boundary for Omniphony.

## Product law

Omniphony for Windows is a system-wide spatial renderer for headphones.

It is intended to occupy the same broad role as proprietary headphone spatial-rendering systems while remaining free, open, inspectable, and source-authoritative.

```text
Windows audio
     ↓
Omniphony
     ↓
headphones
```

The user-facing product remains deliberately small:

```text
OmniphonySetup.exe
        ↓
one UAC elevation
        ↓
attach Omniphony to the selected Windows render endpoint
        ↓
headless system-wide rendering
        ↓
preference/manual-recovery tray icon
```

Normal use has:

- one installer executable;
- no virtual cable;
- no loopback host;
- no console;
- no taskbar audio-host window;
- no helper application that must remain open;
- one small notification-area icon for preferences;
- rendering that continues if the tray UI is closed.

The current Windows host uses unsigned user-mode APOs and enables Windows' unprotected AudioDG compatibility mode for that deployment. Previous machine state is recorded so rollback and uninstall can restore it.

A signed/componentized DriverStore route remains an optional deployment alternative. It does not define a different renderer architecture.

## One renderer across all source types

Omniphony is not a stereo enhancer plus a separate surround engine. It is one renderer whose behavior becomes more source-authoritative as the host supplies richer input.

```text
stereo
→ preserve the finished master
→ infer only missing spatial structure
→ enhance through Omniphony

5.1 / 7.1 / height PCM
→ preserve authored channels and positions
→ do less spatial inference
→ enhance through the same Omniphony renderer

8.1.4.4 static objects + dynamic XYZ objects
→ preserve supplied scene geometry directly
→ avoid reconstructing geometry already supplied
→ enhance through the same Omniphony renderer
```

> **The richer the source truth, the less Omniphony invents and the more authority it gives the source.**

Stereo is the most inference-heavy case. Native surround should be a stronger input to the same enhancement system because authored direction replaces guesswork. Raw static and dynamic spatial objects are richer again.

The final physical endpoint remains ordinary stereo headphones in every case.

## Windows audio topology

Conventional PCM and Windows Spatial Audio are different ingress paths into one portable source scene and one renderer.

```text
conventional PCM applications
        ↓
shared-mode stereo / 5.1 / 7.1 / height bed
        ↓
Omniphony stream SFX
        ↓
source-authoritative Omniphony scene
        ┐
        │
        ├──────────────→ Current spatial renderer → binaural stereo
        │
        ┘
Windows Spatial Audio applications
        ↓
static 8.1.4.4 objects + dynamic XYZ objects
        ↓
Omniphony spatial-object ingress
        ↓
same source scene → same renderer
        ↓
physical stereo endpoint
```

The conventional stream-SFX path is the current production baseline. When Windows exposes richer authored spatial truth before headphone rendering, Omniphony should ingest that richer representation rather than collapse it to stereo or reconstruct geometry that already exists.

The exact system-wide Windows boundary for receiving another application's raw Spatial Audio objects must be proven from supported interfaces. Opening an `ISpatialAudioClient` by itself is not evidence that another application's object stream has been intercepted.

Legacy virtual-device and process-loopback designs are migration history, not the product architecture.

## Canonical source scene

The internal static scene uses a 17-position **8.1.4.4** vocabulary:

```text
horizontal
FL FR C LFE SL SR BL BR BC

upper
TFL TFR TBL TBR

lower
BFL BFR BBL BBR
```

This is the ideal full fixed Windows scene vocabulary. It is a coordinate frame, not a claim that every source contains seventeen authored channels.

Every static lane carries source authority:

```text
AUTHORED  source or host supplied this signal / position
DERIVED   Omniphony inferred bounded support
EMPTY     no trustworthy signal is assigned
```

For authored 7.1:

```text
FL FR C LFE SL SR BL BR = AUTHORED
BC TFL TFR TBL TBR BFL BFR BBL BBR = EMPTY unless separately earned
```

For stereo, Omniphony may derive bounded support while preserving the finished master as the musical authority.

Dynamic XYZ objects remain continuous objects parallel to the static frame rather than being snapped into fixed anchors.

## Current renderer

The Windows APOs load `omniphony_realtime.dll`, which hosts the same Current renderer used by the portable engine. Windows packaging must not fork, simplify, or replace that renderer.

```text
source truth
        ↓
8.1.4.4-capable semantic scene
+ dynamic objects where supplied
        ↓
source authority
        ↓
Current 22-direction support shell
        ↓
HRTF / ITD / distance / room
        ↓
listener correction and safety
        ↓
binaural stereo
```

The 17-position scene and the 22-direction shell are deliberately different concepts. The scene is semantic source geometry. The shell is internal rendering geometry.

For authored Windows speaker beds, the realtime native-bed path maps supplied `WAVEFORMATEXTENSIBLE` channel masks to authored source positions and bypasses stereo inference for those channels. Missing canonical anchors remain empty. LFE remains semantically distinct from directional HRTF placement.

## Conventional Windows ingress

The accepted production path is:

```text
stereo client
→ Omniphony stream SFX
→ protected stereo Current path
→ stereo endpoint

or

authored 7.1 client
→ Omniphony stream SFX
→ native-bed realtime ABI
→ authored source coordinates
→ Current spatial renderer
→ stereo endpoint
```

The physically accepted native-surround boundary is:

```text
48 kHz / float32 / 7.1 shared client
        ↓
Omniphony stream SFX
        ↓
48 kHz / 32-bit / stereo endpoint
```

The endpoint remaining stereo is intentional. `IAudioClient::GetMixFormat` describes the endpoint/shared engine mix. Richer authored source input exists upstream of that final mix.

Acceptance therefore requires the multichannel client format itself to be supported and successfully initialized, not the physical endpoint mix to become multichannel.

A separate stereo endpoint EFX remains implemented as a transactional rollback and recovery floor. It is removed from the steady-state graph after the stream SFX passes native-surround acceptance so Current runs exactly once.

Authored 7.1.4 processing is also implemented and regression-tested inside the stream APO/native-bed path.

## Windows Spatial Audio ingress

Windows Spatial Audio is the richer target because it can carry predefined static spatial roles and dynamic 3-D objects.

The target representation is:

```text
8.1.4.4 static spatial roles
        +
dynamic spatial objects
with continuous XYZ trajectories
        ↓
Omniphony source-scene adapter
        ↓
existing Current spatial renderer
        ↓
stereo headphones
```

Raw Windows Spatial Audio object ingress is a required long-term host capability, but it is not claimed complete merely because the conventional SFX can accept multichannel PCM.

Completion requires evidence that Omniphony receives the source application's spatial representation **before** Windows Sonic, Dolby, DTS, or another headphone renderer destroys that geometry in a final binaural mix.

### Spatial-ingress acceptance conditions

A raw spatial path must prove that it:

1. receives the static spatial-object mask made available by the active Windows spatial interface;
2. preserves every received static role without remapping it through stereo inference;
3. preserves dynamic-object identity, PCM, and continuous 3-D position updates;
4. keeps static and dynamic source authority distinct;
5. feeds the same portable Omniphony scene and renderer used by conventional PCM;
6. returns cleanly to the ordinary Windows spatial path when Omniphony is disabled or unavailable;
7. keeps Windows-specific provider/capture concepts out of the portable renderer core.

A supported system boundary is required. The product architecture does not authorize injection into protected applications or a user-visible virtual-cable workaround solely to obtain object metadata.

## Already-binaural material

If another spatial renderer has already converted the source scene to final binaural stereo before Omniphony receives it, the source is no longer equivalent to raw surround or objects.

The safe target is:

```text
already-binaural stereo
        ↓
Omniphony spatial bypass
or explicitly validated non-spatial correction only
        ↓
headphones
```

A trustworthy host signal is required before automating this policy. Stereo channel count alone is not sufficient evidence that a source is already binaural.

## Realtime architecture

Windows realtime callbacks do not run the allocating renderer graph directly. The callback-facing layer uses bounded/preallocated transfer while a dedicated worker owns Current DSP.

The runtime retains:

- preallocated callback-facing rings;
- dedicated Current worker processing;
- time-aligned dry fallback;
- non-finite sanitization;
- linked peak safety;
- explicit create/destroy lifecycle testing;
- manifest, import, and ABI checks in CI.

Realtime callbacks must not perform filesystem I/O, network activity, device discovery, or research-time analysis.

A future spatial-object host must obey the same realtime law: Windows-facing callbacks exchange bounded preallocated object audio/state with the renderer worker rather than moving the allocating graph onto an OS realtime thread.

## Installer behavior

`OmniphonySetup.exe` performs the complete normal installation without asking the user to run scripts manually.

It:

1. validates the realtime renderer before endpoint mutation;
2. resolves and persists the selected physical endpoint identity;
3. records previous endpoint and AudioDG state;
4. establishes the stereo Current rollback floor;
5. verifies stereo processing and endpoint health;
6. registers the native stream APO;
7. attaches the format-changing SFX;
8. removes the temporary stereo EFX before final graph restart;
9. waits for the exact endpoint to return ACTIVE;
10. requires the physical endpoint to remain stereo;
11. requires a 48 kHz / float32 / 7.1 shared client format to report supported and successfully initialize;
12. keeps the stream SFX only after that client-boundary proof;
13. restores the stereo Current EFX if native-surround promotion fails;
14. starts the preference tray after successful setup.

The installed runtime is intentionally small:

```text
C:\Program Files\Omniphony\APO\OmniphonyAPO.dll
C:\Program Files\Omniphony\APO\OmniphonyStreamAPO.dll
C:\Program Files\Omniphony\APO\omniphony_realtime.dll
C:\Program Files\Omniphony\support\...
```

There is no resident audio-host application.

## Endpoint continuity

A physical endpoint may temporarily become inactive when a USB DAC is powered off, unplugged, restarted, or when Windows restarts audio services. That must not erase Omniphony's installation state.

Omniphony persists the verified endpoint identity and uses it for recovery. Installation and recovery must never deregister a previously working global APO merely because endpoint discovery temporarily returns no active device. The endpoint must become ACTIVE before Omniphony mutates endpoint FX state or declares the path healthy.

Normal power cycling is therefore endpoint availability, not product installation state. The current tray can request a finite manual Windows Audio graph reset after a surprise removal, while automatic hotplug recovery remains a product-hardening task. A genuinely new Windows endpoint identity after a driver or topology change may require reattachment.

## Tray contract

The notification-area icon is the normal UI surface for preferences.

The tray writes small preference state and exposes a finite manual Windows Audio graph reset. It does not carry the audio stream, and exiting it does not stop Current.

## Failure and uninstall law

Installation must leave ordinary Windows audio recoverable.

The transaction is deliberately staged: establish a proven stereo Current floor, attempt native-surround promotion, and remove that floor only after the stream SFX has passed acceptance. If the richer client-stream proof fails, the installer removes the SFX and restores the stereo Current EFX.

A failure before endpoint discovery must not dismantle a previously known installation. Rollback is successful only when restored endpoint state is verified.

Uninstall removes Omniphony's stream/endpoint attachments and runtime files and restores previous AudioDG state. It must not replace or uninstall the physical audio driver.

Any future spatial-object component must obey the same rollback law.

## Optional signed deployment

`windows_installer/endpoint_apo/production/` contains the componentized DriverStore/APO deployment work.

It may eventually provide a signed/protected distribution route, but it must preserve the same product contract:

```text
one installer
headless system-wide renderer
physical Windows endpoint remains the user's normal output
same Omniphony scene and renderer
```

## Product direction

The Windows host should grow upward in source authority without changing the product identity:

```text
stereo
→ native surround PCM
→ height beds
→ 8.1.4.4 static spatial scene
→ dynamic XYZ objects
```

Each richer ingress should reuse the same source scene, provenance rules, spatial character, and final binaural renderer.

The long-term target is a free and open Windows-wide spatial renderer that can receive whatever trustworthy spatial representation an application supplies and perform the final headphone render itself.
