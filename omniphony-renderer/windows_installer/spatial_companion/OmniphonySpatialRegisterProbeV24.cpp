#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shellapi.h>

#include <cstdint>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <iterator>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace {

constexpr wchar_t kDefaultPackageFamily[] = L"Omniphony.SpatialCompanion_1nv7pqmcjcq0w";

struct UniqueHandle {
    HANDLE value = nullptr;

    UniqueHandle() = default;
    explicit UniqueHandle(HANDLE handle) : value(handle) {}
    UniqueHandle(const UniqueHandle&) = delete;
    UniqueHandle& operator=(const UniqueHandle&) = delete;
    ~UniqueHandle() {
        if (value != nullptr && value != INVALID_HANDLE_VALUE) {
            CloseHandle(value);
        }
    }
};

struct TokenState {
    DWORD integrityRid = 0;
    bool elevated = false;
    bool appContainer = false;
};

DWORD IntegrityRid(HANDLE token) {
    DWORD required = 0;
    GetTokenInformation(token, TokenIntegrityLevel, nullptr, 0, &required);
    if (required == 0) {
        throw std::runtime_error("GetTokenInformation(TokenIntegrityLevel) size failed");
    }

    std::vector<std::uint8_t> buffer(required);
    if (!GetTokenInformation(token, TokenIntegrityLevel, buffer.data(), required, &required)) {
        throw std::runtime_error("GetTokenInformation(TokenIntegrityLevel) failed");
    }

    const auto label = reinterpret_cast<TOKEN_MANDATORY_LABEL*>(buffer.data());
    const UCHAR* count = GetSidSubAuthorityCount(label->Label.Sid);
    if (count == nullptr || *count == 0) {
        throw std::runtime_error("invalid integrity SID");
    }
    return *GetSidSubAuthority(label->Label.Sid, *count - 1);
}

TokenState CurrentTokenState() {
    HANDLE rawToken = nullptr;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &rawToken)) {
        throw std::runtime_error("OpenProcessToken failed");
    }
    UniqueHandle token(rawToken);

    TOKEN_ELEVATION elevation{};
    DWORD returned = 0;
    if (!GetTokenInformation(token.value, TokenElevation, &elevation, sizeof(elevation), &returned)) {
        throw std::runtime_error("GetTokenInformation(TokenElevation) failed");
    }

    DWORD appContainer = 0;
    returned = 0;
    if (!GetTokenInformation(
            token.value,
            TokenIsAppContainer,
            &appContainer,
            sizeof(appContainer),
            &returned)) {
        throw std::runtime_error("GetTokenInformation(TokenIsAppContainer) failed");
    }

    return {IntegrityRid(token.value), elevation.TokenIsElevated != 0, appContainer != 0};
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

std::wstring HexHresult(HRESULT hr) {
    std::wostringstream stream;
    stream << L"0x" << std::hex << std::uppercase
           << static_cast<std::uint32_t>(hr);
    return stream.str();
}

HRESULT RegisterPackage(const std::wstring& familyName, bool& apiAvailable) {
    using RegisterMediaExtensionPackageFn = HRESULT(WINAPI*)(PCWSTR);

    HMODULE module = LoadLibraryW(L"CompPkgSup.dll");
    if (module == nullptr) {
        apiAvailable = false;
        return HRESULT_FROM_WIN32(GetLastError());
    }

    const auto function = reinterpret_cast<RegisterMediaExtensionPackageFn>(
        GetProcAddress(module, "RegisterMediaExtensionPackage"));
    if (function == nullptr) {
        const HRESULT hr = HRESULT_FROM_WIN32(GetLastError());
        FreeLibrary(module);
        apiAvailable = false;
        return hr;
    }

    apiAvailable = true;
    const HRESULT hr = function(familyName.c_str());
    FreeLibrary(module);
    return hr;
}

void EmitContext(
    std::wostream& output,
    const wchar_t* context,
    const std::wstring& familyName,
    const TokenState& state,
    bool apiAvailable,
    HRESULT hr) {
    output << L"REGISTER_PROBE_CONTEXT\t" << context << L"\n";
    output << L"REGISTER_PROBE_PACKAGE_FAMILY\t" << familyName << L"\n";
    output << L"REGISTER_PROBE_PROCESS_INTEGRITY\t" << IntegrityName(state.integrityRid) << L"\n";
    output << L"REGISTER_PROBE_PROCESS_INTEGRITY_RID\t0x"
           << std::hex << std::uppercase << state.integrityRid << std::dec << L"\n";
    output << L"REGISTER_PROBE_PROCESS_ELEVATED\t" << (state.elevated ? 1 : 0) << L"\n";
    output << L"REGISTER_PROBE_PROCESS_TOKEN_IS_APPCONTAINER\t" << (state.appContainer ? 1 : 0) << L"\n";
    output << L"REGISTER_PROBE_API_AVAILABLE\t" << (apiAvailable ? 1 : 0) << L"\n";
    output << L"REGISTER_PROBE_HRESULT\t" << HexHresult(hr) << L"\n";
    output << L"REGISTER_PROBE_REGISTERED\t" << (SUCCEEDED(hr) ? 1 : 0) << L"\n";
}

