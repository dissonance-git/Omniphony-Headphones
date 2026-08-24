#include <windows.h>
#include <mmdeviceapi.h>
#include <spatialaudioclient.h>
#include <wrl/client.h>

#include <cstdint>
#include <iomanip>
#include <iostream>

using Microsoft::WRL::ComPtr;

namespace {

struct StaticRole {
    AudioObjectType type;
    const wchar_t* name;
};

constexpr StaticRole kCanonicalStaticRoles[] = {
    {AudioObjectType_FrontLeft, L"FrontLeft"},
    {AudioObjectType_FrontRight, L"FrontRight"},
    {AudioObjectType_FrontCenter, L"FrontCenter"},
    {AudioObjectType_LowFrequency, L"LowFrequency"},
    {AudioObjectType_SideLeft, L"SideLeft"},
    {AudioObjectType_SideRight, L"SideRight"},
    {AudioObjectType_BackLeft, L"BackLeft"},
    {AudioObjectType_BackRight, L"BackRight"},
    {AudioObjectType_BackCenter, L"BackCenter"},
    {AudioObjectType_TopFrontLeft, L"TopFrontLeft"},
    {AudioObjectType_TopFrontRight, L"TopFrontRight"},
    {AudioObjectType_TopBackLeft, L"TopBackLeft"},
    {AudioObjectType_TopBackRight, L"TopBackRight"},
    {AudioObjectType_BottomFrontLeft, L"BottomFrontLeft"},
    {AudioObjectType_BottomFrontRight, L"BottomFrontRight"},
    {AudioObjectType_BottomBackLeft, L"BottomBackLeft"},
    {AudioObjectType_BottomBackRight, L"BottomBackRight"},
};

bool HasRole(AudioObjectType mask, AudioObjectType role) noexcept {
    const auto maskBits = static_cast<std::uint32_t>(mask);
    const auto roleBits = static_cast<std::uint32_t>(role);
    return (maskBits & roleBits) == roleBits;
}

int Fail(const wchar_t* stage, HRESULT hr) {
    std::wcerr << L"error=" << stage << L",hr=0x"
               << std::hex << std::uppercase << static_cast<unsigned long>(hr)
               << std::dec << L"\n";
    return 1;
}

class ComApartment final {
public:
    ComApartment() noexcept : status_(CoInitializeEx(nullptr, COINIT_MULTITHREADED)) {}
    ~ComApartment() {
        if (SUCCEEDED(status_)) {
            CoUninitialize();
        }
    }

    HRESULT status() const noexcept { return status_; }

private:
    HRESULT status_;
};

} // namespace

int wmain() {
    ComApartment com;
    if (FAILED(com.status())) {
        return Fail(L"CoInitializeEx", com.status());
    }

    ComPtr<IMMDeviceEnumerator> deviceEnumerator;
    HRESULT hr = CoCreateInstance(
        __uuidof(MMDeviceEnumerator),
        nullptr,
        CLSCTX_ALL,
        IID_PPV_ARGS(deviceEnumerator.GetAddressOf()));
    if (FAILED(hr)) {
        return Fail(L"CoCreateInstance(MMDeviceEnumerator)", hr);
    }

    ComPtr<IMMDevice> endpoint;
    hr = deviceEnumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, endpoint.GetAddressOf());
    if (FAILED(hr)) {
        return Fail(L"GetDefaultAudioEndpoint", hr);
    }

    ComPtr<ISpatialAudioClient> spatialClient;
    hr = endpoint->Activate(
        __uuidof(ISpatialAudioClient),
        CLSCTX_INPROC_SERVER,
        nullptr,
        reinterpret_cast<void**>(spatialClient.GetAddressOf()));
    if (FAILED(hr)) {
        std::wcout << L"spatial_client=unavailable\n";
        return Fail(L"IMMDevice::Activate(ISpatialAudioClient)", hr);
    }

    std::wcout << L"spatial_client=available\n";

    AudioObjectType nativeMask = AudioObjectType_None;
    hr = spatialClient->GetNativeStaticObjectTypeMask(&nativeMask);
    if (FAILED(hr)) {
        return Fail(L"GetNativeStaticObjectTypeMask", hr);
    }

    std::wcout << L"native_static_mask=0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(nativeMask)
               << std::dec << L"\n";

    UINT32 maxDynamicObjects = 0;
    hr = spatialClient->GetMaxDynamicObjectCount(&maxDynamicObjects);
    if (FAILED(hr)) {
        return Fail(L"GetMaxDynamicObjectCount", hr);
    }
    std::wcout << L"max_dynamic_objects=" << maxDynamicObjects << L"\n";

    ComPtr<IAudioFormatEnumerator> formatEnumerator;
    hr = spatialClient->GetSupportedAudioObjectFormatEnumerator(formatEnumerator.GetAddressOf());
    if (FAILED(hr)) {
        return Fail(L"GetSupportedAudioObjectFormatEnumerator", hr);
    }

    UINT32 formatCount = 0;
    hr = formatEnumerator->GetCount(&formatCount);
    if (FAILED(hr)) {
        return Fail(L"IAudioFormatEnumerator::GetCount", hr);
    }
    std::wcout << L"object_format_count=" << formatCount << L"\n";

    if (formatCount > 0) {
        WAVEFORMATEX* format = nullptr;
        hr = formatEnumerator->GetFormat(0, &format);
        if (FAILED(hr)) {
            return Fail(L"IAudioFormatEnumerator::GetFormat(0)", hr);
        }
        if (format != nullptr) {
            std::wcout << L"preferred_object_format="
                       << format->nSamplesPerSec << L"Hz,"
                       << format->nChannels << L"ch,"
                       << format->wBitsPerSample << L"bit,tag=0x"
                       << std::hex << std::uppercase << format->wFormatTag
                       << std::dec << L"\n";
            CoTaskMemFree(format);
        }
    }

    std::wcout << std::fixed << std::setprecision(6);
    for (const auto& role : kCanonicalStaticRoles) {
        const bool present = HasRole(nativeMask, role.type);
        std::wcout << L"static." << role.name << L"=" << (present ? L"present" : L"absent");
        if (present) {
            float x = 0.0f;
            float y = 0.0f;
            float z = 0.0f;
            const HRESULT positionHr = spatialClient->GetStaticObjectPosition(role.type, &x, &y, &z);
            if (SUCCEEDED(positionHr)) {
                std::wcout << L",windows_xyz_m=[" << x << L"," << y << L"," << z << L"]"
                           << L",omniphony_xyz_m=[" << x << L"," << -z << L"," << y << L"]";
            } else {
                std::wcout << L",position_hr=0x"
                           << std::hex << std::uppercase << static_cast<unsigned long>(positionHr)
                           << std::dec;
            }
        }
        std::wcout << L"\n";
    }

    return 0;
}
