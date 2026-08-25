#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <objbase.h>

#include <winrt/base.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Media.Audio.h>

#include <cwctype>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <string>
#include <utility>

namespace {

using winrt::Windows::Media::Audio::SetDefaultSpatialAudioFormatStatus;
using winrt::Windows::Media::Audio::SpatialAudioDeviceConfiguration;

constexpr int kExitUsage = 2;
constexpr int kExitNotRegistered = 3;
constexpr int kExitAccess = 4;
constexpr int kExitVerify = 5;
constexpr int kExitSelectionUnsupported = 6;
constexpr int kExitSelectionRejected = 7;
constexpr int kExitSelectionReadback = 8;
constexpr int kExitSelectionRuntime = 9;

constexpr wchar_t kDisplayName[] = L"Omniphony";
constexpr wchar_t kFormatGuid[] = L"{4BD75423-A66C-4586-B782-1FCBBDF2AE74}";
constexpr wchar_t kClsidText[] = L"{F3CDF827-20C4-405E-A430-8F739343FC89}";
constexpr GUID kProviderClsid = {
    0xf3cdf827, 0x20c4, 0x405e, {0xa4, 0x30, 0x8f, 0x73, 0x93, 0x43, 0xfc, 0x89}};

constexpr wchar_t kEncoderBase[] = L"SOFTWARE\\Microsoft\\Multimedia\\Audio\\Spatial\\Encoder";
constexpr wchar_t kComBase[] = L"SOFTWARE\\Classes\\CLSID";

std::wstring Join(const wchar_t* left, const wchar_t* right) {
    std::wstring value(left);
    value += L"\\";
    value += right;
    return value;
}

std::wstring Win32Text(DWORD error) {
    wchar_t* buffer = nullptr;
    const DWORD flags = FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM |
                        FORMAT_MESSAGE_IGNORE_INSERTS;
    const DWORD count = FormatMessageW(
        flags,
        nullptr,
        error,
        MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT),
        reinterpret_cast<wchar_t*>(&buffer),
        0,
        nullptr);

    std::wostringstream out;
    out << error << L" (0x" << std::uppercase << std::hex << std::setw(8)
        << std::setfill(L'0') << error << L")";
    if (count && buffer) {
        std::wstring message(buffer, count);
        while (!message.empty() && (message.back() == L'\r' || message.back() == L'\n')) {
            message.pop_back();
        }
        out << L" " << message;
    }
    if (buffer) {
        LocalFree(buffer);
    }
    return out.str();
}

bool IsElevated() {
    HANDLE token = nullptr;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) {
        return false;
    }
    TOKEN_ELEVATION elevation{};
    DWORD size = sizeof(elevation);
    const bool ok = GetTokenInformation(
        token, TokenElevation, &elevation, sizeof(elevation), &size) != FALSE;
    CloseHandle(token);
    return ok && elevation.TokenIsElevated != 0;
}

bool FileExists(const std::wstring& path) {
    const DWORD attributes = GetFileAttributesW(path.c_str());
    return attributes != INVALID_FILE_ATTRIBUTES &&
           (attributes & FILE_ATTRIBUTE_DIRECTORY) == 0;
}

bool AbsolutePath(const wchar_t* input, std::wstring& output) {
    const DWORD needed = GetFullPathNameW(input, 0, nullptr, nullptr);
    if (needed == 0) {
        return false;
    }
    std::wstring buffer(needed, L'\0');
    const DWORD written = GetFullPathNameW(input, needed, buffer.data(), nullptr);
    if (written == 0 || written >= needed) {
        return false;
    }
    buffer.resize(written);
    output = std::move(buffer);
    return true;
}

LONG SetString(HKEY key, const wchar_t* name, const std::wstring& value) {
    const DWORD bytes = static_cast<DWORD>((value.size() + 1) * sizeof(wchar_t));
    return RegSetValueExW(
        key,
        name,
        0,
        REG_SZ,
        reinterpret_cast<const BYTE*>(value.c_str()),
        bytes);
}

