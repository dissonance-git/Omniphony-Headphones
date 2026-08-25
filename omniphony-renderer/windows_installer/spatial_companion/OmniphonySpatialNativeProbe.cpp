#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <mmdeviceapi.h>
#include <propsys.h>
#include <functiondiscoverykeys_devpkey.h>
#include <spatialaudioclient.h>

#include <winrt/base.h>
#include <winrt/Windows.Media.Audio.h>

#include <array>
#include <cstdint>
#include <iomanip>
#include <iostream>
#include <string>

void PauseIfExplorerLaunch();

namespace {

using winrt::Windows::Media::Audio::SpatialAudioDeviceConfiguration;
using winrt::Windows::Media::Audio::SpatialAudioFormatSubtype;

constexpr wchar_t kOmniphonyGuidText[] = L"{4BD75423-A66C-4586-B782-1FCBBDF2AE74}";

struct SpatialProperty {
    const wchar_t* label;
    PROPERTYKEY key;
};

constexpr GUID kGuidEnabled =
    {0x6737016f, 0x5360, 0x48ee, {0xaf, 0x05, 0xe6, 0x16, 0xc8, 0xff, 0x27, 0xa7}};
constexpr GUID kGuidActive =
    {0xfd8a7b27, 0x0b18, 0x4025, {0xab, 0x1c, 0xbd, 0xd6, 0xb3, 0x2e, 0x16, 0x04}};
constexpr GUID kGuidProvider =
    {0x908dba32, 0xedff, 0x4c28, {0x8e, 0x45, 0xc9, 0x18, 0x56, 0x1f, 0x67, 0x48}};
constexpr GUID kGuidSelection =
    {0x8a845654, 0xd6c3, 0x4cd7, {0xb4, 0xeb, 0x24, 0x3d, 0x4b, 0xd9, 0x90, 0x32}};
constexpr GUID kGuidCarrier =
    {0xf19f064d, 0x082c, 0x4e27, {0xbc, 0x73, 0x68, 0x82, 0xa1, 0xbb, 0x8e, 0x4c}};

constexpr std::array<SpatialProperty, 5> kSpatialProperties = {{
    {L"ENABLED", {kGuidEnabled, 2}},
    {L"ACTIVE", {kGuidActive, 2}},
    {L"PROVIDER", {kGuidProvider, 2}},
    {L"SELECTION", {kGuidSelection, 2}},
    {L"CARRIER", {kGuidCarrier, 0}},
}};

std::wstring DeviceId(IMMDevice* device) {
    LPWSTR raw = nullptr;
    if (device == nullptr || FAILED(device->GetId(&raw)) || raw == nullptr) {
        if (raw != nullptr) {
            CoTaskMemFree(raw);
        }
        return {};
    }
    std::wstring value(raw);
    CoTaskMemFree(raw);
    return value;
}

std::wstring FriendlyName(IMMDevice* device) {
    if (device == nullptr) {
        return L"<unknown>";
    }
    winrt::com_ptr<IPropertyStore> store;
    if (FAILED(device->OpenPropertyStore(STGM_READ, store.put()))) {
        return L"<unknown>";
    }
    PROPVARIANT value{};
    PropVariantInit(&value);
    const HRESULT hr = store->GetValue(PKEY_Device_FriendlyName, &value);
    std::wstring result = L"<unknown>";
    if (SUCCEEDED(hr) && value.vt == VT_LPWSTR && value.pwszVal != nullptr) {
        result.assign(value.pwszVal);
    }
    PropVariantClear(&value);
    return result;
}

void PrintBytes(const BYTE* data, ULONG size) {
    if (data == nullptr || size == 0) {
        std::wcout << L"<empty>";
        return;
    }
    const auto flags = std::wcout.flags();
    const auto fill = std::wcout.fill();
    for (ULONG i = 0; i < size; ++i) {
        if (i != 0) {
            std::wcout << L' ';
        }
        std::wcout << std::hex << std::uppercase << std::setw(2) << std::setfill(L'0')
                   << static_cast<unsigned int>(data[i]);
    }
    std::wcout.flags(flags);
    std::wcout.fill(fill);
}

bool ContainsGuidBytes(const BYTE* data, ULONG size, const GUID& guid) {
    if (data == nullptr || size < sizeof(GUID)) {
        return false;
    }
    const auto* needle = reinterpret_cast<const BYTE*>(&guid);
    for (ULONG offset = 0; offset + sizeof(GUID) <= size; ++offset) {
        bool match = true;
        for (ULONG index = 0; index < sizeof(GUID); ++index) {
            if (data[offset + index] != needle[index]) {
                match = false;
                break;
            }
        }
        if (match) {
            return true;
        }
    }
    return false;
}

void DumpSpatialProperties(IMMDevice* device, const GUID& sonicGuid, const GUID& omniphonyGuid) {
    winrt::com_ptr<IPropertyStore> store;
    const HRESULT openHr = device->OpenPropertyStore(STGM_READ, store.put());
    std::wcout << L"NATIVE_PROBE_PROPERTY_STORE_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(openHr)
               << std::dec << L'\n';
    if (FAILED(openHr)) {
        return;
    }

    for (const auto& property : kSpatialProperties) {
        PROPVARIANT value{};
        PropVariantInit(&value);
        const HRESULT hr = store->GetValue(property.key, &value);
        std::wcout << L"NATIVE_PROBE_MMDEVICE_PROPERTY\t" << property.label
                   << L"\tHRESULT=0x" << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(hr) << std::dec
                   << L"\tVT=" << value.vt;

        const BYTE* bytes = nullptr;
        ULONG byteCount = 0;
        if (SUCCEEDED(hr)) {
            if (value.vt == VT_BLOB) {
                bytes = value.blob.pBlobData;
                byteCount = value.blob.cbSize;
            } else if (value.vt == (VT_VECTOR | VT_UI1)) {
                bytes = value.caub.pElems;
                byteCount = value.caub.cElems;
            }
        }

        if (bytes != nullptr || byteCount != 0) {
            std::wcout << L"\tBYTES=" << byteCount << L"\tHEX=";
            PrintBytes(bytes, byteCount);
            std::wcout << L"\tHAS_WINDOWS_SONIC_GUID="
                       << (ContainsGuidBytes(bytes, byteCount, sonicGuid) ? 1 : 0)
                       << L"\tHAS_OMNIPHONY_GUID="
                       << (ContainsGuidBytes(bytes, byteCount, omniphonyGuid) ? 1 : 0);
        } else if (SUCCEEDED(hr) && value.vt == VT_UI4) {
            std::wcout << L"\tVALUE=" << value.ulVal;
        } else if (SUCCEEDED(hr) && value.vt == VT_BOOL) {
            std::wcout << L"\tVALUE=" << (value.boolVal == VARIANT_TRUE ? 1 : 0);
        }
        std::wcout << L'\n';
        PropVariantClear(&value);
    }
}

void ProbeWinRt(const std::wstring& id, winrt::hstring& sonicSubtype) {
    try {
        sonicSubtype = SpatialAudioFormatSubtype::WindowsSonic();
        std::wcout << L"NATIVE_PROBE_WINDOWS_SONIC_SUBTYPE\t" << sonicSubtype.c_str() << L'\n';

        const auto config = SpatialAudioDeviceConfiguration::GetForDeviceId(winrt::hstring{id});
        std::wcout << L"NATIVE_PROBE_WINRT_CONFIG_AVAILABLE\t1\n";
        std::wcout << L"NATIVE_PROBE_WINRT_SPATIAL_SUPPORTED\t"
                   << (config.IsSpatialAudioSupported() ? 1 : 0) << L'\n';
        std::wcout << L"NATIVE_PROBE_WINRT_SONIC_SUPPORTED\t"
                   << (config.IsSpatialAudioFormatSupported(sonicSubtype) ? 1 : 0) << L'\n';
        std::wcout << L"NATIVE_PROBE_WINRT_OMNIPHONY_SUPPORTED\t"
                   << (config.IsSpatialAudioFormatSupported(winrt::hstring{kOmniphonyGuidText}) ? 1 : 0)
                   << L'\n';
        std::wcout << L"NATIVE_PROBE_WINRT_DEFAULT_FORMAT\t"
                   << config.DefaultSpatialAudioFormat().c_str() << L'\n';
        std::wcout << L"NATIVE_PROBE_WINRT_ACTIVE_FORMAT\t"
                   << config.ActiveSpatialAudioFormat().c_str() << L'\n';
    } catch (const winrt::hresult_error& error) {
        std::wcout << L"NATIVE_PROBE_WINRT_CONFIG_AVAILABLE\t0\n";
        std::wcout << L"NATIVE_PROBE_WINRT_HRESULT\t0x"
                   << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(error.code().value)
                   << std::dec << L'\n';
    }
}

void ProbeNativeSpatialClient(IMMDevice* device) {
    winrt::com_ptr<ISpatialAudioClient> spatialClient;
    const HRESULT activateHr = device->Activate(
        __uuidof(ISpatialAudioClient), CLSCTX_INPROC_SERVER, nullptr, spatialClient.put_void());
    std::wcout << L"NATIVE_PROBE_ISPATIALAUDIOCLIENT_ACTIVATE_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(activateHr)
               << std::dec << L'\n';
    std::wcout << L"NATIVE_PROBE_ISPATIALAUDIOCLIENT_AVAILABLE\t"
               << (SUCCEEDED(activateHr) && spatialClient ? 1 : 0) << L'\n';
    if (FAILED(activateHr) || !spatialClient) {
        return;
    }

    UINT32 maxDynamicObjects = 0;
    const HRESULT dynamicHr = spatialClient->GetMaxDynamicObjectCount(&maxDynamicObjects);
    std::wcout << L"NATIVE_PROBE_MAX_DYNAMIC_OBJECTS_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(dynamicHr)
               << std::dec << L'\n';
    if (SUCCEEDED(dynamicHr)) {
        std::wcout << L"NATIVE_PROBE_MAX_DYNAMIC_OBJECTS\t" << maxDynamicObjects << L'\n';
    }

    winrt::com_ptr<IAudioFormatEnumerator> formats;
    const HRESULT formatsHr = spatialClient->GetSupportedAudioObjectFormatEnumerator(formats.put());
    std::wcout << L"NATIVE_PROBE_OBJECT_FORMAT_ENUM_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(formatsHr)
               << std::dec << L'\n';
    if (FAILED(formatsHr) || !formats) {
        return;
    }

    UINT32 count = 0;
    const HRESULT countHr = formats->GetCount(&count);
    std::wcout << L"NATIVE_PROBE_OBJECT_FORMAT_COUNT_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(countHr)
               << std::dec << L'\n';
    if (FAILED(countHr)) {
        return;
    }
    std::wcout << L"NATIVE_PROBE_OBJECT_FORMAT_COUNT\t" << count << L'\n';

    for (UINT32 index = 0; index < count; ++index) {
        WAVEFORMATEX* format = nullptr;
        const HRESULT formatHr = formats->GetFormat(index, &format);
        std::wcout << L"NATIVE_PROBE_OBJECT_FORMAT\t" << index
                   << L"\tHRESULT=0x" << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(formatHr) << std::dec;
        if (SUCCEEDED(formatHr) && format != nullptr) {
            std::wcout << L"\tTAG=" << format->wFormatTag
                       << L"\tCHANNELS=" << format->nChannels
                       << L"\tRATE=" << format->nSamplesPerSec
                       << L"\tBITS=" << format->wBitsPerSample
                       << L"\tBLOCK=" << format->nBlockAlign;
        }
        std::wcout << L'\n';
        if (format != nullptr) {
            CoTaskMemFree(format);
        }
    }
}

} // namespace

