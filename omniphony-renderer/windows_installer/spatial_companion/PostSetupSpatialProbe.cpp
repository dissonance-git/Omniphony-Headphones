#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <winstring.h>
#include <mmdeviceapi.h>

#include <winrt/base.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>

#include <cstdint>
#include <cstdlib>
#include <iomanip>
#include <iostream>
#include <string>

namespace {

using winrt::Windows::Foundation::IInspectable;
using winrt::Windows::Foundation::IPropertyValue;
using winrt::Windows::Foundation::PropertyType;
using winrt::Windows::Foundation::Collections::IIterable;
using winrt::Windows::Foundation::Collections::IPropertySet;
using winrt::Windows::Foundation::Collections::IVector;

constexpr wchar_t kPackageFamilyName[] = L"Omniphony.SpatialCompanion_1nv7pqmcjcq0w";
constexpr wchar_t kAliasName[] = L"OmniphonySpatialCompanion.exe";

DWORD CurrentIntegrityRid() {
    HANDLE token = nullptr;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) {
        return 0;
    }

    DWORD required = 0;
    GetTokenInformation(token, TokenIntegrityLevel, nullptr, 0, &required);
    if (required == 0) {
        CloseHandle(token);
        return 0;
    }

    auto buffer = std::make_unique<std::uint8_t[]>(required);
    if (!GetTokenInformation(token, TokenIntegrityLevel, buffer.get(), required, &required)) {
        CloseHandle(token);
        return 0;
    }
    CloseHandle(token);

    const auto label = reinterpret_cast<TOKEN_MANDATORY_LABEL*>(buffer.get());
    const auto count = GetSidSubAuthorityCount(label->Label.Sid);
    if (count == nullptr || *count == 0) {
        return 0;
    }
    return *GetSidSubAuthority(label->Label.Sid, *count - 1);
}