bool ReadString(HKEY root, const std::wstring& path, const wchar_t* name, std::wstring& value) {
    HKEY key = nullptr;
    LONG result = RegOpenKeyExW(root, path.c_str(), 0, KEY_READ, &key);
    if (result != ERROR_SUCCESS) {
        return false;
    }

    DWORD type = 0;
    DWORD bytes = 0;
    result = RegQueryValueExW(key, name, nullptr, &type, nullptr, &bytes);
    if (result != ERROR_SUCCESS || (type != REG_SZ && type != REG_EXPAND_SZ) || bytes < sizeof(wchar_t)) {
        RegCloseKey(key);
        return false;
    }

    std::wstring buffer(bytes / sizeof(wchar_t), L'\0');
    result = RegQueryValueExW(
        key,
        name,
        nullptr,
        &type,
        reinterpret_cast<BYTE*>(buffer.data()),
        &bytes);
    RegCloseKey(key);
    if (result != ERROR_SUCCESS) {
        return false;
    }

    if (!buffer.empty() && buffer.back() == L'\0') {
        buffer.pop_back();
    }
    value = std::move(buffer);
    return true;
}

bool KeyExists(HKEY root, const std::wstring& path) {
    HKEY key = nullptr;
    const LONG result = RegOpenKeyExW(root, path.c_str(), 0, KEY_READ, &key);
    if (result == ERROR_SUCCESS) {
        RegCloseKey(key);
        return true;
    }
    return false;
}

LONG DeleteOwnedKey(const std::wstring& path) {
    const LONG result = RegDeleteTreeW(HKEY_LOCAL_MACHINE, path.c_str());
    if (result == ERROR_FILE_NOT_FOUND || result == ERROR_PATH_NOT_FOUND) {
        return ERROR_SUCCESS;
    }
    return result;
}

std::wstring NormalizeGuid(std::wstring value) {
    for (wchar_t& ch : value) {
        ch = static_cast<wchar_t>(std::towupper(ch));
    }
    return value;
}

bool IsOmniphonyFormat(const winrt::hstring& value) {
    return NormalizeGuid(value.c_str()) == NormalizeGuid(kFormatGuid);
}

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

void PrintContract() {
    std::wcout << L"FORMAT_GUID\t" << kFormatGuid << L'\n';
    std::wcout << L"COM_CLSID\t" << kClsidText << L'\n';
    std::wcout << L"ENCODER_BASE\tHKLM\\" << kEncoderBase << L'\n';
    std::wcout << L"COM_BASE\tHKLM\\" << kComBase << L'\n';
    std::wcout << L"STATIC_OBJECTS\t17\n";
    std::wcout << L"MAX_DYNAMIC_OBJECTS\t16\n";
    std::wcout << L"SELECTION_API\tWindows.Media.Audio.SpatialAudioDeviceConfiguration\n";
    std::wcout << L"DIRECT_MMDEVICES_SELECTION_WRITES\t0\n";
}

int ListProviders() {
    HKEY root = nullptr;
    const LONG open = RegOpenKeyExW(HKEY_LOCAL_MACHINE, kEncoderBase, 0, KEY_READ, &root);
    if (open == ERROR_FILE_NOT_FOUND) {
        std::wcout << L"SPATIAL_ENCODER_BASE_ABSENT\n";
        return 0;
    }
    if (open != ERROR_SUCCESS) {
        std::wcerr << L"ERROR\topen Spatial\\Encoder\t" << Win32Text(open) << L'\n';
        return kExitAccess;
    }

    DWORD index = 0;
    bool any = false;
    for (;;) {
        wchar_t name[256] = {};
        DWORD chars = static_cast<DWORD>(sizeof(name) / sizeof(name[0]));
        FILETIME time{};
        const LONG result = RegEnumKeyExW(root, index++, name, &chars, nullptr, nullptr, nullptr, &time);
        if (result == ERROR_NO_MORE_ITEMS) {
            break;
        }
        if (result != ERROR_SUCCESS) {
            RegCloseKey(root);
            std::wcerr << L"ERROR\tenumerate Spatial\\Encoder\t" << Win32Text(result) << L'\n';
            return kExitAccess;
        }

        any = true;
        const std::wstring path = Join(kEncoderBase, name);
        std::wstring display;
        std::wstring clsid;
        ReadString(HKEY_LOCAL_MACHINE, path, nullptr, display);
        ReadString(HKEY_LOCAL_MACHINE, path, L"CLSID", clsid);
        std::wcout << L"SPATIAL_ENCODER\t" << name
                   << L"\tNAME=" << (display.empty() ? L"<none>" : display)
                   << L"\tCLSID=" << (clsid.empty() ? L"<none>" : clsid) << L'\n';
    }
    RegCloseKey(root);
    if (!any) {
        std::wcout << L"SPATIAL_ENCODER_NONE\n";
    }
    return 0;
}

