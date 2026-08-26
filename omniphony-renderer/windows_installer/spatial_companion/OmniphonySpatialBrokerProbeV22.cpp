#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <mmdeviceapi.h>

#include <winrt/base.h>
#include <winrt/Windows.ApplicationModel.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.Media.Audio.h>
#include <winrt/Windows.Storage.h>

#include <cstdint>
#include <iomanip>
#include <iostream>
#include <string>

namespace {

using winrt::Windows::ApplicationModel::Package;
using winrt::Windows::Foundation::IPropertyValue;
using winrt::Windows::Foundation::PropertyType;
using winrt::Windows::Foundation::Collections::IPropertySet;
using winrt::Windows::Media::Audio::SetDefaultSpatialAudioFormatStatus;
using winrt::Windows::Media::Audio::SpatialAudioDeviceConfiguration;
using winrt::Windows::Media::Audio::SpatialAudioFormatConfiguration;
using winrt::Windows::Storage::ApplicationData;

constexpr wchar_t kFormatGuid[] = L"{4BD75423-A66C-4586-B782-1FCBBDF2AE74}";
constexpr wchar_t kCountKey[] = L"SpatialLicenseBroker.RequestCount";
constexpr wchar_t kCommandKey[] = L"SpatialLicenseBroker.LastCommand";
constexpr wchar_t kDeviceKey[] = L"SpatialLicenseBroker.LastDeviceID";
constexpr wchar_t kCodecKey[] = L"SpatialLicenseBroker.LastMediaCodecName";
constexpr wchar_t kSubtypeKey[] = L"SpatialLicenseBroker.LastSpatialAudioSubtype";

const wchar_t* SelectionStatusText(SetDefaultSpatialAudioFormatStatus status) {
    switch (status) {
    case SetDefaultSpatialAudioFormatStatus::Succeeded:
        return L"Succeeded";
    case SetDefaultSpatialAudioFormatStatus::AccessDenied:
        return L"AccessDenied";
    case SetDefaultSpatialAudioFormatStatus::LicenseExpired:
        return L"LicenseExpired";
    case SetDefaultSpatialAudioFormatStatus::LicenseNotValidForAudioEndpoint:
        return L"LicenseNotValidForAudioEndpoint";
    case SetDefaultSpatialAudioFormatStatus::NotSupportedOnAudioEndpoint:
        return L"NotSupportedOnAudioEndpoint";
    case SetDefaultSpatialAudioFormatStatus::UnknownError:
        return L"UnknownError";
    default:
        return L"Unrecognized";
    }
}

std::wstring DefaultRenderEndpointId() {
    winrt::com_ptr<IMMDeviceEnumerator> enumerator;
    winrt::check_hresult(CoCreateInstance(
        __uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
        __uuidof(IMMDeviceEnumerator), enumerator.put_void()));

    winrt::com_ptr<IMMDevice> device;
    winrt::check_hresult(enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, device.put()));

    LPWSTR raw = nullptr;
    winrt::check_hresult(device->GetId(&raw));
    std::wstring endpoint = raw == nullptr ? L"" : raw;
    if (raw != nullptr) {
        CoTaskMemFree(raw);
    }
    return endpoint;
}

HRESULT RegisterCurrentMediaExtension() {
    using RegisterMediaExtensionPackageFn = HRESULT(WINAPI*)(PCWSTR);

    HMODULE module = LoadLibraryW(L"CompPkgSup.dll");
    if (module == nullptr) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    const auto registerPackage = reinterpret_cast<RegisterMediaExtensionPackageFn>(
        GetProcAddress(module, "RegisterMediaExtensionPackage"));
    if (registerPackage == nullptr) {
        const HRESULT result = HRESULT_FROM_WIN32(ERROR_PROC_NOT_FOUND);
        FreeLibrary(module);
        return result;
    }

    const auto family = Package::Current().Id().FamilyName();
    const HRESULT result = registerPackage(family.c_str());
    FreeLibrary(module);
    return result;
}

IPropertySet ObservationValues() {
    return ApplicationData::Current().LocalSettings().Values();
}

