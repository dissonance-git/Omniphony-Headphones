#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <filesystem>
#include <iostream>
#include <string>
#include <vector>

// Reuse the hardened exact-provider v15 implementation verbatim. Rename its
// entry point so this v21 wrapper can add the direct packaged AppService check
// before starting the ETW + real setter sequence.
#define wmain OmniphonySpatialLicenseEtlProbeV15Main
#include "OmniphonySpatialLicenseEtlProbeV15.cpp"
#undef wmain

namespace {

constexpr wchar_t kAppServiceProbeAlias[] = L"OmniphonySpatialAppServiceProbeV17.exe";

std::filesystem::path WindowsAppsAlias(const wchar_t* aliasName) {
    wchar_t localAppData[32768]{};
    const DWORD length = GetEnvironmentVariableW(
        L"LOCALAPPDATA",
        localAppData,
        static_cast<DWORD>(sizeof(localAppData) / sizeof(localAppData[0])));
    if (length == 0 || length >= sizeof(localAppData) / sizeof(localAppData[0])) {
        return {};
    }
    return std::filesystem::path(localAppData) / L"Microsoft" / L"WindowsApps" / aliasName;
}

DWORD RunPackagedAppServiceSelfTest() {
    const auto alias = WindowsAppsAlias(kAppServiceProbeAlias);
    if (alias.empty()) {
        std::wcout << L"OWNERSHIP_V21_APPSERVICE_PROBE_ALIAS_OK\t0\n";
        return ERROR_PATH_NOT_FOUND;
    }

    std::wstring commandLine = L"\"" + alias.wstring() + L"\"";
    std::vector<wchar_t> mutableCommand(commandLine.begin(), commandLine.end());
    mutableCommand.push_back(L'\0');

    STARTUPINFOW startup{};
    startup.cb = sizeof(startup);
    PROCESS_INFORMATION process{};
    const BOOL created = CreateProcessW(
        alias.c_str(),
        mutableCommand.data(),
        nullptr,
        nullptr,
        TRUE,
        0,
        nullptr,
        nullptr,
        &startup,
        &process);

    std::wcout << L"OWNERSHIP_V21_APPSERVICE_PROBE_ALIAS_OK\t" << (created ? 1 : 0) << L'\n';
    if (!created) {
        const DWORD error = GetLastError();
        std::wcout << L"OWNERSHIP_V21_APPSERVICE_PROBE_CREATE_ERROR\t" << error << L'\n';
        return error;
    }

    CloseHandle(process.hThread);
    const DWORD wait = WaitForSingleObject(process.hProcess, 30000);
    std::wcout << L"OWNERSHIP_V21_APPSERVICE_PROBE_WAIT\t" << wait << L'\n';

    DWORD exitCode = STILL_ACTIVE;
    GetExitCodeProcess(process.hProcess, &exitCode);
    CloseHandle(process.hProcess);
    std::wcout << L"OWNERSHIP_V21_APPSERVICE_PROBE_EXIT\t" << exitCode << L'\n';
    return exitCode;
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    // The v15 implementation self-elevates this same executable for the ETW
    // capture child. Do not recurse into the AppService self-test in that child.
    if (argc == 6 && _wcsicmp(argv[1], L"--capture") == 0) {
        return OmniphonySpatialLicenseEtlProbeV15Main(argc, argv);
    }

    std::wcout << L"OWNERSHIP_V21_PROBE_BEGIN\t1\n";
    std::wcout << L"OWNERSHIP_V21_EXPECTED_FORMAT_GUID\t{4BD75423-A66C-4586-B782-1FCBBDF2AE74}\n";
    std::wcout << L"OWNERSHIP_V21_SEQUENCE\tAPPSERVICE_THEN_EXACT_LICENSE_ETW_THEN_SETTER\n";

    const DWORD appServiceExit = RunPackagedAppServiceSelfTest();
    std::wcout << L"OWNERSHIP_V21_APPSERVICE_HEALTHY\t" << (appServiceExit == 0 ? 1 : 0) << L'\n';

    std::wcout << L"OWNERSHIP_V21_LICENSE_TRACE_BEGIN\t1\n";
    const int traceExit = OmniphonySpatialLicenseEtlProbeV15Main(argc, argv);
    std::wcout << L"OWNERSHIP_V21_LICENSE_TRACE_EXIT\t" << traceExit << L'\n';
    std::wcout << L"OWNERSHIP_V21_PROBE_END\t1\n";
    return traceExit;
}
