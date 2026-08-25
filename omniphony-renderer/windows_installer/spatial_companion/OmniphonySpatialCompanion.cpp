#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <appmodel.h>
#include <mmdeviceapi.h>
#include <shobjidl_core.h>

#include <winrt/base.h>
#include <winrt/Windows.ApplicationModel.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.Media.Audio.h>

#include <algorithm>
#include <cstdint>
#include <cwctype>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <string>
#include <vector>

namespace {

using winrt::Windows::ApplicationModel::Package;
using winrt::Windows::Foundation::Collections::IPropertySet;
using winrt::Windows::Foundation::Collections::IVector;
using winrt::Windows::Media::Audio::SetDefaultSpatialAudioFormatStatus;
using winrt::Windows::Media::Audio::SpatialAudioDeviceConfiguration;
using winrt::Windows::Media::Audio::SpatialAudioFormatConfiguration;

constexpr wchar_t kFormatGuid[] = L"{4BD75423-A66C-4586-B782-1FCBBDF2AE74}";
constexpr wchar_t kApplicationId[] = L"Companion";

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

DWORD TokenIntegrityRid(HANDLE token) {
    DWORD required = 0;
    GetTokenInformation(token, TokenIntegrityLevel, nullptr, 0, &required);
    if (required == 0) {
        throw std::runtime_error("GetTokenInformation(TokenIntegrityLevel) failed");
    }

    std::vector<std::uint8_t> buffer(required);
    if (!GetTokenInformation(token, TokenIntegrityLevel, buffer.data(), required, &required)) {
        throw std::runtime_error("GetTokenInformation(TokenIntegrityLevel) failed");
    }

    const auto label = reinterpret_cast<TOKEN_MANDATORY_LABEL*>(buffer.data());
    const auto count = GetSidSubAuthorityCount(label->Label.Sid);
    if (count == nullptr || *count == 0) {
        throw std::runtime_error("Token integrity SID is invalid");
    }
    return *GetSidSubAuthority(label->Label.Sid, *count - 1);
}

bool QueryTokenBool(HANDLE token, TOKEN_INFORMATION_CLASS infoClass) {
    DWORD value = 0;
    DWORD size = 0;
    if (!GetTokenInformation(token, infoClass, &value, sizeof(value), &size)) {
        throw std::runtime_error("GetTokenInformation boolean query failed");
    }
    return value != 0;
}

bool TokenElevated(HANDLE token) {
    TOKEN_ELEVATION elevation{};
    DWORD size = 0;
    if (!GetTokenInformation(token, TokenElevation, &elevation, sizeof(elevation), &size)) {
        throw std::runtime_error("GetTokenInformation(TokenElevation) failed");
    }
    return elevation.TokenIsElevated != 0;
}

const wchar_t* IntegrityName(DWORD rid) {
    if (rid < SECURITY_MANDATORY_LOW_RID) {
        return L"Untrusted";
    }
    if (rid < SECURITY_MANDATORY_MEDIUM_RID) {
        return L"Low";
    }
    if (rid < SECURITY_MANDATORY_HIGH_RID) {
        return L"Medium";
    }
    if (rid < SECURITY_MANDATORY_SYSTEM_RID) {
        return L"High";
    }
    return L"System";
}

std::wstring CurrentApplicationUserModelId(LONG& status) {
    UINT32 length = 0;
    status = GetCurrentApplicationUserModelId(&length, nullptr);
    if (status != ERROR_INSUFFICIENT_BUFFER || length == 0) {
        return {};
    }

    std::vector<wchar_t> buffer(length);
    status = GetCurrentApplicationUserModelId(&length, buffer.data());
    if (status != ERROR_SUCCESS) {
        return {};
    }
    return std::wstring(buffer.data());
}

void PrintProcessTrustDiagnostics() {
    HANDLE rawToken = nullptr;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &rawToken)) {
        std::wcout << L"PROCESS_TOKEN_QUERY_OK\t0\n";
        return;
    }

    const DWORD rid = TokenIntegrityRid(rawToken);
    const bool elevated = TokenElevated(rawToken);
    const bool appContainer = QueryTokenBool(rawToken, TokenIsAppContainer);
    CloseHandle(rawToken);

    std::wcout << L"PROCESS_TOKEN_QUERY_OK\t1\n";
    std::wcout << L"PROCESS_INTEGRITY\t" << IntegrityName(rid) << L'\n';
    std::wcout << L"PROCESS_INTEGRITY_RID\t0x" << std::hex << std::uppercase << rid
               << std::dec << L'\n';
    std::wcout << L"PROCESS_ELEVATED\t" << (elevated ? 1 : 0) << L'\n';
    std::wcout << L"PROCESS_TOKEN_IS_APPCONTAINER\t" << (appContainer ? 1 : 0) << L'\n';
    std::wcout << L"PROCESS_TOKEN_FULL_TRUST_SHAPE\t"
               << (!appContainer && rid >= SECURITY_MANDATORY_MEDIUM_RID &&
                       rid < SECURITY_MANDATORY_HIGH_RID && !elevated
                       ? 1
                       : 0)
               << L'\n';

    LONG aumidStatus = ERROR_SUCCESS;
    const auto aumid = CurrentApplicationUserModelId(aumidStatus);
    std::wcout << L"CURRENT_AUMID_QUERY_STATUS\t" << aumidStatus << L'\n';
    std::wcout << L"CURRENT_AUMID_PRESENT\t" << (!aumid.empty() ? 1 : 0) << L'\n';
    if (!aumid.empty()) {
        std::wcout << L"CURRENT_AUMID\t" << aumid << L'\n';
    }
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
    PrintProcessTrustDiagnostics();
    return 0;
}