int RegistrationStatus() {
    const std::wstring encoderPath = Join(kEncoderBase, kFormatGuid);
    const std::wstring classPath = Join(kComBase, kClsidText);
    const std::wstring inprocPath = classPath + L"\\InProcServer32";

    std::wstring display;
    std::wstring clsid;
    std::wstring icon;
    std::wstring server;
    const bool encoder = KeyExists(HKEY_LOCAL_MACHINE, encoderPath);
    const bool com = KeyExists(HKEY_LOCAL_MACHINE, inprocPath);
    if (encoder) {
        ReadString(HKEY_LOCAL_MACHINE, encoderPath, nullptr, display);
        ReadString(HKEY_LOCAL_MACHINE, encoderPath, L"CLSID", clsid);
        ReadString(HKEY_LOCAL_MACHINE, encoderPath, L"IconPath", icon);
    }
    if (com) {
        ReadString(HKEY_LOCAL_MACHINE, inprocPath, nullptr, server);
    }

    std::wcout << L"SPATIAL_PROVIDER_STATUS\tENCODER=" << (encoder ? 1 : 0)
               << L"\tCOM=" << (com ? 1 : 0) << L'\n';
    std::wcout << L"FORMAT_GUID\t" << kFormatGuid << L'\n';
    std::wcout << L"COM_CLSID\t" << kClsidText << L'\n';
    if (encoder) {
        std::wcout << L"ENCODER_NAME\t" << (display.empty() ? L"<none>" : display) << L'\n';
        std::wcout << L"ENCODER_CLSID\t" << (clsid.empty() ? L"<none>" : clsid) << L'\n';
        std::wcout << L"ENCODER_ICON\t" << (icon.empty() ? L"<none>" : icon) << L'\n';
    }
    if (com) {
        std::wcout << L"COM_SERVER\t" << (server.empty() ? L"<none>" : server) << L'\n';
    }
    return (encoder && com) ? 0 : kExitNotRegistered;
}

int UnregisterOwnedKeys() {
    if (!IsElevated()) {
        std::wcerr << L"ERROR\tspatial-unregister requires an elevated Administrator terminal\n";
        return kExitAccess;
    }

    const std::wstring encoderPath = Join(kEncoderBase, kFormatGuid);
    const std::wstring classPath = Join(kComBase, kClsidText);
    const LONG encoder = DeleteOwnedKey(encoderPath);
    const LONG com = DeleteOwnedKey(classPath);
    if (encoder != ERROR_SUCCESS) {
        std::wcerr << L"ERROR\tdelete Omniphony Spatial\\Encoder key\t" << Win32Text(encoder) << L'\n';
        return kExitAccess;
    }
    if (com != ERROR_SUCCESS) {
        std::wcerr << L"ERROR\tdelete Omniphony COM key\t" << Win32Text(com) << L'\n';
        return kExitAccess;
    }

    std::wcout << L"SPATIAL_PROVIDER_UNREGISTERED\tFORMAT_GUID=" << kFormatGuid
               << L"\tCLSID=" << kClsidText << L'\n';
    return 0;
}

