# Omniphony Spatial Companion

This directory is the packaged-identity experiment for the Windows Spatial Sound ownership boundary.

## Why it exists

The registry/COM provider path is internally valid, but a physical Windows 11 test reached the documented setter and Windows returned `LicenseNotValidForAudioEndpoint`. Microsoft documents `SetDefaultSpatialAudioFormatAsync` as callable by the app that owns the format, and `SpatialAudioFormatConfiguration` as the notification surface for a spatial-format companion app.

Windows' own `SpatialAudioLicenseSrv` trace/binary metadata also exposes an AppService broker. It sends `GetLicenseInfo` / `GetRuntimeParameters` requests containing the endpoint and spatial subtype, then reads `Status`, `ExpirationDate`, `IsAudioRendererCapable`, and `LaunchUri` response fields.

This package therefore provides:

- a real MSIX package identity (`Omniphony.SpatialCompanion`);
- a `windows.appService` named `OmniphonySpatialLicense`;
- a Windows Runtime background component that answers the observed license-broker message shape;
- a packaged CLI that calls `ReportLicenseChangedAsync`, `ReportConfigurationChangedAsync`, and `SetDefaultSpatialAudioFormatAsync` from package identity;
- a development build/signing script and CI artifact.

## Evidence boundary

This package does **not** claim that a self-signed MSIX automatically becomes the owner of an arbitrary Windows Spatial Sound subtype. No public Microsoft manifest schema found so far documents the binding that maps a third-party spatial subtype GUID to a package family/AppService, and Microsoft may gate that association through a Store/partner entitlement.

The experiment is successful only if a physical machine proves that Windows recognizes the package as the Omniphony format owner and the packaged `select` command moves beyond `LicenseNotValidForAudioEndpoint`.

The production `OmniphonySetup.exe` must not claim this package solves raw Spatial Audio ingress until that physical ownership test passes.

## Development package

CI builds a signed development MSIX plus its public development certificate. The certificate is disposable and is only for sideload testing.

After installing the certificate and MSIX, the app execution alias exposes:

```text
OmniphonySpatialCompanion.exe identity
OmniphonySpatialCompanion.exe notify
OmniphonySpatialCompanion.exe status <endpoint-id>
OmniphonySpatialCompanion.exe select <endpoint-id>
```

`identity` must print `PACKAGE_IDENTITY_OK 1`. The decisive physical test is the packaged `select` result, not successful MSIX installation by itself.