int wmain() {
    struct PauseGuard {
        ~PauseGuard() { PauseIfExplorerLaunch(); }
    } pauseGuard;

    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        std::wcout << L"NATIVE_PROBE_BEGIN\t1\n";

        winrt::com_ptr<IMMDeviceEnumerator> enumerator;
        winrt::check_hresult(CoCreateInstance(
            __uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
            __uuidof(IMMDeviceEnumerator), enumerator.put_void()));

        winrt::com_ptr<IMMDevice> device;
        winrt::check_hresult(enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, device.put()));
        const auto id = DeviceId(device.get());
        std::wcout << L"NATIVE_PROBE_ENDPOINT_ID\t" << id << L'\n';
        std::wcout << L"NATIVE_PROBE_ENDPOINT_NAME\t" << FriendlyName(device.get()) << L'\n';

        winrt::hstring sonicSubtype;
        ProbeWinRt(id, sonicSubtype);

        GUID sonicGuid{};
        GUID omniphonyGuid{};
        const HRESULT sonicGuidHr = CLSIDFromString(sonicSubtype.c_str(), &sonicGuid);
        const HRESULT omniphonyGuidHr = CLSIDFromString(kOmniphonyGuidText, &omniphonyGuid);
        std::wcout << L"NATIVE_PROBE_WINDOWS_SONIC_GUID_PARSE_HRESULT\t0x"
                   << std::hex << std::uppercase << static_cast<std::uint32_t>(sonicGuidHr)
                   << std::dec << L'\n';
        std::wcout << L"NATIVE_PROBE_OMNIPHONY_GUID_PARSE_HRESULT\t0x"
                   << std::hex << std::uppercase << static_cast<std::uint32_t>(omniphonyGuidHr)
                   << std::dec << L'\n';

        if (SUCCEEDED(sonicGuidHr) && SUCCEEDED(omniphonyGuidHr)) {
            DumpSpatialProperties(device.get(), sonicGuid, omniphonyGuid);
        }

        ProbeNativeSpatialClient(device.get());
        std::wcout << L"NATIVE_PROBE_END\t1\n";
        return 0;
    } catch (const winrt::hresult_error& error) {
        std::wcout << L"NATIVE_PROBE_FATAL_HRESULT\t0x"
                   << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(error.code().value)
                   << std::dec << L"\t" << error.message().c_str() << L'\n';
    } catch (const std::exception& error) {
        std::cout << "NATIVE_PROBE_FATAL_EXCEPTION\t" << error.what() << '\n';
    }
    return 99;
}