int RegisterOwnedKeys(const wchar_t* dllArgument) {
    if (!IsElevated()) {
        std::wcerr << L"ERROR\tspatial-register requires an elevated Administrator terminal\n";
        return kExitAccess;
    }

    std::wstring dllPath;
    if (!AbsolutePath(dllArgument, dllPath) || !FileExists(dllPath)) {
        std::wcerr << L"ERROR\tprovider DLL not found\t" << dllArgument << L'\n';
        return kExitUsage;
    }

    const std::wstring classPath = Join(kComBase, kClsidText);
    const std::wstring inprocPath = classPath + L"\\InProcServer32";
    const std::wstring encoderPath = Join(kEncoderBase, kFormatGuid);

    HKEY classKey = nullptr;
    LONG result = RegCreateKeyExW(
        HKEY_LOCAL_MACHINE, classPath.c_str(), 0, nullptr, REG_OPTION_NON_VOLATILE,
        KEY_WRITE, nullptr, &classKey, nullptr);
    if (result != ERROR_SUCCESS) {
        std::wcerr << L"ERROR\tcreate Omniphony COM class\t" << Win32Text(result) << L'\n';
        return kExitAccess;
    }
    result = SetString(classKey, nullptr, L"Omniphony Spatial Provider");
    RegCloseKey(classKey);
    if (result != ERROR_SUCCESS) {
        DeleteOwnedKey(classPath);
        std::wcerr << L"ERROR\tname Omniphony COM class\t" << Win32Text(result) << L'\n';
        return kExitAccess;
    }

    HKEY inprocKey = nullptr;
    result = RegCreateKeyExW(
        HKEY_LOCAL_MACHINE, inprocPath.c_str(), 0, nullptr, REG_OPTION_NON_VOLATILE,
        KEY_WRITE, nullptr, &inprocKey, nullptr);
    if (result == ERROR_SUCCESS) {
        result = SetString(inprocKey, nullptr, dllPath);
    }
    if (result == ERROR_SUCCESS) {
        result = SetString(inprocKey, L"ThreadingModel", L"Both");
    }
    if (inprocKey) {
        RegCloseKey(inprocKey);
    }
    if (result != ERROR_SUCCESS) {
        DeleteOwnedKey(classPath);
        std::wcerr << L"ERROR\tregister Omniphony InProcServer32\t" << Win32Text(result) << L'\n';
        return kExitAccess;
    }

    HKEY encoderKey = nullptr;
    result = RegCreateKeyExW(
        HKEY_LOCAL_MACHINE, encoderPath.c_str(), 0, nullptr, REG_OPTION_NON_VOLATILE,
        KEY_WRITE, nullptr, &encoderKey, nullptr);
    if (result == ERROR_SUCCESS) {
        result = SetString(encoderKey, nullptr, kDisplayName);
    }
    if (result == ERROR_SUCCESS) {
        result = SetString(encoderKey, L"CLSID", kClsidText);
    }
    if (result == ERROR_SUCCESS) {
        result = SetString(encoderKey, L"IconPath", dllPath + L",0");
    }
    if (encoderKey) {
        RegCloseKey(encoderKey);
    }
    if (result != ERROR_SUCCESS) {
        DeleteOwnedKey(encoderPath);
        DeleteOwnedKey(classPath);
        std::wcerr << L"ERROR\tregister Omniphony Spatial\\Encoder format\t"
                   << Win32Text(result) << L'\n';
        return kExitAccess;
    }

    const HRESULT init = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    if (FAILED(init) && init != RPC_E_CHANGED_MODE) {
        DeleteOwnedKey(encoderPath);
        DeleteOwnedKey(classPath);
        std::wcerr << L"ERROR\tCoInitializeEx failed; registration rolled back\t0x"
                   << std::uppercase << std::hex << static_cast<unsigned long>(init) << L'\n';
        return kExitVerify;
    }

    IUnknown* provider = nullptr;
    const HRESULT activate = CoCreateInstance(
        kProviderClsid, nullptr, CLSCTX_INPROC_SERVER, IID_IUnknown,
        reinterpret_cast<void**>(&provider));
    if (provider) {
        provider->Release();
    }
    if (SUCCEEDED(init)) {
        CoUninitialize();
    }
    if (FAILED(activate)) {
        DeleteOwnedKey(encoderPath);
        DeleteOwnedKey(classPath);
        std::wcerr << L"ERROR\tprovider COM activation failed; registration rolled back\t0x"
                   << std::uppercase << std::hex << static_cast<unsigned long>(activate) << L'\n';
        return kExitVerify;
    }

    std::wcout << L"SPATIAL_PROVIDER_REGISTERED\tFORMAT_GUID=" << kFormatGuid
               << L"\tCLSID=" << kClsidText << L"\tDLL=" << dllPath << L'\n';
    std::wcout << L"COM_ACTIVATION_OK\tIUnknown\n";
    std::wcout << L"OBJECT_CAPACITY\tSTATIC=17\tDYNAMIC=16\n";
    return 0;
}