void RemoveIfPresent(const IPropertySet& values, const wchar_t* key) {
    const winrt::hstring name{key};
    if (values.HasKey(name)) {
        values.Remove(name);
    }
}

void ResetObservation() {
    const auto values = ObservationValues();
    RemoveIfPresent(values, kCountKey);
    RemoveIfPresent(values, kCommandKey);
    RemoveIfPresent(values, kDeviceKey);
    RemoveIfPresent(values, kCodecKey);
    RemoveIfPresent(values, kSubtypeKey);
    values.Insert(kCountKey, winrt::box_value(std::uint32_t{0}));
    std::wcout << L"BROKER_V22_OBSERVATION_RESET\t1\n";
}

std::uint32_t ReadUInt32(const IPropertySet& values, const wchar_t* key) {
    const winrt::hstring name{key};
    if (!values.HasKey(name)) {
        return 0;
    }
    const auto property = values.Lookup(name).try_as<IPropertyValue>();
    if (!property || property.Type() != PropertyType::UInt32) {
        return 0;
    }
    return property.GetUInt32();
}

std::wstring ReadString(const IPropertySet& values, const wchar_t* key) {
    const winrt::hstring name{key};
    if (!values.HasKey(name)) {
        return L"<absent>";
    }
    const auto property = values.Lookup(name).try_as<IPropertyValue>();
    if (!property || property.Type() != PropertyType::String) {
        return L"<non-string>";
    }
    return std::wstring(property.GetString().c_str());
}

struct ObservationSnapshot {
    std::uint32_t requestCount = 0;
    std::wstring command;
    std::wstring deviceId;
    std::wstring mediaCodecName;
    std::wstring spatialAudioSubtype;
};

ObservationSnapshot ReadObservation() {
    const auto values = ObservationValues();
    ObservationSnapshot snapshot;
    snapshot.requestCount = ReadUInt32(values, kCountKey);
    snapshot.command = ReadString(values, kCommandKey);
    snapshot.deviceId = ReadString(values, kDeviceKey);
    snapshot.mediaCodecName = ReadString(values, kCodecKey);
    snapshot.spatialAudioSubtype = ReadString(values, kSubtypeKey);
    return snapshot;
}

void PrintObservation(const wchar_t* phase, const ObservationSnapshot& snapshot) {
    std::wcout << L"BROKER_V22_OBSERVATION_PHASE\t" << phase << L'\n';
    std::wcout << L"BROKER_V22_REQUEST_COUNT\t" << snapshot.requestCount << L'\n';
    std::wcout << L"BROKER_V22_LAST_COMMAND\t" << snapshot.command << L'\n';
    std::wcout << L"BROKER_V22_LAST_DEVICE_ID\t" << snapshot.deviceId << L'\n';
    std::wcout << L"BROKER_V22_LAST_MEDIA_CODEC_NAME\t" << snapshot.mediaCodecName << L'\n';
    std::wcout << L"BROKER_V22_LAST_SPATIAL_AUDIO_SUBTYPE\t" << snapshot.spatialAudioSubtype << L'\n';
}

bool EqualsInsensitive(const std::wstring& left, const std::wstring& right) {
    return _wcsicmp(left.c_str(), right.c_str()) == 0;
}

} // namespace