std::wstring SelfPath() {
    std::vector<wchar_t> buffer(32768);
    const DWORD length = GetModuleFileNameW(nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
    if (length == 0 || length >= buffer.size()) {
        throw std::runtime_error("GetModuleFileNameW failed");
    }
    return std::wstring(buffer.data(), length);
}

std::wstring TemporaryResultPath() {
    wchar_t temp[32768]{};
    const DWORD length = GetTempPathW(static_cast<DWORD>(std::size(temp)), temp);
    if (length == 0 || length >= std::size(temp)) {
        throw std::runtime_error("GetTempPathW failed");
    }
    return std::wstring(temp) + L"OmniphonySpatialRegisterProbeV24-" +
        std::to_wstring(GetCurrentProcessId()) + L".txt";
}

int ElevatedChild(const std::wstring& familyName, const std::wstring& resultPath) {
    const TokenState state = CurrentTokenState();
    bool apiAvailable = false;
    const HRESULT hr = RegisterPackage(familyName, apiAvailable);

    std::wofstream output(resultPath, std::ios::trunc);
    if (!output) {
        return 41;
    }
    EmitContext(output, L"ELEVATED_CHILD", familyName, state, apiAvailable, hr);
    output.flush();
    return SUCCEEDED(hr) ? 0 : 23;
}

int LaunchElevatedChild(const std::wstring& familyName, const std::wstring& resultPath) {
    const std::wstring self = SelfPath();
    const std::wstring parameters =
        L"--elevated \"" + familyName + L"\" \"" + resultPath + L"\" --capture";

    SHELLEXECUTEINFOW execute{};
    execute.cbSize = sizeof(execute);
    execute.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI;
    execute.lpVerb = L"runas";
    execute.lpFile = self.c_str();
    execute.lpParameters = parameters.c_str();
    execute.nShow = SW_HIDE;

    std::wcout << L"REGISTER_PROBE_ELEVATION_REQUESTED\t1\n";
    if (!ShellExecuteExW(&execute)) {
        const DWORD error = GetLastError();
        std::wcout << L"REGISTER_PROBE_ELEVATION_LAUNCHED\t0\n";
        std::wcout << L"REGISTER_PROBE_ELEVATION_WIN32_ERROR\t" << error << L"\n";
        return 40;
    }
    if (execute.hProcess == nullptr) {
        std::wcout << L"REGISTER_PROBE_ELEVATION_LAUNCHED\t0\n";
        return 40;
    }

    UniqueHandle process(execute.hProcess);
    std::wcout << L"REGISTER_PROBE_ELEVATION_LAUNCHED\t1\n";
    WaitForSingleObject(process.value, INFINITE);

    DWORD exitCode = 1;
    if (!GetExitCodeProcess(process.value, &exitCode)) {
        return 40;
    }
    std::wcout << L"REGISTER_PROBE_ELEVATED_CHILD_EXIT_CODE\t" << exitCode << L"\n";
    return static_cast<int>(exitCode);
}

void PrintResultFile(const std::wstring& path) {
    std::wifstream input(path);
    if (!input) {
        std::wcout << L"REGISTER_PROBE_ELEVATED_RESULT_AVAILABLE\t0\n";
        return;
    }
    std::wcout << L"REGISTER_PROBE_ELEVATED_RESULT_AVAILABLE\t1\n";
    std::wstring line;
    while (std::getline(input, line)) {
        std::wcout << line << L"\n";
    }
    input.close();
    DeleteFileW(path.c_str());
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    try {
        if (argc >= 4 && std::wstring(argv[1]) == L"--elevated") {
            return ElevatedChild(argv[2], argv[3]);
        }

        const std::wstring familyName = argc >= 2 ? argv[1] : kDefaultPackageFamily;
        std::wcout << L"REGISTER_PROBE_VERSION\t24\n";
        std::wcout << L"REGISTER_PROBE_PURPOSE\tCompare RegisterMediaExtensionPackage at medium and elevated integrity\n";

        const TokenState state = CurrentTokenState();
        bool apiAvailable = false;
        const HRESULT mediumHr = RegisterPackage(familyName, apiAvailable);
        EmitContext(std::wcout, L"INITIAL_PROCESS", familyName, state, apiAvailable, mediumHr);

        if (state.elevated || state.integrityRid >= SECURITY_MANDATORY_HIGH_RID) {
            std::wcout << L"REGISTER_PROBE_MEDIUM_VS_ELEVATED_COMPARISON_AVAILABLE\t0\n";
            std::wcout << L"REGISTER_PROBE_ACTION\tRun this probe normally, not as administrator, to capture both contexts\n";
            return SUCCEEDED(mediumHr) ? 0 : 23;
        }

        const std::wstring resultPath = TemporaryResultPath();
        DeleteFileW(resultPath.c_str());
        const int elevatedExit = LaunchElevatedChild(familyName, resultPath);
        PrintResultFile(resultPath);
        std::wcout << L"REGISTER_PROBE_MEDIUM_VS_ELEVATED_COMPARISON_AVAILABLE\t1\n";
        std::wcout << L"REGISTER_PROBE_INITIAL_HRESULT\t" << HexHresult(mediumHr) << L"\n";
        std::wcout << L"REGISTER_PROBE_ELEVATED_SUCCESS\t" << (elevatedExit == 0 ? 1 : 0) << L"\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "REGISTER_PROBE_EXCEPTION\t" << error.what() << "\n";
        return 99;
    }
}