int Diagnose() {
    const int status = RegistrationStatus();
    const int listed = ListProviders();
    if (listed != 0) {
        return listed;
    }
    if (status != 0) {
        std::wcerr << L"DIAGNOSIS\tOmniphony provider is not fully registered.\n";
        return status;
    }

    const HRESULT init = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    if (FAILED(init) && init != RPC_E_CHANGED_MODE) {
        std::wcerr << L"ERROR\tCoInitializeEx\t0x" << std::uppercase << std::hex
                   << static_cast<unsigned long>(init) << L'\n';
        return kExitVerify;
    }

    IUnknown* provider = nullptr;
    const HRESULT activate = CoCreateInstance(
        kProviderClsid, nullptr, CLSCTX_INPROC_SERVER, IID_IUnknown,
        reinterpret_cast<void**>(&provider));
    if (provider) {
        provider->Release();
    }
    if (SUCCEEDED(init)) {
        CoUninitialize();
    }
    if (FAILED(activate)) {
        std::wcerr << L"ERROR\tCoCreateInstance provider\t0x" << std::uppercase << std::hex
                   << static_cast<unsigned long>(activate) << L'\n';
        return kExitVerify;
    }

    std::wcout << L"COM_ACTIVATION_OK\tIUnknown\n";
    std::wcout << L"DIAGNOSIS\tRegistration and provider COM construction are internally consistent.\n";
    std::wcout << L"OBJECT_CAPACITY\tSTATIC=17\tDYNAMIC=16\n";
    return 0;
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

int SelectionStatus(const wchar_t* endpointId, bool requireOmniphony) {
    const auto config = SpatialAudioDeviceConfiguration::GetForDeviceId(winrt::hstring{endpointId});
    PrintSelectionState(config);
    if (!requireOmniphony) {
        return 0;
    }
    if (!config.IsSpatialAudioSupported() ||
        !config.IsSpatialAudioFormatSupported(winrt::hstring{kFormatGuid})) {
        return kExitSelectionUnsupported;
    }
    if (!IsOmniphonyFormat(config.DefaultSpatialAudioFormat()) ||
        !IsOmniphonyFormat(config.ActiveSpatialAudioFormat())) {
        return kExitSelectionReadback;
    }
    std::wcout << L"OMNIPHONY_SPATIAL_SELECTION_VERIFIED\t1\n";
    return 0;
}

int SelectEndpoint(const wchar_t* endpointId) {
    const auto config = SpatialAudioDeviceConfiguration::GetForDeviceId(winrt::hstring{endpointId});
    std::wcout << L"BEFORE\n";
    PrintSelectionState(config);

    if (!config.IsSpatialAudioSupported() ||
        !config.IsSpatialAudioFormatSupported(winrt::hstring{kFormatGuid})) {
        std::wcerr << L"ERROR\tOmniphony is not reported as supported on this endpoint.\n";
        return kExitSelectionUnsupported;
    }

    if (!IsOmniphonyFormat(config.DefaultSpatialAudioFormat())) {
        const auto result = config.SetDefaultSpatialAudioFormatAsync(winrt::hstring{kFormatGuid}).get();
        const auto status = result.Status();
        std::wcout << L"SET_STATUS\t" << static_cast<int>(status)
                   << L"\t" << SelectionStatusText(status) << L'\n';
        if (status != SetDefaultSpatialAudioFormatStatus::Succeeded) {
            return kExitSelectionRejected;
        }
    } else {
        std::wcout << L"SET_STATUS\t0\tAlreadyDefault\n";
    }

    const auto after = SpatialAudioDeviceConfiguration::GetForDeviceId(winrt::hstring{endpointId});
    std::wcout << L"AFTER\n";
    PrintSelectionState(after);
    if (!IsOmniphonyFormat(after.DefaultSpatialAudioFormat())) {
        std::wcerr << L"ERROR\tWindows did not retain Omniphony as the default spatial format.\n";
        return kExitSelectionReadback;
    }
    std::wcout << L"OMNIPHONY_SPATIAL_DEFAULT_SET\t1\n";
    return 0;
}

int RunSelectionCommand(const std::wstring& command, const wchar_t* endpointId) {
    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        if (command == L"selection-status") {
            return SelectionStatus(endpointId, false);
        }
        if (command == L"selection-select") {
            return SelectEndpoint(endpointId);
        }
        if (command == L"selection-verify") {
            return SelectionStatus(endpointId, true);
        }
        return kExitUsage;
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"ERROR\tWinRT\t0x" << std::hex << std::uppercase
                   << static_cast<unsigned long>(error.code().value)
                   << L"\t" << error.message().c_str() << L'\n';
        return kExitSelectionRuntime;
    } catch (const std::exception& error) {
        std::cerr << "ERROR\tstd::exception\t" << error.what() << '\n';
        return kExitSelectionRuntime;
    } catch (...) {
        std::wcerr << L"ERROR\tUnknown spatial selection failure.\n";
        return kExitSelectionRuntime;
    }
}