std::wstring PropertyValueText(const winrt::Windows::Foundation::IInspectable& value) {
    if (!value) {
        return L"<null>";
    }
    const auto property = value.try_as<winrt::Windows::Foundation::IPropertyValue>();
    if (!property) {
        return L"<inspectable>";
    }

    using winrt::Windows::Foundation::PropertyType;
    switch (property.Type()) {
    case PropertyType::String:
        return std::wstring(property.GetString().c_str());
    case PropertyType::Guid:
        return std::wstring(winrt::to_hstring(property.GetGuid()).c_str());
    case PropertyType::Boolean:
        return property.GetBoolean() ? L"true" : L"false";
    case PropertyType::Int32:
        return std::to_wstring(property.GetInt32());
    case PropertyType::UInt32:
        return std::to_wstring(property.GetUInt32());
    case PropertyType::Int64:
        return std::to_wstring(property.GetInt64());
    case PropertyType::UInt64:
        return std::to_wstring(property.GetUInt64());
    case PropertyType::Single:
        return std::to_wstring(property.GetSingle());
    case PropertyType::Double:
        return std::to_wstring(property.GetDouble());
    default:
        return L"<property-type-" + std::to_wstring(static_cast<int>(property.Type())) + L">";
    }
}

bool ContainsInsensitive(const std::wstring& haystack, const std::wstring& needle) {
    if (needle.empty() || haystack.size() < needle.size()) {
        return false;
    }
    std::wstring haystackLower = haystack;
    std::wstring needleLower = needle;
    std::transform(
        haystackLower.begin(), haystackLower.end(), haystackLower.begin(),
        [](wchar_t value) { return static_cast<wchar_t>(std::towlower(value)); });
    std::transform(
        needleLower.begin(), needleLower.end(), needleLower.begin(),
        [](wchar_t value) { return static_cast<wchar_t>(std::towlower(value)); });
    return haystackLower.find(needleLower) != std::wstring::npos;
}

