#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <winrt/base.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Media.Audio.h>

#include <cwctype>
#include <iostream>
#include <string>

namespace {

using winrt::Windows::Media::Audio::SetDefaultSpatialAudioFormatStatus;
using winrt::Windows::Media::Audio::SpatialAudioDeviceConfiguration;

constexpr wchar_t kOmniphonyFormat[] = L"{4BD75423-A66C-4586-B782-1FCBBDF2AE74}";
constexpr int kExitUsage = 2;
constexpr int kExitSetRejected = 4;
constexpr int kExitReadback = 5;
constexpr int kExitRuntime = 6;

std::wstring NormalizeGuid(std::wstring value) {
    for (wchar_t& ch : value) {
        ch = static_cast<wchar_t>(std::towupper(ch));
    }
    return value;
}

bool IsOmniphony(const winrt::hstring& value) {
    return NormalizeGuid(value.c_str()) == NormalizeGuid(kOmniphonyFormat);
}

const wchar_t* StatusText(SetDefaultSpatialAudioFormatStatus status) {
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

void PrintState(const SpatialAudioDeviceConfiguration& config) {
    const auto defaultFormat = config.DefaultSpatialAudioFormat();
    const auto activeFormat = config.ActiveSpatialAudioFormat();
    std::wcout << L"SPATIAL_SUPPORTED\t" << (config.IsSpatialAudioSupported() ? 1 : 0) << L'\n';
    std::wcout << L"OMNIPHONY_FORMAT_SUPPORTED\t"
               << (config.IsSpatialAudioFormatSupported(kOmniphonyFormat) ? 1 : 0) << L'\n';
    std::wcout << L"DEFAULT_FORMAT\t" << defaultFormat.c_str() << L'\n';
    std::wcout << L"ACTIVE_FORMAT\t" << activeFormat.c_str() << L'\n';
    std::wcout << L"OMNIPHONY_DEFAULT\t" << (IsOmniphony(defaultFormat) ? 1 : 0) << L'\n';
    std::wcout << L"OMNIPHONY_ACTIVE\t" << (IsOmniphony(activeFormat) ? 1 : 0) << L'\n';
}

int Inspect(const wchar_t* endpointId, bool requireActive) {
    const auto config = SpatialAudioDeviceConfiguration::GetForDeviceId(endpointId);
    PrintState(config);
    if (!IsOmniphony(config.DefaultSpatialAudioFormat())) {
        return kExitReadback;
    }
    if (requireActive && !IsOmniphony(config.ActiveSpatialAudioFormat())) {
        return kExitReadback;
    }
    return 0;
}

int Select(const wchar_t* endpointId) {
    const auto config = SpatialAudioDeviceConfiguration::GetForDeviceId(endpointId);
    std::wcout << L"BEFORE\n";
    PrintState(config);
    std::wcout << L"CAPABILITY_FLAGS\tDIAGNOSTIC_ONLY\n";

    if (!IsOmniphony(config.DefaultSpatialAudioFormat())) {
        const auto result = config.SetDefaultSpatialAudioFormatAsync(kOmniphonyFormat).get();
        const auto status = result.Status();
        std::wcout << L"SET_STATUS\t" << static_cast<int>(status)
                   << L"\t" << StatusText(status) << L'\n';
        if (status != SetDefaultSpatialAudioFormatStatus::Succeeded) {
            return kExitSetRejected;
        }
    } else {
        std::wcout << L"SET_STATUS\t0\tAlreadyDefault\n";
    }

    const auto after = SpatialAudioDeviceConfiguration::GetForDeviceId(endpointId);
    std::wcout << L"AFTER\n";
    PrintState(after);
    if (!IsOmniphony(after.DefaultSpatialAudioFormat())) {
        std::wcerr << L"ERROR\tWindows did not retain Omniphony as the default spatial format.\n";
        return kExitReadback;
    }

    std::wcout << L"OMNIPHONY_SPATIAL_DEFAULT_SET\t1\n";
    return 0;
}

void Contract() {
    std::wcout << L"FORMAT_GUID\t" << kOmniphonyFormat << L'\n';
    std::wcout << L"SELECTION_API\tWindows.Media.Audio.SpatialAudioDeviceConfiguration\n";
    std::wcout << L"SET_API\tSetDefaultSpatialAudioFormatAsync\n";
    std::wcout << L"SELECTION_CAPABILITY_FLAGS\tDIAGNOSTIC_ONLY\n";
    std::wcout << L"SELECTION_RESULT_AUTHORITY\tSetDefaultSpatialAudioFormatResult.Status\n";
    std::wcout << L"VERIFY_DEFAULT\t1\n";
    std::wcout << L"VERIFY_ACTIVE_SUPPORTED\t1\n";
    std::wcout << L"UNDOCUMENTED_FORMAT_ID_WRITES\t0\n";
}

void Usage() {
    std::wcerr
        << L"usage: OmniphonySpatialSelectCtl <contract|status|verify|select> [endpoint-id]\n"
        << L"  contract               print the headless-selection contract\n"
        << L"  status <endpoint-id>   print Windows spatial state; never modify it\n"
        << L"  verify <endpoint-id>   require Omniphony to be both default and active\n"
        << L"  select <endpoint-id>   ask Windows to make Omniphony the default format and read it back\n";
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc < 2) {
        Usage();
        return kExitUsage;
    }

    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        const std::wstring command = argv[1];
        if (command == L"contract") {
            if (argc != 2) {
                Usage();
                return kExitUsage;
            }
            Contract();
            return 0;
        }
        if (argc != 3 || !argv[2] || !*argv[2]) {
            Usage();
            return kExitUsage;
        }
        if (command == L"status") {
            const auto config = SpatialAudioDeviceConfiguration::GetForDeviceId(argv[2]);
            PrintState(config);
            return 0;
        }
        if (command == L"verify") {
            return Inspect(argv[2], true);
        }
        if (command == L"select") {
            return Select(argv[2]);
        }
        Usage();
        return kExitUsage;
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"ERROR\tWinRT\t0x" << std::hex << std::uppercase
                   << static_cast<unsigned long>(error.code().value)
                   << L"\t" << error.message().c_str() << L'\n';
        return kExitRuntime;
    } catch (const std::exception& error) {
        std::cerr << "ERROR\tstd::exception\t" << error.what() << '\n';
        return kExitRuntime;
    } catch (...) {
        std::wcerr << L"ERROR\tUnknown selector failure.\n";
        return kExitRuntime;
    }
}