void Usage() {
    std::wcerr
        << L"usage: OmniphonySpatialProbeCtl <command> [argument]\n"
        << L"  contract                         print stable provider and selection contract\n"
        << L"  list                             list HKLM Spatial\\Encoder entries (read-only)\n"
        << L"  status                           inspect Omniphony registration (read-only)\n"
        << L"  register <provider-dll>          register Omniphony provider (Administrator)\n"
        << L"  diagnose                         verify registry plus provider COM construction\n"
        << L"  unregister                       remove only Omniphony registration (Administrator)\n"
        << L"  selection-status <endpoint-id>   print Windows spatial selection state\n"
        << L"  selection-select <endpoint-id>   set Omniphony by GUID and read default back\n"
        << L"  selection-verify <endpoint-id>   require Omniphony as both default and active\n";
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc < 2) {
        Usage();
        return kExitUsage;
    }

    const std::wstring command = argv[1];
    if (command == L"contract") {
        if (argc != 2) {
            Usage();
            return kExitUsage;
        }
        PrintContract();
        return 0;
    }
    if (command == L"list") {
        return argc == 2 ? ListProviders() : kExitUsage;
    }
    if (command == L"status") {
        return argc == 2 ? RegistrationStatus() : kExitUsage;
    }
    if (command == L"register") {
        if (argc != 3 || !argv[2] || !*argv[2]) {
            Usage();
            return kExitUsage;
        }
        return RegisterOwnedKeys(argv[2]);
    }
    if (command == L"diagnose") {
        return argc == 2 ? Diagnose() : kExitUsage;
    }
    if (command == L"unregister") {
        return argc == 2 ? UnregisterOwnedKeys() : kExitUsage;
    }
    if (command == L"selection-status" || command == L"selection-select" ||
        command == L"selection-verify") {
        if (argc != 3 || !argv[2] || !*argv[2]) {
            Usage();
            return kExitUsage;
        }
        return RunSelectionCommand(command, argv[2]);
    }

    Usage();
    return kExitUsage;
}
