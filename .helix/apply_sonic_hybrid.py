from pathlib import Path

root = Path.cwd()
source = root / 'omniphony-renderer/windows_installer/endpoint_apo/OmniphonyStreamAPO.cpp'
cmake = root / 'omniphony-renderer/windows_installer/endpoint_apo/CMakeLists.txt'
installer = root / 'omniphony-renderer/windows_installer/endpoint_apo/Install-OmniphonyAdaptive.ps1'


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f'{label}: expected one anchor, found {count}')
    return text.replace(old, new, 1)


text = source.read_text(encoding='utf-8').replace('\r\n', '\n')

text = replace_once(
    text,
    '#include <ksmedia.h>\n\n#include "omniphony_realtime.h"',
    '''#include <ksmedia.h>\n#include <mmdeviceapi.h>\n#include <wrl/client.h>\n\n#include <winrt/base.h>\n#include <winrt/Windows.Foundation.h>\n#include <winrt/Windows.Media.Audio.h>\n\n#include "omniphony_realtime.h"''',
    'spatial headers',
)

text = replace_once(
    text,
    'HINSTANCE g_module = nullptr;\nvolatile LONG g_factoryLocks = 0;\n\n'
    'bool ReadAudioFormat(IAudioMediaType* mediaType, UNCOMPRESSEDAUDIOFORMAT& format) noexcept {',
    r'''HINSTANCE g_module = nullptr;
volatile LONG g_factoryLocks = 0;

// Hybrid ownership rule:
//   Spatial Sound OFF -> Omniphony owns the pre-mix SFX binaural reduction.
//   Spatial Sound ON  -> preserve the authored stream here; Windows Spatial
//                        Sound renders first and the stereo-only Omniphony EFX
//                        may then provide the post-spatial Current enclosure.
// The endpoint handed to APOInitSystemEffects2/3 is the final device in the
// collection. Query only the ACTIVE spatial format so a stale/default provider
// selection does not bypass Omniphony while Spatial Sound is actually off.
bool ExternalSpatialRendererActive(UINT32 dataSize, BYTE* data) noexcept {
    if (!data) return false;

    IMMDeviceCollection* devices = nullptr;
    BOOL discoveryOnly = FALSE;
    if (dataSize == sizeof(APOInitSystemEffects3)) {
        auto* init = reinterpret_cast<APOInitSystemEffects3*>(data);
        devices = init->pDeviceCollection;
        discoveryOnly = init->InitializeForDiscoveryOnly;
    } else if (dataSize == sizeof(APOInitSystemEffects2)) {
        auto* init = reinterpret_cast<APOInitSystemEffects2*>(data);
        devices = init->pDeviceCollection;
        discoveryOnly = init->InitializeForDiscoveryOnly;
    } else {
        return false;
    }
    if (discoveryOnly || !devices) return false;

    try {
        UINT count = 0;
        if (FAILED(devices->GetCount(&count)) || count == 0) return false;

        Microsoft::WRL::ComPtr<IMMDevice> endpoint;
        if (FAILED(devices->Item(count - 1, endpoint.ReleaseAndGetAddressOf())) || !endpoint) {
            return false;
        }

        LPWSTR rawId = nullptr;
        if (FAILED(endpoint->GetId(&rawId)) || !rawId) return false;
        const winrt::hstring endpointId{rawId};
        CoTaskMemFree(rawId);

        const auto configuration =
            winrt::Windows::Media::Audio::SpatialAudioDeviceConfiguration::GetForDeviceId(endpointId);
        return !configuration.ActiveSpatialAudioFormat().empty();
    } catch (...) {
        // Never break graph creation because the state query failed. Failing
        // toward the normal Omniphony SFX is safer than silence.
        return false;
    }
}

bool ReadAudioFormat(IAudioMediaType* mediaType, UNCOMPRESSEDAUDIOFORMAT& format) noexcept {''',
    'active spatial helper',
)