int ProbeMediaComponentPackageInfo(const wchar_t* category, bool trustedOnly) {
    using GetMediaComponentPackageInfoFn = HRESULT(WINAPI*)(bool, HSTRING, void**);

    HMODULE module = LoadLibraryW(L"CompPkgSup.dll");
    if (module == nullptr) {
        const DWORD error = GetLastError();
        std::wcout << L"MEDIA_COMPONENT_QUERY_API_AVAILABLE\t0\n";
        std::wcout << L"MEDIA_COMPONENT_QUERY_LOAD_ERROR\t" << error << L'\n';
        return 11;
    }

    const auto getInfo = reinterpret_cast<GetMediaComponentPackageInfoFn>(
        GetProcAddress(module, "GetMediaComponentPackageInfo"));
    if (getInfo == nullptr) {
        std::wcout << L"MEDIA_COMPONENT_QUERY_API_AVAILABLE\t0\n";
        FreeLibrary(module);
        return 11;
    }

    std::wcout << L"MEDIA_COMPONENT_QUERY_API_AVAILABLE\t1\n";
    std::wcout << L"MEDIA_COMPONENT_QUERY_CATEGORY\t" << category << L'\n';
    std::wcout << L"MEDIA_COMPONENT_QUERY_TRUSTED_ONLY\t" << (trustedOnly ? 1 : 0) << L'\n';

    winrt::hstring categoryValue{category};
    void* rawVector = nullptr;
    const HRESULT hr = getInfo(
        trustedOnly,
        reinterpret_cast<HSTRING>(winrt::get_abi(categoryValue)),
        &rawVector);
    FreeLibrary(module);

    std::wcout << L"MEDIA_COMPONENT_QUERY_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(hr)
               << std::dec << L'\n';
    if (FAILED(hr) || rawVector == nullptr) {
        std::wcout << L"MEDIA_COMPONENT_QUERY_RESULT_AVAILABLE\t0\n";
        return FAILED(hr) ? 11 : 0;
    }

    IVector<IPropertySet> packages{rawVector, winrt::take_ownership_from_abi};
    std::wcout << L"MEDIA_COMPONENT_QUERY_RESULT_AVAILABLE\t1\n";
    std::wcout << L"MEDIA_COMPONENT_QUERY_COUNT\t" << packages.Size() << L'\n';

    const std::wstring targetFamily = Package::Current().Id().FamilyName().c_str();
    bool omniphonyFound = false;
    std::uint32_t matchCount = 0;
    for (std::uint32_t index = 0; index < packages.Size(); ++index) {
        const auto properties = packages.GetAt(index);
        bool entryMatches = false;
        for (const auto& pair : properties) {
            const std::wstring value = PropertyValueText(pair.Value());
            if (ContainsInsensitive(value, targetFamily) || ContainsInsensitive(value, kFormatGuid)) {
                entryMatches = true;
            }
        }
        if (!entryMatches) {
            continue;
        }

        omniphonyFound = true;
        ++matchCount;
        std::wcout << L"MEDIA_COMPONENT_OMNIPHONY_ENTRY_INDEX\t" << index << L'\n';
        for (const auto& pair : properties) {
            std::wcout << L"MEDIA_COMPONENT_OMNIPHONY_PROPERTY\t"
                       << pair.Key().c_str() << L"\t" << PropertyValueText(pair.Value()) << L'\n';
        }
    }

    std::wcout << L"MEDIA_COMPONENT_OMNIPHONY_FOUND\t" << (omniphonyFound ? 1 : 0) << L'\n';
    std::wcout << L"MEDIA_COMPONENT_OMNIPHONY_MATCH_COUNT\t" << matchCount << L'\n';
    return 0;
}

void ProbeMediaComponentTrustMetadata() {
    std::wcout << L"MEDIA_COMPONENT_TRUST_PROBE_BEGIN\t1\n";
    ProbeMediaComponentPackageInfo(L"MediaPlayback", false);
    ProbeMediaComponentPackageInfo(L"MediaPlayback", true);
    ProbeMediaComponentPackageInfo(L"windows.mediaPlayback", false);
    ProbeMediaComponentPackageInfo(L"windows.mediaPlayback", true);
    std::wcout << L"MEDIA_COMPONENT_TRUST_PROBE_END\t1\n";
}

int RegisterCurrentMediaExtension(HRESULT* registrationResult = nullptr) {
    using RegisterMediaExtensionPackageFn = HRESULT(WINAPI*)(PCWSTR);

    const auto package = Package::Current();
    const auto familyName = package.Id().FamilyName();
    HMODULE module = LoadLibraryW(L"CompPkgSup.dll");
    if (module == nullptr) {
        const auto error = GetLastError();
        std::wcout << L"MEDIA_EXTENSION_REGISTER_API_AVAILABLE\t0\n";
        std::wcerr << L"ERROR\tCompPkgSup.dll unavailable\t" << error << L'\n';
        if (registrationResult != nullptr) {
            *registrationResult = HRESULT_FROM_WIN32(error);
        }
        return 6;
    }

    const auto registerMediaExtensionPackage = reinterpret_cast<RegisterMediaExtensionPackageFn>(
        GetProcAddress(module, "RegisterMediaExtensionPackage"));
    if (registerMediaExtensionPackage == nullptr) {
        std::wcout << L"MEDIA_EXTENSION_REGISTER_API_AVAILABLE\t0\n";
        std::wcout << L"MEDIA_EXTENSION_REGISTER_REQUIRES_WINDOWS_11_24H2\t1\n";
        FreeLibrary(module);
        if (registrationResult != nullptr) {
            *registrationResult = HRESULT_FROM_WIN32(ERROR_PROC_NOT_FOUND);
        }
        return 6;
    }

    std::wcout << L"MEDIA_EXTENSION_REGISTER_API_AVAILABLE\t1\n";
    std::wcout << L"MEDIA_EXTENSION_PACKAGE_FAMILY\t" << familyName.c_str() << L'\n';
    const HRESULT result = registerMediaExtensionPackage(familyName.c_str());
    FreeLibrary(module);

    if (registrationResult != nullptr) {
        *registrationResult = result;
    }
    std::wcout << L"MEDIA_EXTENSION_REGISTER_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(result) << std::dec << L'\n';
    if (FAILED(result)) {
        std::wcout << L"MEDIA_EXTENSION_REGISTERED\t0\n";
        return 6;
    }
    std::wcout << L"MEDIA_EXTENSION_REGISTERED\t1\n";
    return 0;
}

