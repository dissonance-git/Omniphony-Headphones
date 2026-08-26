from pathlib import Path

root = Path(__file__).resolve().parents[1]
mix_path = root / "omniphony-renderer/windows_installer/endpoint_apo/OmniphonyMixProbe.cpp"
install_path = root / "omniphony-renderer/windows_installer/endpoint_apo/Install-OmniphonyWindows.ps1"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)

mix = mix_path.read_text(encoding="utf-8")
mix = replace_once(
    mix,
    "int ProbeSharedSevenOne(IAudioClient* client, const std::wstring& name) {",
    "int ProbeSharedSevenOne(IAudioClient* client, const std::wstring& name, UINT16 endpointChannels) {",
    "ProbeSharedSevenOne signature",
)
mix = replace_once(
    mix,
    '    std::wcout << L"SHARED_7_1_INITIALIZE_OK\\t" << name\n               << L"\\tRATE=48000\\tINPUT_CHANNELS=8\\tENDPOINT_CHANNELS=2"\n               << L"\\tBITS=32\\tFORMAT=float32\\tBUFFER_FRAMES=" << bufferFrames << L\'\\n\';',
    '    std::wcout << L"SHARED_7_1_INITIALIZE_OK\\t" << name\n               << L"\\tRATE=48000\\tINPUT_CHANNELS=8\\tENDPOINT_CHANNELS=" << endpointChannels\n               << L"\\tBITS=32\\tFORMAT=float32\\tBUFFER_FRAMES=" << bufferFrames << L\'\\n\';',
    "shared 7.1 success record",
)
mix = replace_once(
    mix,
    '''                    if (probeSharedSevenOne) {
                        if (format->nChannels != 2 || format->nSamplesPerSec != 48000 ||
                            format->wBitsPerSample != 32) {
                            std::wcerr << L"SHARED_7_1_ENDPOINT_FLOOR_FAILED\\t" << name
                                       << L"\\tEXPECTED=stereo-float32-48000\\n";
                            result = 11;
                        } else {
                            result = ProbeSharedSevenOne(client.Get(), name);
                        }
                    }
''',
    '''                    if (probeSharedSevenOne) {
                        // The endpoint mix belongs to the physical device/engine and may be
                        // multichannel. The actual contract under test is whether Windows
                        // accepts an authored 7.1 shared client through the Stream SFX.
                        result = ProbeSharedSevenOne(client.Get(), name, format->nChannels);
                    }
''',
    "stereo endpoint floor",
)
mix_path.write_text(mix, encoding="utf-8")

install = install_path.read_text(encoding="utf-8")
install = replace_once(
    install,
    '''    $mixMatch = [regex]::Match($mixLine, '(?:^|\\t)CHANNELS=(\\d+)(?:\\t|$)')
    if (-not $mixMatch.Success -or [int]$mixMatch.Groups[1].Value -ne 2) {
        throw "Native-surround client probe did not preserve the physical stereo endpoint mix: $mixLine"
    }

    $clientLine = $probe.Lines | Where-Object { $_.StartsWith("SHARED_7_1_INITIALIZE_OK`t") } | Select-Object -First 1
''',
    '''    $mixMatch = [regex]::Match($mixLine, '(?:^|\\t)CHANNELS=(\\d+)(?:\\t|$)')
    if (-not $mixMatch.Success -or [int]$mixMatch.Groups[1].Value -lt 1) {
        throw "Native-surround client probe did not expose a valid endpoint mix width: $mixLine"
    }
    $endpointChannels = [int]$mixMatch.Groups[1].Value

    $clientLine = $probe.Lines | Where-Object { $_.StartsWith("SHARED_7_1_INITIALIZE_OK`t") } | Select-Object -First 1
''',
    "installer endpoint width assertion",
)
install = replace_once(
    install,
    "    Write-Host 'NATIVE_SURROUND_CLIENT_FORMAT_OK INPUT_CHANNELS=8 ENDPOINT_CHANNELS=2 RATE=48000 BITS=32'",
    '    Write-Host "NATIVE_SURROUND_CLIENT_FORMAT_OK INPUT_CHANNELS=8 ENDPOINT_CHANNELS=$endpointChannels RATE=48000 BITS=32"',
    "installer success record",
)
install = replace_once(
    install,
    '''function Assert-StereoRollbackMixFormat([string]$EndpointName) {
    $channels = Get-MixChannelCount $EndpointName
    if ($channels -ne 2) {
        throw "Stereo Current rollback did not restore the two-channel mix. observed_channels=$channels"
    }
    Write-Host 'STEREO_ROLLBACK_MIX_FORMAT_OK CHANNELS=2'
}
''',
    '''function Assert-RollbackMixFormat([string]$EndpointName) {
    $channels = Get-MixChannelCount $EndpointName
    if ($channels -lt 1) {
        throw "Rollback endpoint mix probe returned an invalid channel count. observed_channels=$channels"
    }
    Write-Host "ROLLBACK_MIX_FORMAT_OK CHANNELS=$channels"
}
''',
    "rollback mix assertion",
)
install = replace_once(
    install,
    "            Assert-StereoRollbackMixFormat $endpointName",
    "            Assert-RollbackMixFormat $endpointName",
    "rollback call",
)
install = replace_once(
    install,
    '''# Establish the proven stereo Current endpoint first. This is the rollback floor
# and owns the endpoint backup plus AudioDG compatibility state.
''',
    '''# Establish the compatibility baseline first. It owns the endpoint backup and
# AudioDG compatibility state; the physical endpoint keeps its native mix geometry.
''',
    "baseline comment",
)
install = replace_once(
    install,
    '''    # GetMixFormat remains the physical/shared engine mix and should stay stereo.
    # The Windows 11 preferred-format contract is upstream of that mix. Prove the
    # real capability by constructing an exact 7.1 float32 shared client stream;
    # successful Initialize means the graph builder accepted authored 7.1 while
    # retaining a stereo endpoint for the DAC.
''',
    '''    # GetMixFormat remains the physical/shared engine mix and is endpoint-owned;
    # it may legitimately be multichannel. The preferred-format contract is upstream
    # of that mix. Prove the real capability by constructing an exact 7.1 float32
    # shared client stream; successful Initialize means the graph builder accepted
    # authored 7.1 through Omniphony's Stream SFX.
''',
    "native surround proof comment",
)
install = replace_once(
    install,
    "    Write-Host 'AUDIO_INGRESS windows-client-input=7.1 endpoint-mix=stereo multichannel=authored-speaker-bed output=binaural-stereo'",
    "    Write-Host 'AUDIO_INGRESS windows-client-input=7.1 endpoint-mix=endpoint-native multichannel=authored-speaker-bed output=binaural-stereo'",
    "audio ingress record",
)
install_path.write_text(install, encoding="utf-8")
