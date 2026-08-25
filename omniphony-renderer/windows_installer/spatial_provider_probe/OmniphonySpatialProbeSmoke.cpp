#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <audioclient.h>
#include <spatialaudioclient.h>
#include <unknwn.h>

#include <cstdint>
#include <iomanip>
#include <iostream>

namespace {

constexpr GUID kProbeClsid = {
    0xf3cdf827, 0x20c4, 0x405e, {0xa4, 0x30, 0x8f, 0x73, 0x93, 0x43, 0xfc, 0x89}};
constexpr UINT32 kExpectedDynamicObjects = 16;

constexpr std::uint32_t Bits(AudioObjectType type) noexcept {
    return static_cast<std::uint32_t>(type);
}

constexpr AudioObjectType kExpectedStaticMask = static_cast<AudioObjectType>(
    Bits(AudioObjectType_FrontLeft) |
    Bits(AudioObjectType_FrontRight) |
    Bits(AudioObjectType_FrontCenter) |
    Bits(AudioObjectType_LowFrequency) |
    Bits(AudioObjectType_SideLeft) |
    Bits(AudioObjectType_SideRight) |
    Bits(AudioObjectType_BackLeft) |
    Bits(AudioObjectType_BackRight) |
    Bits(AudioObjectType_TopFrontLeft) |
    Bits(AudioObjectType_TopFrontRight) |
    Bits(AudioObjectType_TopBackLeft) |
    Bits(AudioObjectType_TopBackRight) |
    Bits(AudioObjectType_BottomFrontLeft) |
    Bits(AudioObjectType_BottomFrontRight) |
    Bits(AudioObjectType_BottomBackLeft) |
    Bits(AudioObjectType_BottomBackRight) |
    Bits(AudioObjectType_BackCenter));

using DllGetClassObjectFn = HRESULT(STDAPICALLTYPE*)(REFCLSID, REFIID, LPVOID*);

int Fail(const wchar_t* stage, HRESULT hr) {
    std::wcerr << L"SPATIAL_PROVIDER_SMOKE_FAIL stage=" << stage
               << L" hr=0x" << std::hex << std::uppercase
               << static_cast<unsigned long>(hr) << std::dec << L"\n";
    return 1;
}

bool IsExpectedObjectFormat(const WAVEFORMATEX* format) noexcept {
    return format != nullptr &&
           format->wFormatTag == WAVE_FORMAT_IEEE_FLOAT &&
           format->nChannels == 1 &&
           format->nSamplesPerSec == 48'000 &&
           format->wBitsPerSample == 32 &&
           format->nBlockAlign == sizeof(float) &&
           format->nAvgBytesPerSec == 48'000 * sizeof(float);
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc != 2 || !argv[1] || !*argv[1]) {
        std::wcerr << L"usage: OmniphonySpatialProbeSmoke <OmniphonySpatialProbe.dll>\n";
        return 2;
    }

    HMODULE module = LoadLibraryW(argv[1]);
    if (!module) {
        std::wcerr << L"SPATIAL_PROVIDER_SMOKE_FAIL stage=LoadLibrary error="
                   << GetLastError() << L"\n";
        return 1;
    }

    const auto getClassObject = reinterpret_cast<DllGetClassObjectFn>(
        GetProcAddress(module, "DllGetClassObject"));
    if (!getClassObject) {
        FreeLibrary(module);
        std::wcerr << L"SPATIAL_PROVIDER_SMOKE_FAIL stage=GetProcAddress\n";
        return 1;
    }

    IClassFactory* factory = nullptr;
    HRESULT hr = getClassObject(
        kProbeClsid,
        IID_IClassFactory,
        reinterpret_cast<void**>(&factory));
    if (FAILED(hr) || !factory) {
        FreeLibrary(module);
        return Fail(L"DllGetClassObject", hr);
    }

    ISpatialAudioClient* spatial = nullptr;
    hr = factory->CreateInstance(
        nullptr,
        __uuidof(ISpatialAudioClient),
        reinterpret_cast<void**>(&spatial));
    factory->Release();
    if (FAILED(hr) || !spatial) {
        FreeLibrary(module);
        return Fail(L"CreateInstance(ISpatialAudioClient)", hr);
    }