std::filesystem::path ActivationProbeResultPath() {
    wchar_t temp[32768]{};
    const DWORD length = GetTempPathW(static_cast<DWORD>(std::size(temp)), temp);
    if (length == 0 || length >= std::size(temp)) {
        throw std::runtime_error("GetTempPathW failed for activation probe");
    }
    return std::filesystem::path(temp) /
        (L"OmniphonySpatialRegisterAumid-" + std::to_wstring(GetCurrentProcessId()) + L".txt");
}

bool WriteActivationProbeResult(const std::filesystem::path& path, HRESULT hr) {
    std::wofstream output(path, std::ios::trunc);
    if (!output) {
        return false;
    }
    output << L"0x" << std::hex << std::uppercase << static_cast<std::uint32_t>(hr) << L'\n';
    return static_cast<bool>(output);
}

bool ReadActivationProbeResult(const std::filesystem::path& path, HRESULT& hr) {
    std::wifstream input(path);
    if (!input) {
        return false;
    }
    std::wstring value;
    input >> value;
    if (value.size() < 3 || value.rfind(L"0x", 0) != 0) {
        return false;
    }
    try {
        const auto parsed = std::stoul(value.substr(2), nullptr, 16);
        hr = static_cast<HRESULT>(static_cast<std::uint32_t>(parsed));
        return true;
    } catch (...) {
        return false;
    }
}

int RegisterActivatedProbe(const wchar_t* resultPath) {
    HRESULT registrationResult = E_FAIL;
    const int exitCode = RegisterCurrentMediaExtension(&registrationResult);
    const bool written = WriteActivationProbeResult(std::filesystem::path(resultPath), registrationResult);
    return written ? exitCode : 10;
}

int ActivateRegisterThroughAumid() {
    const auto package = Package::Current();
    const std::wstring aumid = std::wstring(package.Id().FamilyName().c_str()) + L"!" + kApplicationId;
    const auto resultPath = ActivationProbeResultPath();
    std::error_code ec;
    std::filesystem::remove(resultPath, ec);

    const std::wstring arguments = L"register-activated \"" + resultPath.wstring() + L"\"";
    winrt::com_ptr<IApplicationActivationManager> manager;
    const HRESULT createResult = CoCreateInstance(
        CLSID_ApplicationActivationManager,
        nullptr,
        CLSCTX_LOCAL_SERVER,
        __uuidof(IApplicationActivationManager),
        manager.put_void());
    std::wcout << L"AUMID_ACTIVATION_MANAGER_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(createResult)
               << std::dec << L'\n';
    if (FAILED(createResult)) {
        return 6;
    }

    DWORD processId = 0;
    const HRESULT activateResult = manager->ActivateApplication(
        aumid.c_str(),
        arguments.c_str(),
        AO_NONE,
        &processId);
    std::wcout << L"AUMID_REGISTER_APPLICATION_ID\t" << aumid << L'\n';
    std::wcout << L"AUMID_REGISTER_ACTIVATE_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(activateResult)
               << std::dec << L'\n';
    std::wcout << L"AUMID_REGISTER_ACTIVATED_PID\t" << processId << L'\n';
    if (FAILED(activateResult)) {
        return 6;
    }

    HANDLE process = nullptr;
    if (processId != 0) {
        process = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, FALSE, processId);
    }
    if (process != nullptr) {
        WaitForSingleObject(process, 10000);
        CloseHandle(process);
    }

    HRESULT registrationResult = E_FAIL;
    bool resultAvailable = false;
    for (int attempt = 0; attempt != 40; ++attempt) {
        if (ReadActivationProbeResult(resultPath, registrationResult)) {
            resultAvailable = true;
            break;
        }
        Sleep(250);
    }
    std::filesystem::remove(resultPath, ec);

    std::wcout << L"AUMID_REGISTER_RESULT_AVAILABLE\t" << (resultAvailable ? 1 : 0) << L'\n';
    if (!resultAvailable) {
        return 6;
    }
    std::wcout << L"AUMID_REGISTER_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(registrationResult)
               << std::dec << L'\n';
    std::wcout << L"AUMID_REGISTERED\t" << (SUCCEEDED(registrationResult) ? 1 : 0) << L'\n';
    return SUCCEEDED(registrationResult) ? 0 : 6;
}

