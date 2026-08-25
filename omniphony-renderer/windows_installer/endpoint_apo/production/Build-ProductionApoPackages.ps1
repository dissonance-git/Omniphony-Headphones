param(
    [Parameter(Mandatory = $true)]
    [string]$CaptureJson,

    [Parameter(Mandatory = $true)]
    [string]$ApoDll,

    [Parameter(Mandatory = $true)]
    [string]$RealtimeDll,

    [Parameter(Mandatory = $true)]
    [string]$ProductionProbe,

    [string]$OutputRoot = '',
    [string]$CertificateThumbprint = '',
    [switch]$MachineCertificateStore,
    [string]$TimestampUrl = '',
    [string]$Inf2CatOs = '10_CO_X64,10_NI_X64,10_GE_X64,10_25H2_X64',
    [switch]$SkipCatalogs
)

$ErrorActionPreference = 'Stop'
$productionRoot = $PSScriptRoot

function Resolve-RequiredFile([string]$Path, [string]$Label) {
    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Label was not found: $Path"
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

function Find-WindowsKitTool([string]$Name) {
    $fromPath = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source
    if ($fromPath) { return $fromPath }

    $kitsRoot = 'C:\Program Files (x86)\Windows Kits\10'
    if (-not (Test-Path -LiteralPath $kitsRoot)) { return '' }
    return Get-ChildItem -LiteralPath $kitsRoot -Recurse -Filter $Name -File -ErrorAction SilentlyContinue |
        # Microsoft ships Windows Kit command-line tools under either x64 or
        # x86; tool architecture does not constrain the package architecture.
        Where-Object { $_.FullName -match '\\(x64|x86)\\' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}

function Invoke-NativeChecked([string]$FilePath, [string[]]$Arguments, [string]$Label) {
    Write-Host "RUN $Label"
    Write-Host "$FilePath $($Arguments -join ' ')"
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

function Verify-AuthenticodeSignature([string]$SignTool, [string]$Path, [string]$Label) {
    Invoke-NativeChecked $SignTool @('verify', '/pa', '/v', $Path) "Verify $Label signature"
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid) {
        throw "$Label Authenticode verification is not Valid: $($signature.Status) $($signature.StatusMessage)"
    }
    Write-Host "SIGNATURE_VERIFIED $Label $($signature.SignerCertificate.Thumbprint)"
}

function Get-FileRecord([string]$Path) {
    $item = Get-Item -LiteralPath $Path
    return [ordered]@{
        Name = $item.Name
        Length = [long]$item.Length
        Sha256 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

$capture = Resolve-RequiredFile $CaptureJson 'target capture JSON'
$apo = Resolve-RequiredFile $ApoDll 'OmniphonyAPO.dll'
$realtime = Resolve-RequiredFile $RealtimeDll 'omniphony_realtime.dll'
$probe = Resolve-RequiredFile $ProductionProbe 'OmniphonyProductionProbe.exe'
$componentTemplate = Resolve-RequiredFile (Join-Path $productionRoot 'OmniphonyApoComponent.inx') 'component INF template'
$generator = Resolve-RequiredFile (Join-Path $productionRoot 'generate_extension_inf.py') 'extension INF generator'

if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
    $OutputRoot = Join-Path (Get-Location) 'omniphony-production-packages'
}
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)
$componentRoot = Join-Path $OutputRoot 'component'
$extensionRoot = Join-Path $OutputRoot 'extension'
$diagnosticsRoot = Join-Path $OutputRoot 'diagnostics'

if (Test-Path -LiteralPath $OutputRoot) {
    Remove-Item -LiteralPath $OutputRoot -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $componentRoot, $extensionRoot, $diagnosticsRoot | Out-Null

$componentInf = Join-Path $componentRoot 'OmniphonyApoComponent.inf'
$extensionInf = Join-Path $extensionRoot 'OmniphonyApoExtension.inf'
$boundCapture = Join-Path $OutputRoot 'target-capture.json'
$packagedProbe = Join-Path $diagnosticsRoot 'OmniphonyProductionProbe.exe'
Copy-Item -LiteralPath $componentTemplate -Destination $componentInf -Force
Copy-Item -LiteralPath $apo -Destination (Join-Path $componentRoot 'OmniphonyAPO.dll') -Force
Copy-Item -LiteralPath $realtime -Destination (Join-Path $componentRoot 'omniphony_realtime.dll') -Force
Copy-Item -LiteralPath $probe -Destination $packagedProbe -Force
Copy-Item -LiteralPath $capture -Destination $boundCapture -Force

# Generate from the copy that will travel with the package so the extension INF,
# installer collision checks and package manifest all refer to the same witness.
python $generator $boundCapture $extensionInf
if ($LASTEXITCODE -ne 0) { throw "extension INF generation failed with exit code $LASTEXITCODE" }
if (-not (Test-Path -LiteralPath $extensionInf)) { throw 'extension INF generator produced no file' }

$infverif = Find-WindowsKitTool 'InfVerif.exe'
if (-not $infverif) { throw 'InfVerif.exe was not found. Install a current Windows 11 WDK.' }
Invoke-NativeChecked $infverif @('/w', '/v', $componentInf) 'InfVerif component package'
Invoke-NativeChecked $infverif @('/w', '/v', $extensionInf) 'InfVerif extension package'

$componentApo = Join-Path $componentRoot 'OmniphonyAPO.dll'
$componentRealtime = Join-Path $componentRoot 'omniphony_realtime.dll'
$signTool = ''
$signaturesVerified = $false
if (-not [string]::IsNullOrWhiteSpace($CertificateThumbprint)) {
    $signTool = Find-WindowsKitTool 'signtool.exe'
    if (-not $signTool) { throw 'x64 signtool.exe was not found. Install a current Windows SDK/WDK.' }

    $signArgs = @('sign', '/fd', 'SHA256')
    if ($MachineCertificateStore) { $signArgs += '/sm' }
    $signArgs += @('/s', 'My', '/sha1', $CertificateThumbprint)
    if (-not [string]::IsNullOrWhiteSpace($TimestampUrl)) {
        $signArgs += @('/tr', $TimestampUrl, '/td', 'SHA256')
    }

    # PE payloads are signed and verified before catalog generation. The probe
    # is not AudioDG-loaded, but shipping one unsigned executable beside a
    # trusted driver candidate makes physical acceptance harder to reason about.
    Invoke-NativeChecked $signTool ($signArgs + @($componentApo)) 'Sign OmniphonyAPO.dll'
    Invoke-NativeChecked $signTool ($signArgs + @($componentRealtime)) 'Sign omniphony_realtime.dll'
    Invoke-NativeChecked $signTool ($signArgs + @($packagedProbe)) 'Sign OmniphonyProductionProbe.exe'
    Verify-AuthenticodeSignature $signTool $componentApo 'OmniphonyAPO.dll'
    Verify-AuthenticodeSignature $signTool $componentRealtime 'omniphony_realtime.dll'
    Verify-AuthenticodeSignature $signTool $packagedProbe 'OmniphonyProductionProbe.exe'
}

if (-not $SkipCatalogs) {
    $inf2cat = Find-WindowsKitTool 'Inf2Cat.exe'
    if (-not $inf2cat) { throw 'Inf2Cat.exe was not found. Install a current Windows 11 WDK.' }

    # These are independent PnP packages and therefore receive independent catalogs.
    Invoke-NativeChecked $inf2cat @("/driver:$componentRoot", "/os:$Inf2CatOs") 'Inf2Cat component package'
    Invoke-NativeChecked $inf2cat @("/driver:$extensionRoot", "/os:$Inf2CatOs") 'Inf2Cat extension package'

    $componentCat = Resolve-RequiredFile (Join-Path $componentRoot 'OmniphonyApo.cat') 'component catalog'
    $extensionCat = Resolve-RequiredFile (Join-Path $extensionRoot 'OmniphonyApoExtension.cat') 'extension catalog'

    if (-not [string]::IsNullOrWhiteSpace($CertificateThumbprint)) {
        $catSignArgs = @('sign', '/fd', 'SHA256')
        if ($MachineCertificateStore) { $catSignArgs += '/sm' }
        $catSignArgs += @('/s', 'My', '/sha1', $CertificateThumbprint)
        if (-not [string]::IsNullOrWhiteSpace($TimestampUrl)) {
            $catSignArgs += @('/tr', $TimestampUrl, '/td', 'SHA256')
        }
        Invoke-NativeChecked $signTool ($catSignArgs + @($componentCat)) 'Sign component catalog'
        Invoke-NativeChecked $signTool ($catSignArgs + @($extensionCat)) 'Sign extension catalog'
        Verify-AuthenticodeSignature $signTool $componentCat 'OmniphonyApo.cat'
        Verify-AuthenticodeSignature $signTool $extensionCat 'OmniphonyApoExtension.cat'
        $signaturesVerified = $true
    }
}

$files = New-Object System.Collections.Generic.List[object]
Get-ChildItem -LiteralPath $OutputRoot -Recurse -File | Sort-Object FullName | ForEach-Object {
    $rootPrefix = $OutputRoot.TrimEnd('\') + '\'
    if (-not $_.FullName.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Package file escaped output root: $($_.FullName)"
    }
    $relative = $_.FullName.Substring($rootPrefix.Length)
    $record = Get-FileRecord $_.FullName
    $files.Add([ordered]@{
        Path = $relative
        Length = $record.Length
        Sha256 = $record.Sha256
    })
}

$manifest = [ordered]@{
    Schema = 'omniphony.windows.apo-package-build.v2'
    BuiltAtUtc = [DateTime]::UtcNow.ToString('o')
    Capture = Get-FileRecord $boundCapture
    CapturePath = 'target-capture.json'
    ProductionProbePath = 'diagnostics\OmniphonyProductionProbe.exe'
    Inf2CatOs = $Inf2CatOs
    CatalogsGenerated = -not [bool]$SkipCatalogs
    CertificateThumbprint = if ([string]::IsNullOrWhiteSpace($CertificateThumbprint)) { $null } else { $CertificateThumbprint.ToUpperInvariant() }
    SignaturesVerified = [bool]$signaturesVerified
    Files = $files.ToArray()
}
$manifestPath = Join-Path $OutputRoot 'package-manifest.json'
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $manifestPath -Encoding UTF8

Write-Host ''
Write-Host 'OMNIPHONY_PRODUCTION_PACKAGE_BUILD_OK 1'
Write-Host "OUTPUT_ROOT $OutputRoot"
Write-Host "TARGET_CAPTURE $boundCapture"
Write-Host "PRODUCTION_PROBE $packagedProbe"
Write-Host "COMPONENT_INF $componentInf"
Write-Host "EXTENSION_INF $extensionInf"
Write-Host "MANIFEST $manifestPath"
if ([string]::IsNullOrWhiteSpace($CertificateThumbprint)) {
    Write-Warning 'Packages are not certificate-signed by this build. They are not proven suitable for protected AudioDG.'
} elseif (-not $signaturesVerified -and -not $SkipCatalogs) {
    throw 'A signing certificate was supplied but package signature verification did not complete.'
} elseif ($SkipCatalogs) {
    Write-Warning 'PE signatures were verified, but catalogs were skipped; this is not a complete signed driver-package candidate.'
} else {
    Write-Host 'OMNIPHONY_PACKAGE_SIGNATURES_VERIFIED 1'
    Write-Warning 'Valid Authenticode signatures are necessary evidence only. They do not prove protected AudioDG or Microsoft driver-trust acceptance on the target machine.'
}