    AudioObjectType mask = AudioObjectType_None;
    hr = spatial->GetNativeStaticObjectTypeMask(&mask);
    if (FAILED(hr)) {
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"GetNativeStaticObjectTypeMask", hr);
    }
    if (Bits(mask) != Bits(kExpectedStaticMask)) {
        std::wcerr << L"SPATIAL_PROVIDER_SMOKE_FAIL stage=static-mask expected=0x"
                   << std::hex << Bits(kExpectedStaticMask) << L" actual=0x"
                   << Bits(mask) << std::dec << L"\n";
        spatial->Release();
        FreeLibrary(module);
        return 1;
    }

    float x = 0.0f;
    float y = 0.0f;
    float z = 0.0f;
    hr = spatial->GetStaticObjectPosition(AudioObjectType_TopFrontLeft, &x, &y, &z);
    if (FAILED(hr) || !(y > 0.0f && z < 0.0f)) {
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"GetStaticObjectPosition(TopFrontLeft)", FAILED(hr) ? hr : E_FAIL);
    }
    hr = spatial->GetStaticObjectPosition(AudioObjectType_BottomBackRight, &x, &y, &z);
    if (FAILED(hr) || !(y < 0.0f && z > 0.0f)) {
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"GetStaticObjectPosition(BottomBackRight)", FAILED(hr) ? hr : E_FAIL);
    }

    UINT32 dynamicCount = 999;
    hr = spatial->GetMaxDynamicObjectCount(&dynamicCount);
    if (FAILED(hr) || dynamicCount != kExpectedDynamicObjects) {
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"GetMaxDynamicObjectCount", FAILED(hr) ? hr : E_FAIL);
    }

    IAudioFormatEnumerator* formats = nullptr;
    hr = spatial->GetSupportedAudioObjectFormatEnumerator(&formats);
    if (FAILED(hr) || !formats) {
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"GetSupportedAudioObjectFormatEnumerator", hr);
    }

    UINT32 formatCount = 0;
    hr = formats->GetCount(&formatCount);
    if (FAILED(hr) || formatCount != 1) {
        formats->Release();
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"IAudioFormatEnumerator::GetCount", FAILED(hr) ? hr : E_FAIL);
    }

    WAVEFORMATEX* format = nullptr;
    hr = formats->GetFormat(0, &format);
    formats->Release();
    if (FAILED(hr) || !IsExpectedObjectFormat(format)) {
        if (format) {
            CoTaskMemFree(format);
        }
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"IAudioFormatEnumerator::GetFormat", FAILED(hr) ? hr : E_FAIL);
    }

    hr = spatial->IsAudioObjectFormatSupported(format);
    if (FAILED(hr)) {
        CoTaskMemFree(format);
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"IsAudioObjectFormatSupported", hr);
    }

    UINT32 frameCount = 0;
    hr = spatial->GetMaxFrameCount(format, &frameCount);
    CoTaskMemFree(format);
    if (FAILED(hr) || frameCount != 480) {
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"GetMaxFrameCount", FAILED(hr) ? hr : E_FAIL);
    }

    hr = spatial->IsSpatialAudioStreamAvailable(
        __uuidof(ISpatialAudioObjectRenderStream), nullptr);
    if (hr != SPTLAUDCLNT_E_STREAM_NOT_AVAILABLE) {
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"IsSpatialAudioStreamAvailable expected unavailable", hr);
    }

    void* stream = reinterpret_cast<void*>(1);
    hr = spatial->ActivateSpatialAudioStream(
        nullptr,
        __uuidof(ISpatialAudioObjectRenderStream),
        &stream);
    if (hr != SPTLAUDCLNT_E_STREAM_NOT_AVAILABLE || stream != nullptr) {
        spatial->Release();
        FreeLibrary(module);
        return Fail(L"ActivateSpatialAudioStream expected unavailable", hr);
    }

    spatial->Release();
    FreeLibrary(module);

    std::wcout << L"SPATIAL_PROVIDER_COM_OK 1\n";
    std::wcout << L"SPATIAL_PROVIDER_INTERFACE ISpatialAudioClient\n";
    std::wcout << L"SPATIAL_PROVIDER_STATIC_8_1_4_4_OK 1\n";
    std::wcout << L"SPATIAL_PROVIDER_OBJECT_FORMAT FLOAT32_48000_MONO\n";
    std::wcout << L"SPATIAL_PROVIDER_MAX_DYNAMIC_OBJECTS " << kExpectedDynamicObjects << L"\n";
    std::wcout << L"SPATIAL_PROVIDER_STREAM_AVAILABLE 0\n";
    std::wcout << L"SPATIAL_PROVIDER_CAPABILITY_SMOKE_OK 1\n";
    return 0;
}