int RegisterWithActivationFallback() {
    ProbeMediaComponentTrustMetadata();

    HRESULT directResult = E_FAIL;
    const int directExit = RegisterCurrentMediaExtension(&directResult);
    if (directExit == 0) {
        return 0;
    }

    if (directResult != E_ACCESSDENIED) {
        return directExit;
    }

    std::wcout << L"MEDIA_EXTENSION_REGISTER_ACTIVATION_FALLBACK\t1\n";
    return ActivateRegisterThroughAumid();
}

int NotifySpatialFormatChanged() {
    const auto formatConfiguration = SpatialAudioFormatConfiguration::GetDefault();
    formatConfiguration.ReportLicenseChangedAsync(winrt::hstring{kFormatGuid}).get();
    std::wcout << L"SPATIAL_LICENSE_CHANGE_REPORTED\t1\n";
    formatConfiguration.ReportConfigurationChangedAsync(winrt::hstring{kFormatGuid}).get();
    std::wcout << L"SPATIAL_CONFIGURATION_CHANGE_REPORTED\t1\n";
    return 0;
}

std::wstring DefaultRenderEndpointId() {
    winrt::com_ptr<IMMDeviceEnumerator> enumerator;
    winrt::check_hresult(CoCreateInstance(
        __uuidof(MMDeviceEnumerator),
        nullptr,
        CLSCTX_ALL,
        __uuidof(IMMDeviceEnumerator),
        enumerator.put_void()));

    winrt::com_ptr<IMMDevice> device;
    winrt::check_hresult(enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, device.put()));

    LPWSTR rawEndpointId = nullptr;
    winrt::check_hresult(device->GetId(&rawEndpointId));
    std::wstring endpointId;
    if (rawEndpointId != nullptr) {
        endpointId.assign(rawEndpointId);
        CoTaskMemFree(rawEndpointId);
    }
    if (endpointId.empty()) {
        throw winrt::hresult_error(E_FAIL, L"Default render endpoint returned an empty device ID.");
    }

    std::wcout << L"DEFAULT_RENDER_ROLE\teMultimedia\n";
    std::wcout << L"DEFAULT_RENDER_ENDPOINT_DISCOVERED\t1\n";
    std::wcout << L"DEFAULT_RENDER_ENDPOINT_ID\t" << endpointId << L'\n';
    return endpointId;
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

int VerifyDefaultEndpoint() {
    std::wcout << L"SPATIAL_OWNERSHIP_VERIFY_DEFAULT_BEGIN\t1\n";
    PrintIdentity();

    int result = RegisterWithActivationFallback();
    if (result != 0) {
        std::wcout << L"SPATIAL_OWNERSHIP_VERIFY_DEFAULT_OK\t0\n";
        return result;
    }

    result = NotifySpatialFormatChanged();
    if (result != 0) {
        std::wcout << L"SPATIAL_OWNERSHIP_VERIFY_DEFAULT_OK\t0\n";
        return result;
    }

    const auto endpointId = DefaultRenderEndpointId();
    std::wcout << L"VERIFY_DEFAULT_STATUS_BEFORE\n";
    result = SelectionStatus(endpointId.c_str());
    if (result != 0) {
        std::wcout << L"SPATIAL_OWNERSHIP_VERIFY_DEFAULT_OK\t0\n";
        return result;
    }

    std::wcout << L"VERIFY_DEFAULT_SELECT\n";
    result = SelectEndpoint(endpointId.c_str());
    if (result != 0) {
        std::wcout << L"SPATIAL_OWNERSHIP_VERIFY_DEFAULT_OK\t0\n";
        return result;
    }

    std::wcout << L"SPATIAL_OWNERSHIP_VERIFY_DEFAULT_OK\t1\n";
    return 0;
}

void Usage() {
    std::wcerr
        << L"usage: OmniphonySpatialCompanion <command> [endpoint-id]\n"
        << L"  identity              prove the process is running with package/application identity\n"
        << L"  register              inspect media-component trust, then register with AUMID fallback\n"
        << L"  notify                report license/configuration change for Omniphony\n"
        << L"  status <endpoint-id>  read spatial selection state from packaged identity\n"
        << L"  select <endpoint-id>  ask Windows to select Omniphony from packaged identity\n"
        << L"  verify-default        run the ownership gate against the default multimedia render endpoint\n";
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
            return RegisterWithActivationFallback();
        }
        if (command == L"register-activated" && argc == 3) {
            return RegisterActivatedProbe(argv[2]);
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
        if (command == L"verify-default" && argc == 2) {
            return VerifyDefaultEndpoint();
        }
        Usage();
        return 2;
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"ERROR\tWinRT\t0x" << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(error.code().value)
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