text = replace_once(
    text,
    r'''bool IsRawBypassFormat(const UNCOMPRESSEDAUDIOFORMAT& format) noexcept {
    return IsFloat32Format(format) && format.dwSamplesPerFrame == 2;
}''',
    r'''bool IsRawBypassFormat(const UNCOMPRESSEDAUDIOFORMAT& format) noexcept {
    if (!IsFloat32Format(format) || format.dwSamplesPerFrame == 0 ||
        format.dwSamplesPerFrame > 18) {
        return false;
    }
    return format.dwSamplesPerFrame == 2 || format.dwChannelMask != 0;
}''',
    'bypass format',
)

text = replace_once(
    text,
    '''    return IsRawBypassFormat(input) && IsRawBypassFormat(output) &&\n           input.fFramesPerSecond == output.fFramesPerSecond &&\n           input.dwBytesPerSampleContainer == output.dwBytesPerSampleContainer &&''',
    '''    return IsRawBypassFormat(input) && IsRawBypassFormat(output) &&\n           input.fFramesPerSecond == output.fFramesPerSecond &&\n           input.dwSamplesPerFrame == output.dwSamplesPerFrame &&\n           input.dwBytesPerSampleContainer == output.dwBytesPerSampleContainer &&''',
    'bypass pair channel identity',
)

text = replace_once(
    text,
    '''class OmniphonyStreamAPO final : public CBaseAudioProcessingObject,\n                                 public IAudioSystemEffects,''',
    '''class OmniphonyStreamAPO final : public CBaseAudioProcessingObject,\n                                 public IAudioSystemEffects2,''',
    'IAudioSystemEffects2 base',
)

text = replace_once(
    text,
    r'''        } else if (IsEqualIID(riid, __uuidof(IAudioSystemEffects))) {
            *object = static_cast<IAudioSystemEffects*>(this);''',
    r'''        } else if (IsEqualIID(riid, __uuidof(IAudioSystemEffects)) ||
                   IsEqualIID(riid, __uuidof(IAudioSystemEffects2))) {
            *object = static_cast<IAudioSystemEffects2*>(this);''',
    'system effects QI',
)

text = replace_once(
    text,
    '        rawBypass_ = IsEqualGUID(processingMode, AUDIO_SIGNALPROCESSINGMODE_RAW);',
    '''        rawBypass_ = IsEqualGUID(processingMode, AUDIO_SIGNALPROCESSINGMODE_RAW);\n        externalSpatialBypass_ = !rawBypass_ && ExternalSpatialRendererActive(dataSize, data);''',
    'initialize ownership',
)

# Every existing rawBypass_ branch is also the safe same-format identity path
# for an active external spatial renderer. Keep the flags distinct so future
# diagnostics can tell actual RAW apart from Sonic/Dolby/DTS ownership.
text = text.replace('if (rawBypass_) {', 'if (rawBypass_ || externalSpatialBypass_) {')
text = text.replace('(rawBypass_\n                ? IsRawBypassPair', '((rawBypass_ || externalSpatialBypass_)\n                ? IsRawBypassPair')
# Close the extra parenthesis introduced in the conditional expression above.
text = text.replace(': IsSupportedFormatPair(inputFormat, outputFormat));', ': IsSupportedFormatPair(inputFormat, outputFormat));')

text = replace_once(
    text,
    '''    bool rawBypass_ = false;\n    IUnknown* outer_ = nullptr;''',
    '''    bool rawBypass_ = false;\n    bool externalSpatialBypass_ = false;\n    IUnknown* outer_ = nullptr;''',
    'ownership member',
)

source.write_text(text, encoding='utf-8', newline='\n')

ct = cmake.read_text(encoding='utf-8').replace('\r\n', '\n')
ct = replace_once(
    ct,
    'target_link_libraries(OmniphonyStreamAPO PRIVATE ole32 uuid advapi32 audioeng AudioBaseProcessingObjectV140 legacy_stdio_definitions audiomediatypecrt)',
    'target_link_libraries(OmniphonyStreamAPO PRIVATE ole32 uuid advapi32 audioeng AudioBaseProcessingObjectV140 legacy_stdio_definitions audiomediatypecrt windowsapp)',
    'windowsapp link',
)
cmake.write_text(ct, encoding='utf-8', newline='\n')

