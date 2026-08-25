#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <winrt/base.h>
#include <winrt/Windows.ApplicationModel.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Media.Audio.h>

#include <iomanip>
#include <iostream>
#include <string>

namespace {

using winrt::Windows::ApplicationModel::Package;
using winrt::Windows::Media::Audio::SetDefaultSpatialAudioFormatStatus;
using winrt::Windows::Media::Audio::SpatialAudioDeviceConfiguration;
using winrt::Windows::Media::Audio::SpatialAudioFormatConfiguration;

constexpr wchar_t kFormatGuid[] = L"{4BD75423-A66C-4586-B782-1FCBBDF2AE74}";

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

bool IsOmniphonyFormat(const winrt::hstring& value) {
    return _wcsicmp(value.c_str(), kFormatGuid) == 0;
}

void PrintSelectionState(const SpatialAudioDeviceConfiguration& config) {
    const auto defaultFormat = config.DefaultSpatialAudioFormat();
    const auto activeFormat = config.ActiveSpatialAudioFormat();
    std::wcout << L"SPATIAL_SUPPORTED\t" << (config.IsSpatialAudioSupported() ? 1 : 0) << L'\n';
    std::wcout << L"OMNIPHONY_FORMAT_SUPPORTED\t"
               << (config.IsSpatialAudioFormatSupported(winrt::hstring{kFormatGuid}) ? 1 : 0) << L'\n';
    std::wcout << L"DEFAULT_FORMAT\t" << defaultFormat.c_str() << L'\n';
    std::wcout << L"ACTIVE_FORMAT\t" << activeFormat.c_str() << L'\n';
    std::wcout << L"OMNIPHONY_DEFAULT\t" << (IsOmniphonyFormat(defaultFormat) ? 1 : 0) << L'\n';
    std::wcout << L"OMNIPHONY_ACTIVE\t" << (IsOmniphonyFormat(activeFormat) ? 1 : 0) << L'\n';
}

int PrintIdentity() {
    const auto package = Package::Current();
    const auto id = package.Id();
    std::wcout << L"PACKAGE_IDENTITY_OK\t1\n";
    std::wcout << L"PACKAGE_NAME\t" << id.Name().c_str() << L'\n';
    std::wcout << L"PACKAGE_FAMILY_NAME\t" << id.FamilyName().c_str() << L'\n';
    std::wcout << L"PACKAGE_FULL_NAME\t" << id.FullName().c_str() << L'\n';
    std::wcout << L"PACKAGE_PUBLISHER_ID\t" << id.PublisherId().c_str() << L'\n';
    std::wcout << L"SPATIAL_FORMAT_GUID\t" << kFormatGuid << L'\n';
    return 0;
}

int RegisterCurrentMediaExtension() {
    using RegisterMediaExtensionPackageFn = HRESULT(WINAPI*)(PCWSTR);

    const auto package = Package::Current();
    const auto familyName = package.Id().FamilyName();
    HMODULE module = LoadLibraryW(L"CompPkgSup.dll");
    if (module == nullptr) {
        const auto error = GetLastError();
        std::wcout << L"MEDIA_EXTENSION_REGISTER_API_AVAILABLE\t0\n";
        std::wcerr << L"ERROR\tCompPkgSup.dll unavailable\t" << error << L'\n';
        return 6;
    }

    const auto registerMediaExtensionPackage = reinterpret_cast<RegisterMediaExtensionPackageFn>(
        GetProcAddress(module, "RegisterMediaExtensionPackage"));
    if (registerMediaExtensionPackage == nullptr) {
        std::wcout << L"MEDIA_EXTENSION_REGISTER_API_AVAILABLE\t0\n";
        std::wcout << L"MEDIA_EXTENSION_REGISTER_REQUIRES_WINDOWS_11_24H2\t1\n";
        FreeLibrary(module);
        return 6;
    }

    std::wcout << L"MEDIA_EXTENSION_REGISTER_API_AVAILABLE\t1\n";
    std::wcout << L"MEDIA_EXTENSION_PACKAGE_FAMILY\t" << familyName.c_str() << L'\n';
    const HRESULT result = registerMediaExtensionPackage(familyName.c_str());
    FreeLibrary(module);

    std::wcout << L"MEDIA_EXTENSION_REGISTER_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<unsigned long>(result) << std::dec << L'\n';
    if (FAILED(result)) {
        return 6;
    }
    std::wcout << L"MEDIA_EXTENSION_REGISTERED\t1\n";
    return 0;
}

int NotifySpatialFormatChanged() {
    const auto formatConfiguration = SpatialAudioFormatConfiguration::GetDefault();
    formatConfiguration.ReportLicenseChangedAsync(winrt::hstring{kFormatGuid}).get();
    std::wcout << L"SPATIAL_LICENSE_CHANGE_REPORTED\t1\n";
    formatConfiguration.ReportConfigurationChangedAsync(winrt::hstring{kFormatGuid}).get();
    std::wcout << L"SPATIAL_CONFIGURATION_CHANGE_REPORTED\t1\n";
    return 0;
}

int SelectionStatus(const wchar_t* endpointId) {
    const auto config = SpatialAudioDeviceConfiguration::GetForDeviceId(winrt::hstring{endpointId});
    PrintSelectionState(config);
    return 0;
}

int SelectEndpoint(const wchar_t* endpointId) {
    const auto package = Package::Current();
    std::wcout << L"CALLER_PACKAGE_FAMILY\t" << package.Id().FamilyName().c_str() << L'\n';
    std::wcout << L"FORMAT_OWNER_CONTEXT_REQUIRED_BY_WINDOWS\t1\n";

    const auto config = SpatialAudioDeviceConfiguration::GetForDeviceId(winrt::hstring{endpointId});
    std::wcout << L"BEFORE\n";
    PrintSelectionState(config);
    std::wcout << L"CAPABILITY_FLAGS\tDIAGNOSTIC_ONLY\n";

    const auto result = config.SetDefaultSpatialAudioFormatAsync(winrt::hstring{kFormatGuid}).get();
    const auto status = result.Status();
    std::wcout << L"SET_STATUS\t" << static_cast<int>(status)
               << L"\t" << SelectionStatusText(status) << L'\n';
    if (status != SetDefaultSpatialAudioFormatStatus::Succeeded) {
        std::wcout << L"WINDOWS_SETTER_ACCEPTED_CONTEXT\t0\n";
        return 7;
    }
    std::wcout << L"WINDOWS_SETTER_ACCEPTED_CONTEXT\t1\n";

    const auto after = SpatialAudioDeviceConfiguration::GetForDeviceId(winrt::hstring{endpointId});
    std::wcout << L"AFTER\n";
    PrintSelectionState(after);
    if (!IsOmniphonyFormat(after.DefaultSpatialAudioFormat())) {
        std::wcerr << L"ERROR\tWindows did not retain Omniphony as the default spatial format.\n";
        return 8;
    }
    std::wcout << L"OMNIPHONY_SPATIAL_DEFAULT_SET\t1\n";
    return 0;
}

void Usage() {
    std::wcerr
        << L"usage: OmniphonySpatialCompanion <command> [endpoint-id]\n"
        << L"  identity              prove the process is running with package identity\n"
        << L"  register              register the package media extension on Windows 11 24H2+\n"
        << L"  notify                report license/configuration change for Omniphony\n"
        << L"  status <endpoint-id>  read spatial selection state from packaged identity\n"
        << L"  select <endpoint-id>  ask Windows to select Omniphony from packaged identity\n";
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc < 2) {
        Usage();
        return 2;
    }

    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        const std::wstring command = argv[1];
        if (command == L"identity" && argc == 2) {
            return PrintIdentity();
        }
        if (command == L"register" && argc == 2) {
            PrintIdentity();
            return RegisterCurrentMediaExtension();
        }
        if (command == L"notify" && argc == 2) {
            PrintIdentity();
            return NotifySpatialFormatChanged();
        }
        if (command == L"status" && argc == 3) {
            PrintIdentity();
            return SelectionStatus(argv[2]);
        }
        if (command == L"select" && argc == 3) {
            PrintIdentity();
            return SelectEndpoint(argv[2]);
        }
        Usage();
        return 2;
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"ERROR\tWinRT\t0x" << std::hex << std::uppercase
                   << static_cast<unsigned long>(error.code().value)
                   << L"\t" << error.message().c_str() << L'\n';
        return 9;
    } catch (const std::exception& error) {
        std::cerr << "ERROR\tstd::exception\t" << error.what() << '\n';
        return 9;
    } catch (...) {
        std::wcerr << L"ERROR\tUnknown packaged spatial-companion failure.\n";
        return 9;
    }
}
