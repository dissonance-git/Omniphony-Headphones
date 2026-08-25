[CmdletBinding()]
param(
    [string]$OutputDirectory = '',
    [switch]$Sign
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$SourceRoot = $PSScriptRoot
$RepoRoot = (Resolve-Path (Join-Path $SourceRoot '..\..\..')).Path
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $RepoRoot 'dist\Omniphony-Spatial-Companion'
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$BuildRoot = Join-Path $RepoRoot 'build\spatial-companion'
$NativeBuild = Join-Path $BuildRoot 'native'
$PackageRoot = Join-Path $BuildRoot 'package'

Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $BuildRoot
New-Item -ItemType Directory -Force -Path $NativeBuild, $PackageRoot, $OutputDirectory | Out-Null

Write-Host 'Building packaged companion executable...'
cmake -S $SourceRoot -B $NativeBuild -A x64
if ($LASTEXITCODE -ne 0) { throw "Companion CMake configure failed: $LASTEXITCODE" }
cmake --build $NativeBuild --config Release --parallel
if ($LASTEXITCODE -ne 0) { throw "Companion native build failed: $LASTEXITCODE" }

$msbuild = Get-Command msbuild.exe -ErrorAction SilentlyContinue
if ($null -eq $msbuild) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) { throw 'MSBuild was not found.' }
    $msbuildPath = & $vswhere -latest -products * -requires Microsoft.Component.MSBuild -find 'MSBuild\**\Bin\MSBuild.exe' | Select-Object -First 1
    if ([string]::IsNullOrWhiteSpace($msbuildPath)) { throw 'MSBuild was not found by vswhere.' }
} else {
    $msbuildPath = $msbuild.Source
}

Write-Host 'Building AppService Windows Runtime component...'
$serviceProject = Join-Path $SourceRoot 'AppServiceComponent\OmniphonySpatialLicenseService.vcxproj'
& $msbuildPath $serviceProject '/m' '/p:Configuration=Release' '/p:Platform=x64' '/v:minimal'
if ($LASTEXITCODE -ne 0) { throw "AppService component build failed: $LASTEXITCODE" }

$companionExe = Join-Path $NativeBuild 'Release\OmniphonySpatialCompanion.exe'
$serviceOutput = Join-Path $SourceRoot 'AppServiceComponent\x64\Release'
$serviceDll = Join-Path $serviceOutput 'OmniphonySpatialLicenseService.dll'
$serviceWinmd = Join-Path $serviceOutput 'OmniphonySpatialLicenseService.winmd'
foreach ($path in @($companionExe, $serviceDll, $serviceWinmd)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing package payload: $path" }
}

Copy-Item -LiteralPath $companionExe -Destination $PackageRoot -Force
Copy-Item -LiteralPath $serviceDll -Destination $PackageRoot -Force
Copy-Item -LiteralPath $serviceWinmd -Destination $PackageRoot -Force
Copy-Item -LiteralPath (Join-Path $SourceRoot 'Package.appxmanifest') -Destination $PackageRoot -Force

$assets = Join-Path $PackageRoot 'Assets'
New-Item -ItemType Directory -Force -Path $assets | Out-Null
Add-Type -AssemblyName System.Drawing
function New-Logo([string]$Path, [int]$Size) {
    $bitmap = New-Object System.Drawing.Bitmap($Size, $Size)
    try {
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.Clear([System.Drawing.Color]::FromArgb(24, 24, 27))
            $fontSize = [Math]::Max(12, [int]($Size * 0.52))
            $font = New-Object System.Drawing.Font('Segoe UI', $fontSize, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
            try {
                $brush = [System.Drawing.Brushes]::White
                $format = New-Object System.Drawing.StringFormat
                $format.Alignment = [System.Drawing.StringAlignment]::Center
                $format.LineAlignment = [System.Drawing.StringAlignment]::Center
                $graphics.DrawString('O', $font, $brush, [System.Drawing.RectangleF]::new(0, 0, $Size, $Size), $format)
            } finally { $font.Dispose() }
        } finally { $graphics.Dispose() }
        $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $bitmap.Dispose() }
}
New-Logo (Join-Path $assets 'StoreLogo.png') 50
New-Logo (Join-Path $assets 'Square44x44Logo.png') 44
New-Logo (Join-Path $assets 'Square150x150Logo.png') 150

$kitsBin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$makeAppx = Get-ChildItem -Path $kitsBin -Recurse -Filter makeappx.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\x64\\makeappx\.exe$' } |
    Sort-Object FullName -Descending |
    Select-Object -First 1
if ($null -eq $makeAppx) { throw 'MakeAppx.exe was not found in the Windows SDK.' }

$msix = Join-Path $OutputDirectory 'OmniphonySpatialCompanion.msix'
Remove-Item -Force -ErrorAction SilentlyContinue $msix
& $makeAppx.FullName pack /d $PackageRoot /p $msix /o
if ($LASTEXITCODE -ne 0) { throw "MakeAppx failed: $LASTEXITCODE" }

if ($Sign) {
    Write-Host 'Signing development MSIX with a disposable Omniphony development certificate...'
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=Omniphony Development' -CertStoreLocation 'Cert:\CurrentUser\My' -KeyExportPolicy Exportable -HashAlgorithm sha256 -NotAfter (Get-Date).AddYears(2)
    $passwordText = [Guid]::NewGuid().ToString('N')
    $password = ConvertTo-SecureString -String $passwordText -AsPlainText -Force
    $pfx = Join-Path $BuildRoot 'OmniphonySpatialCompanion.pfx'
    $cer = Join-Path $OutputDirectory 'OmniphonySpatialCompanion.cer'
    Export-PfxCertificate -Cert $cert -FilePath $pfx -Password $password | Out-Null
    Export-Certificate -Cert $cert -FilePath $cer -Type CERT | Out-Null

    $signTool = Get-ChildItem -Path $kitsBin -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1
    if ($null -eq $signTool) { throw 'SignTool.exe was not found in the Windows SDK.' }
    & $signTool.FullName sign /fd SHA256 /f $pfx /p $passwordText $msix
    if ($LASTEXITCODE -ne 0) { throw "SignTool failed: $LASTEXITCODE" }
    Remove-Item -Force $pfx
    Remove-Item -Path ("Cert:\CurrentUser\My\" + $cert.Thumbprint) -Force -ErrorAction SilentlyContinue
}

$hash = (Get-FileHash -LiteralPath $msix -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host "SPATIAL_COMPANION_MSIX $msix"
Write-Host "SPATIAL_COMPANION_SHA256 $hash"
Write-Host 'SPATIAL_COMPANION_PACKAGE_IDENTITY Omniphony.SpatialCompanion'
Write-Host 'SPATIAL_COMPANION_APP_SERVICE OmniphonySpatialLicense'
Write-Host 'SPATIAL_COMPANION_FORMAT_GUID {4BD75423-A66C-4586-B782-1FCBBDF2AE74}'
Write-Host 'SPATIAL_COMPANION_EXTERNAL_OWNERSHIP_GATE_UNPROVEN 1'