ps = installer.read_text(encoding='utf-8').replace('\r\n', '\n')
ps = replace_once(
    ps,
    r'''    & $ctl attach-native-sfx-id $endpointId
    if ($LASTEXITCODE -ne 0) { throw "Native SFX attachment failed: $LASTEXITCODE" }
    & $ctl detach-id $endpointId
    if ($LASTEXITCODE -ne 0) { throw "Could not remove duplicate Current EFX after SFX promotion: $LASTEXITCODE" }''',
    r'''    & $ctl attach-native-sfx-id $endpointId
    if ($LASTEXITCODE -ne 0) { throw "Native SFX attachment failed: $LASTEXITCODE" }

    # On a multichannel endpoint keep the stereo-only EFX installed as the
    # post-Spatial-Sound half of the hybrid route. It is transparent while the
    # endpoint graph remains multichannel. The SFX yields to an active Windows
    # spatial renderer, allowing the EFX to receive that renderer's stereo
    # output without stacking two Omniphony Current passes.
    if ($before.Channels -eq 2) {
        & $ctl detach-id $endpointId
        if ($LASTEXITCODE -ne 0) { throw "Could not remove EFX on stereo baseline endpoint: $LASTEXITCODE" }
    }''',
    'hybrid attach',
)

ps = replace_once(
    ps,
    r'''    $fx = Get-FxStatus $endpointId
    if ($fx.Efx -ne '<absent>') { throw "Duplicate endpoint EFX remains attached: $($fx.Efx)" }
    if (-not [string]::Equals($fx.Sfx, $nativeSfxClsid, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Native stream SFX is not the sole Omniphony path. observed_sfx=$($fx.Sfx)"
    }
    Write-Host 'SINGLE_RENDER_PATH_OK EFX=0 SFX=1' ''',
    r'''    $fx = Get-FxStatus $endpointId
    if (-not [string]::Equals($fx.Sfx, $nativeSfxClsid, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Native stream SFX is not attached. observed_sfx=$($fx.Sfx)"
    }
    if ($before.Channels -eq 2) {
        if ($fx.Efx -ne '<absent>') { throw "Stereo endpoint unexpectedly retained EFX: $($fx.Efx)" }
        Write-Host 'CONDITIONAL_RENDER_PATH_OK BASELINE=stereo EFX=0 SFX=1'
    }
    else {
        if (-not [string]::Equals($fx.Efx, $currentEfxClsid, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Hybrid post-spatial EFX is not attached. observed_efx=$($fx.Efx)"
        }
        Write-Host "CONDITIONAL_RENDER_PATH_OK BASELINE=$($before.Channels)ch EFX=1 SFX=1 ACTIVE_CURRENT_PASSES_EXPECTED=1"
        Write-Host 'HYBRID_ROUTE spatial-off=SFX-current spatial-on=SFX-identity->Windows-Spatial->EFX-current-if-stereo'
    }''',
    'hybrid verify',
)

ps = replace_once(
    ps,
    '    Write-Host "AUDIO_INGRESS endpoint-mix-channels=$($before.Channels) stream-sfx=current output=binaural-stereo"\n    Write-Host \'OMNIPHONY_INSTALL_STAGE adaptive-native-sfx-active\'',
    '    Write-Host "AUDIO_INGRESS endpoint-mix-channels=$($before.Channels) conditional-hybrid=1"\n    Write-Host \'OMNIPHONY_INSTALL_STAGE adaptive-sonic-reference-hybrid\'',
    'install stage',
)

installer.write_text(ps, encoding='utf-8', newline='\n')
print('SONIC_REFERENCE_HYBRID_PATCH_OK 1')
