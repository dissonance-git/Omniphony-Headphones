# Omniphony Windows endpoint APO implementation

This directory contains the native Windows APO host used by Omniphony's conventional PCM route.

The canonical durable Windows product contract is [`../../../docs/omniphony-for-windows.md`](../../../docs/omniphony-for-windows.md). This README owns only local implementation orientation, component roles, and diagnostic entry points. It must not create a second Windows product contract.

## Local graph shape

The intended promoted conventional path is:

```text
Windows client audio
stereo / authored multichannel PCM
        ↓
OmniphonyStreamAPO.dll
        ↓
omniphony_realtime.dll
        ↓
Current renderer
        ↓
final binaural headphone render
        ↓
selected physical endpoint
```

The renderer runs inside the Windows audio graph. It does not require a virtual cable, a loopback host, or a foreground audio application to remain open.

## APO roles

### `OmniphonyStreamAPO.dll`

Stable CLSID:

```text
{07D403D9-8A98-43EF-8C28-8651756D83BE}
```

The Stream APO is the promoted steady-state conventional PCM route. It can receive stereo or richer authored PCM where Windows supplies it, route source-authoritative content into the realtime renderer, and return the final headphone render through the selected endpoint graph.

It must not assume that every valid headphone endpoint exposes a stereo `GetMixFormat`. Endpoint mix geometry is Windows/driver state and may be stereo or multichannel. Installation and health checks preserve and report the endpoint's verified baseline geometry instead of forcing a project-selected channel count merely to make a diagnostic pass.

### `OmniphonyAPO.dll`

Stable CLSID:

```text
{A9333BFE-39C1-40FD-B4B0-ECC591410B47}
```

This is the supported stereo EFX rollback floor. It is useful when a compatible stereo graph needs a known-good recovery path, but it is not meant to process simultaneously with the promoted Stream APO.

### `omniphony_realtime.dll`

This is the realtime bridge into the Current renderer. Realtime behavior and latency law are owned by [`../../../docs/realtime-control-contract.md`](../../../docs/realtime-control-contract.md).

## Steady-state invariant

After successful promotion, the conventional Windows route should satisfy:

```text
SFX = OmniphonyStreamAPO
EFX = absent
system effects = enabled
one Omniphony spatial render = exactly once
```

Current must not run through both the Stream SFX and endpoint EFX on the same audible route.

## Endpoint geometry is not the render identity

Do not equate:

```text
GetMixFormat channel count
```

with:

```text
number of final acoustic ears
source authority
proof that the SFX is active
proof that the renderer is inactive
```

The physical product target is binaural headphone output, but Windows may expose a multichannel endpoint mix format. A healthy installer should snapshot the baseline endpoint geometry before mutation and verify that it remains acceptable afterward.

`OmniphonyMixProbe.exe` without special flags reports the endpoint's actual mix format. That result is evidence about endpoint geometry only.

A shared-client 7.1 probe is a separate ingress test. It must not be treated as authoritative on a machine where the probe itself refuses to run solely because the endpoint mix is not stereo. In particular, a diagnostic failure of the form:

```text
SHARED_7_1_ENDPOINT_FLOOR_FAILED
EXPECTED=stereo-float32-48000
```

on an otherwise healthy multichannel endpoint is a limitation of that probe's precondition, not proof that the Stream APO or Current renderer is inactive.

## Runtime proof

Registry state and live process state are separate evidence layers.

Basic attachment check:

```powershell
OmniphonyApoCtl.exe status
```

A healthy promoted graph should report the Stream SFX CLSID and no Omniphony EFX.

While real playback is active, stronger evidence is that the Windows audio engine has actually loaded both:

```text
OmniphonyStreamAPO.dll
omniphony_realtime.dll
```

inside the active `audiodg.exe` process.

Module presence proves that Windows instantiated/loaded the Omniphony path. It does not by itself prove that every sample received the intended transform. Sample-path tests, route-clean physical playback, and listening remain distinct evidence states.

## Windows Sonic and Spatial Sound

The conventional Current path must work with **Windows Sonic disabled**. It is not supposed to need Sonic to create the Omniphony presentation.

Windows Spatial Sound provider selection is a separate host seam. Enabling Sonic or another provider must not create a second hidden dependency or double-render route. If an external provider has already produced final binaural audio, the one-render/bypass law in the canonical Windows contract applies.

Similar sound with Sonic enabled and disabled can support the conclusion that ordinary Current is independent of Sonic. It does not prove that Omniphony is intercepting raw Windows Spatial Audio objects.

## Installed layout

The normal installer places the relevant host files under:

```text
C:\Program Files\Omniphony\
├─ APO\
│  ├─ OmniphonyAPO.dll
│  ├─ OmniphonyStreamAPO.dll
│  └─ omniphony_realtime.dll
└─ support\
   ├─ OmniphonyApoCtl.exe
   ├─ OmniphonyMixProbe.exe
   ├─ OmniphonyEndpointCtl.exe
   ├─ OmniphonySpatialProbe.exe
   └─ OmniphonySpatialProviderProbe.exe
```

The exact installer transaction, rollback policy, and provider boundary are owned by the canonical Windows contract and executable installer/tests.

## Diagnostics

Useful local read-only or status-oriented checks include:

```powershell
OmniphonyApoCtl.exe status
OmniphonyMixProbe.exe "<endpoint-name>"
OmniphonySpatialProbe.exe
OmniphonySpatialProviderProbe.exe
```

`OmniphonySpatialProbe.exe` interrogates public `ISpatialAudioClient` capability. It does not prove that Omniphony receives another application's objects.

`OmniphonySpatialProviderProbe.exe` observes provider-related registry surfaces. Registry observation is not a public provider contract and is not evidence of raw object ingress.

## Evidence ladder

Keep these separate:

```text
APO source builds
≠ renderer / realtime tests pass
≠ COM registration exists
≠ endpoint SFX attachment exists
≠ AudioDG instantiates OmniphonyStreamAPO.dll
≠ AudioDG loads omniphony_realtime.dll
≠ intended source samples enter the renderer
≠ intended samples are transformed exactly once
≠ physical endpoint receives the intended render
≠ physical listening confirms the percept
```

The local implementation should make each layer easier to prove without turning one convenient probe into a universal endpoint assumption.
