# Omniphony Spatial Companion

This directory is the packaged-identity experiment for the Windows Spatial Sound ownership boundary.

## Why it exists

The registry/COM provider path is internally valid, but a physical Windows 11 test reached the documented setter and Windows returned `LicenseNotValidForAudioEndpoint`. Windows requires the spatial-format owner context for `SetDefaultSpatialAudioFormatAsync`, and `SpatialAudioFormatConfiguration` is the notification surface for a spatial-format companion app.

Windows' own `SpatialAudioLicenseSrv` binary exposes the same discovery chain now represented here: `GetMediaComponentPackageInfo`, the `Windows.Media.Audio.SpatialAudioFormatSubtype` category, `windows.mediaPlayback`, and an AppService broker that sends `GetLicenseInfo` / `GetRuntimeParameters` requests containing the endpoint and spatial subtype.

This package therefore provides:

- a real MSIX package identity (`Omniphony.SpatialCompanion`);
- a `windows.mediaPlayback` extension whose codec/subtype name is Omniphony's spatial format GUID `{4BD75423-A66C-4586-B782-1FCBBDF2AE74}`;
- a `windows.appService` named `OmniphonySpatialLicense`;
- a Windows Runtime background component that answers the observed license-broker message shape;
- a packaged CLI that can register the current media-extension package on Windows 11 24H2+, report license/configuration changes, discover the default multimedia render endpoint through Core Audio, inspect endpoint spatial state, and ask Windows to select Omniphony from package identity;
- a one-command physical ownership verifier;
- a development build/signing script and CI artifact.

## Evidence boundary

The `windows.mediaPlayback` declaration is schema-valid, and Windows' own spatial-license broker exposes the matching media-component discovery vocabulary. This materially narrows the previous ownership gap, but CI cannot prove endpoint licensing or ownership because hosted runners do not expose the physical spatial endpoint state used by `SetDefaultSpatialAudioFormatAsync`.

`RegisterMediaExtensionPackage` is a documented Windows 11 24H2 full-trust API. Successful media-extension registration proves that Windows accepted the current package family through that API for the current user. It does **not** by itself prove that the Omniphony spatial subtype is licensed for a particular audio endpoint.

The experiment is successful only if a physical machine proves that Windows recognizes the package/subtype association strongly enough for the packaged setter to move beyond `LicenseNotValidForAudioEndpoint` and Windows then retains Omniphony as the default spatial format.

The production `OmniphonySetup.exe` must not claim this package solves raw Spatial Audio ingress until that physical ownership test passes. Stream availability remains a separate boundary after ownership succeeds.

## Development package

CI builds a signed development MSIX plus its public development certificate and this README. The certificate is disposable and is only for sideload testing.

After installing the certificate and MSIX, the app execution alias exposes:

```text
OmniphonySpatialCompanion.exe identity
OmniphonySpatialCompanion.exe register
OmniphonySpatialCompanion.exe notify
OmniphonySpatialCompanion.exe status <endpoint-id>
OmniphonySpatialCompanion.exe select <endpoint-id>
OmniphonySpatialCompanion.exe verify-default
```

`identity` must print `PACKAGE_IDENTITY_OK 1`.

## One-command physical ownership gate

On Windows 11 24H2 or newer, with the intended headphones/output selected as the default multimedia render endpoint, run:

```text
OmniphonySpatialCompanion.exe verify-default
```

This command performs the bounded sequence itself:

```text
package identity
→ RegisterMediaExtensionPackage
→ spatial license/configuration notification
→ Core Audio default multimedia render-endpoint discovery
→ pre-selection spatial state
→ SetDefaultSpatialAudioFormatAsync
→ retained-default verification
```

It changes the endpoint's default spatial format if Windows accepts Omniphony. It does not change Omniphony's DSP or renderer state.

A successful physical ownership gate ends with:

```text
MEDIA_EXTENSION_REGISTERED	1
DEFAULT_RENDER_ENDPOINT_DISCOVERED	1
WINDOWS_SETTER_ACCEPTED_CONTEXT	1
OMNIPHONY_SPATIAL_DEFAULT_SET	1
SPATIAL_OWNERSHIP_VERIFY_DEFAULT_OK	1
```

If Windows still rejects the package/subtype relationship, the command preserves the exact setter status, including `LicenseNotValidForAudioEndpoint`, and ends without the success bit. That failure remains evidence about the ownership/license boundary rather than being collapsed into a generic setup error.

The earlier explicit commands remain available for diagnosis when the one-command gate fails. Successful MSIX installation, media-extension registration, AppService activation, or endpoint discovery alone is not sufficient.