int wmain() {
    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        std::wcout << L"BROKER_V22_PROBE_BEGIN\t1\n";

        const auto package = Package::Current();
        std::wcout << L"BROKER_V22_PACKAGE_FAMILY\t" << package.Id().FamilyName().c_str() << L'\n';
        std::wcout << L"BROKER_V22_FORMAT_GUID\t" << kFormatGuid << L'\n';
        std::wcout << L"BROKER_V22_SELF_APP_SERVICE_CALLS\t0\n";

        const HRESULT registration = RegisterCurrentMediaExtension();
        std::wcout << L"BROKER_V22_MEDIA_EXTENSION_REGISTER_HRESULT\t0x"
                   << std::hex << std::uppercase << static_cast<std::uint32_t>(registration)
                   << std::dec << L'\n';
        std::wcout << L"BROKER_V22_MEDIA_EXTENSION_REGISTERED\t" << (SUCCEEDED(registration) ? 1 : 0) << L'\n';
        if (FAILED(registration)) {
            std::wcout << L"BROKER_V22_PROBE_END\t1\n";
            return 6;
        }

        const auto endpointId = DefaultRenderEndpointId();
        std::wcout << L"BROKER_V22_ENDPOINT_ID\t" << endpointId << L'\n';
        if (endpointId.empty()) {
            std::wcout << L"BROKER_V22_PROBE_END\t1\n";
            return 5;
        }

        ResetObservation();

        const auto formatConfiguration = SpatialAudioFormatConfiguration::GetDefault();
        formatConfiguration.ReportLicenseChangedAsync(winrt::hstring{kFormatGuid}).get();
        std::wcout << L"BROKER_V22_LICENSE_CHANGE_REPORTED\t1\n";
        formatConfiguration.ReportConfigurationChangedAsync(winrt::hstring{kFormatGuid}).get();
        std::wcout << L"BROKER_V22_CONFIGURATION_CHANGE_REPORTED\t1\n";
        Sleep(500);

        const auto afterNotify = ReadObservation();
        PrintObservation(L"AFTER_NOTIFY", afterNotify);

        const auto config = SpatialAudioDeviceConfiguration::GetForDeviceId(winrt::hstring{endpointId});
        std::wcout << L"BROKER_V22_SPATIAL_SUPPORTED_BEFORE\t" << (config.IsSpatialAudioSupported() ? 1 : 0) << L'\n';
        std::wcout << L"BROKER_V22_FORMAT_SUPPORTED_BEFORE\t"
                   << (config.IsSpatialAudioFormatSupported(winrt::hstring{kFormatGuid}) ? 1 : 0) << L'\n';

        const auto result = config.SetDefaultSpatialAudioFormatAsync(winrt::hstring{kFormatGuid}).get();
        const auto status = result.Status();
        std::wcout << L"BROKER_V22_SET_STATUS\t" << static_cast<int>(status)
                   << L"\t" << SelectionStatusText(status) << L'\n';
        Sleep(500);

        const auto afterSetter = ReadObservation();
        PrintObservation(L"AFTER_SETTER", afterSetter);

        const bool windowsContactObserved = afterSetter.requestCount != 0;
        const bool setterAddedRequest = afterSetter.requestCount > afterNotify.requestCount;
        const bool deviceMatches = windowsContactObserved && EqualsInsensitive(afterSetter.deviceId, endpointId);
        const bool subtypeMatches = windowsContactObserved && EqualsInsensitive(afterSetter.spatialAudioSubtype, kFormatGuid);

        std::wcout << L"BROKER_V22_WINDOWS_APP_SERVICE_CONTACT_OBSERVED\t"
                   << (windowsContactObserved ? 1 : 0) << L'\n';
        std::wcout << L"BROKER_V22_SETTER_ADDED_APP_SERVICE_REQUEST\t"
                   << (setterAddedRequest ? 1 : 0) << L'\n';
        std::wcout << L"BROKER_V22_LAST_DEVICE_MATCHES_ENDPOINT\t"
                   << (deviceMatches ? 1 : 0) << L'\n';
        std::wcout << L"BROKER_V22_LAST_SUBTYPE_MATCHES_FORMAT\t"
                   << (subtypeMatches ? 1 : 0) << L'\n';
        std::wcout << L"BROKER_V22_PROBE_END\t1\n";

        return status == SetDefaultSpatialAudioFormatStatus::Succeeded ? 0 : 7;
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"BROKER_V22_HRESULT\t0x" << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(error.code().value) << std::dec
                   << L"\t" << error.message().c_str() << L'\n';
    } catch (const std::exception& error) {
        std::cerr << "BROKER_V22_EXCEPTION\t" << error.what() << '\n';
    }
    std::wcout << L"BROKER_V22_PROBE_END\t1\n";
    return 99;
}