std::wstring ScalarText(const IInspectable& value) {
    if (!value) {
        return L"<null>";
    }

    const auto property = value.try_as<IPropertyValue>();
    if (!property) {
        return L"<inspectable:" + std::wstring(winrt::get_class_name(value).c_str()) + L">";
    }

    switch (property.Type()) {
    case PropertyType::String:
        return std::wstring(property.GetString().c_str());
    case PropertyType::Guid:
        return std::wstring(winrt::to_hstring(property.GetGuid()).c_str());
    case PropertyType::Boolean:
        return property.GetBoolean() ? L"true" : L"false";
    case PropertyType::Int16:
        return std::to_wstring(property.GetInt16());
    case PropertyType::UInt16:
        return std::to_wstring(property.GetUInt16());
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

void DumpInspectable(const std::wstring& path, const IInspectable& value, int depth) {
    if (depth > 8 || !value) {
        return;
    }

    if (const auto property = value.try_as<IPropertyValue>()) {
        std::wcout << L"POST_SETUP_MEDIA_PROPERTY\t" << path << L"\t" << ScalarText(value) << L'\n';
        return;
    }

    if (const auto set = value.try_as<IPropertySet>()) {
        std::wcout << L"POST_SETUP_MEDIA_PROPERTYSET\t" << path << L"\t" << set.Size() << L'\n';
        for (const auto& pair : set) {
            const std::wstring childPath = path.empty()
                ? std::wstring(pair.Key().c_str())
                : path + L"." + pair.Key().c_str();
            DumpInspectable(childPath, pair.Value(), depth + 1);
        }
        return;
    }

    if (const auto vector = value.try_as<IVector<IPropertySet>>()) {
        std::wcout << L"POST_SETUP_MEDIA_VECTOR_PROPERTYSET\t" << path << L"\t" << vector.Size() << L'\n';
        for (std::uint32_t i = 0; i < vector.Size(); ++i) {
            DumpInspectable(path + L"[" + std::to_wstring(i) + L"]", vector.GetAt(i), depth + 1);
        }
        return;
    }

    if (const auto vector = value.try_as<IVector<IInspectable>>()) {
        std::wcout << L"POST_SETUP_MEDIA_VECTOR_INSPECTABLE\t" << path << L"\t" << vector.Size() << L'\n';
        for (std::uint32_t i = 0; i < vector.Size(); ++i) {
            DumpInspectable(path + L"[" + std::to_wstring(i) + L"]", vector.GetAt(i), depth + 1);
        }
        return;
    }

    if (const auto iterable = value.try_as<IIterable<IInspectable>>()) {
        std::uint32_t i = 0;
        for (const auto& item : iterable) {
            DumpInspectable(path + L"[" + std::to_wstring(i++) + L"]", item, depth + 1);
        }
        if (i != 0) {
            std::wcout << L"POST_SETUP_MEDIA_ITERABLE_COUNT\t" << path << L"\t" << i << L'\n';
            return;
        }
    }

    std::wcout << L"POST_SETUP_MEDIA_OPAQUE\t" << path << L"\t"
               << winrt::get_class_name(value).c_str() << L'\n';
}

void DumpOmniphonyMediaComponent() {
    using GetMediaComponentPackageInfoFn = HRESULT(WINAPI*)(bool, HSTRING, void**);

    HMODULE module = LoadLibraryW(L"CompPkgSup.dll");
    if (module == nullptr) {
        std::wcout << L"POST_SETUP_MEDIA_QUERY_API_AVAILABLE\t0\n";
        return;
    }

    const auto getInfo = reinterpret_cast<GetMediaComponentPackageInfoFn>(
        GetProcAddress(module, "GetMediaComponentPackageInfo"));
    if (getInfo == nullptr) {
        std::wcout << L"POST_SETUP_MEDIA_QUERY_API_AVAILABLE\t0\n";
        FreeLibrary(module);
        return;
    }

    std::wcout << L"POST_SETUP_MEDIA_QUERY_API_AVAILABLE\t1\n";
    winrt::hstring category{L"windows.mediaPlayback"};
    void* rawVector = nullptr;
    const HRESULT hr = getInfo(false, reinterpret_cast<HSTRING>(winrt::get_abi(category)), &rawVector);
    FreeLibrary(module);

    std::wcout << L"POST_SETUP_MEDIA_QUERY_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(hr)
               << std::dec << L'\n';
    if (FAILED(hr) || rawVector == nullptr) {
        std::wcout << L"POST_SETUP_MEDIA_OMNIPHONY_FOUND\t0\n";
        return;
    }

    IVector<IPropertySet> packages{rawVector, winrt::take_ownership_from_abi};
    bool found = false;
    for (std::uint32_t i = 0; i < packages.Size(); ++i) {
        const auto set = packages.GetAt(i);
        bool matches = false;
        for (const auto& pair : set) {
            if (_wcsicmp(pair.Key().c_str(), L"@PackageFamilyName") == 0 &&
                _wcsicmp(ScalarText(pair.Value()).c_str(), kPackageFamilyName) == 0) {
                matches = true;
                break;
            }
        }
        if (!matches) {
            continue;
        }

        found = true;
        std::wcout << L"POST_SETUP_MEDIA_OMNIPHONY_ENTRY_INDEX\t" << i << L'\n';
        for (const auto& pair : set) {
            DumpInspectable(std::wstring(pair.Key().c_str()), pair.Value(), 0);
        }
    }
    std::wcout << L"POST_SETUP_MEDIA_OMNIPHONY_FOUND\t" << (found ? 1 : 0) << L'\n';
}

std::wstring ExecutionAliasPath() {
    DWORD required = GetEnvironmentVariableW(L"LOCALAPPDATA", nullptr, 0);
    if (required == 0) {
        return {};
    }
    std::wstring localAppData(required, L'\0');
    const DWORD written = GetEnvironmentVariableW(L"LOCALAPPDATA", localAppData.data(), required);
    if (written == 0 || written >= required) {
        return {};
    }
    localAppData.resize(written);
    return localAppData + L"\\Microsoft\\WindowsApps\\" + kAliasName;
}

DWORD RunPackagedCommand(const std::wstring& arguments, const wchar_t* marker) {
    const auto alias = ExecutionAliasPath();
    if (alias.empty()) {
        std::wcout << marker << L"_LAUNCHED\t0\n";
        return ERROR_PATH_NOT_FOUND;
    }

    std::wstring commandLine = L"\"" + alias + L"\" " + arguments;
    STARTUPINFOW startup{};
    startup.cb = sizeof(startup);
    PROCESS_INFORMATION process{};
    const BOOL created = CreateProcessW(
        alias.c_str(),
        commandLine.data(),
        nullptr,
        nullptr,
        TRUE,
        0,
        nullptr,
        nullptr,
        &startup,
        &process);
    std::wcout << marker << L"_LAUNCHED\t" << (created ? 1 : 0) << L'\n';
    if (!created) {
        std::wcout << marker << L"_CREATE_ERROR\t" << GetLastError() << L'\n';
        return GetLastError();
    }

    WaitForSingleObject(process.hProcess, 30000);
    DWORD exitCode = STILL_ACTIVE;
    GetExitCodeProcess(process.hProcess, &exitCode);
    CloseHandle(process.hThread);
    CloseHandle(process.hProcess);
    std::wcout << marker << L"_EXIT_CODE\t" << exitCode << L'\n';
    return exitCode;
}

std::wstring DefaultRenderEndpointId() {
    winrt::com_ptr<IMMDeviceEnumerator> enumerator;
    const HRESULT create = CoCreateInstance(
        __uuidof(MMDeviceEnumerator),
        nullptr,
        CLSCTX_ALL,
        __uuidof(IMMDeviceEnumerator),
        enumerator.put_void());
    std::wcout << L"POST_SETUP_DEFAULT_ENDPOINT_ENUMERATOR_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(create)
               << std::dec << L'\n';
    if (FAILED(create)) {
        return {};
    }

    winrt::com_ptr<IMMDevice> device;
    const HRESULT getDefault = enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, device.put());
    std::wcout << L"POST_SETUP_DEFAULT_ENDPOINT_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(getDefault)
               << std::dec << L'\n';
    if (FAILED(getDefault)) {
        return {};
    }

    LPWSTR rawId = nullptr;
    const HRESULT getId = device->GetId(&rawId);
    if (FAILED(getId) || rawId == nullptr) {
        if (rawId != nullptr) {
            CoTaskMemFree(rawId);
        }
        return {};
    }
    std::wstring id(rawId);
    CoTaskMemFree(rawId);
    return id;
}

void PostSetupProbe() {
    const DWORD rid = CurrentIntegrityRid();
    if (rid < SECURITY_MANDATORY_MEDIUM_RID || rid >= SECURITY_MANDATORY_HIGH_RID) {
        return;
    }

    std::wcout << L"POST_SETUP_SPATIAL_PROBE_BEGIN\t1\n";
    std::wcout << L"POST_SETUP_PROCESS_INTEGRITY_RID\t0x"
               << std::hex << std::uppercase << rid << std::dec << L'\n';

    const HRESULT apartment = RoInitialize(RO_INIT_MULTITHREADED);
    std::wcout << L"POST_SETUP_ROINITIALIZE_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<std::uint32_t>(apartment)
               << std::dec << L'\n';

    try {
        DumpOmniphonyMediaComponent();
    } catch (const winrt::hresult_error& error) {
        std::wcout << L"POST_SETUP_MEDIA_PROBE_EXCEPTION\t0x"
                   << std::hex << std::uppercase << static_cast<std::uint32_t>(error.code().value)
                   << std::dec << L'\n';
    } catch (...) {
        std::wcout << L"POST_SETUP_MEDIA_PROBE_EXCEPTION\tUNKNOWN\n";
    }

    const DWORD notifyExit = RunPackagedCommand(L"notify", L"POST_SETUP_NOTIFY");
    std::wcout << L"POST_SETUP_NOTIFY_OK\t" << (notifyExit == 0 ? 1 : 0) << L'\n';

    const auto endpointId = DefaultRenderEndpointId();
    std::wcout << L"POST_SETUP_DEFAULT_ENDPOINT_DISCOVERED\t" << (!endpointId.empty() ? 1 : 0) << L'\n';
    if (!endpointId.empty()) {
        std::wcout << L"POST_SETUP_DEFAULT_ENDPOINT_ID\t" << endpointId << L'\n';
        const std::wstring selectArgs = L"select \"" + endpointId + L"\"";
        const DWORD selectExit = RunPackagedCommand(selectArgs, L"POST_SETUP_SELECT");
        std::wcout << L"POST_SETUP_SETTER_ACCEPTED\t" << (selectExit == 0 ? 1 : 0) << L'\n';
    }

    if (SUCCEEDED(apartment)) {
        RoUninitialize();
    }
    std::wcout << L"POST_SETUP_SPATIAL_PROBE_END\t1\n";
}

struct PostSetupRegistration {
    PostSetupRegistration() {
        std::atexit(PostSetupProbe);
    }
};

PostSetupRegistration g_postSetupRegistration;

} // namespace
