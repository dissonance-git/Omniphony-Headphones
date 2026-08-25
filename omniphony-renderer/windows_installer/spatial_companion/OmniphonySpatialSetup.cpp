#include <windows.h>
#include <shellapi.h>
#include <shlwapi.h>
#include <wincrypt.h>
#include <mmdeviceapi.h>

#include <winrt/base.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Management.Deployment.h>

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <limits>
#include <string>
#include <system_error>
#include <vector>

namespace {

constexpr std::array<char, 16> kBundleMagic = {
    'O', 'M', 'N', 'I', 'S', 'P', 'A', 'T', 'B', 'U', 'N', 'D', 'L', 'E', '1', '!'};
constexpr wchar_t kPackageName[] = L"Omniphony.SpatialCompanion";
constexpr wchar_t kCertificateDisplayName[] = L"Omniphony Development";

#pragma pack(push, 1)
struct BundleFooter {
    char magic[16];
    std::uint64_t msixSize;
    std::uint64_t certificateSize;
};
#pragma pack(pop)

static_assert(sizeof(BundleFooter) == 32, "Bundle footer layout drifted.");

struct BundleLayout {
    std::uint64_t payloadOffset = 0;
    BundleFooter footer{};
};

std::wstring Win32Message(DWORD error) {
    wchar_t* buffer = nullptr;
    const DWORD length = FormatMessageW(
        FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
        nullptr,
        error,
        0,
        reinterpret_cast<wchar_t*>(&buffer),
        0,
        nullptr);
    std::wstring result = length != 0 && buffer != nullptr ? std::wstring(buffer, length) : L"unknown error";
    if (buffer != nullptr) {
        LocalFree(buffer);
    }
    while (!result.empty() && (result.back() == L'\r' || result.back() == L'\n' || result.back() == L' ')) {
        result.pop_back();
    }
    return result;
}

std::filesystem::path SelfPath() {
    std::vector<wchar_t> buffer(32768);
    const DWORD length = GetModuleFileNameW(nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
    if (length == 0 || length >= buffer.size()) {
        throw std::runtime_error("GetModuleFileNameW failed");
    }
    return std::filesystem::path(std::wstring(buffer.data(), length));
}

bool InspectBundle(const std::filesystem::path& path, BundleLayout& layout) {
    std::ifstream input(path, std::ios::binary | std::ios::ate);
    if (!input) {
        std::wcerr << L"SPATIAL_SETUP_BUNDLE_OPEN_ERROR\t" << path << L"\n";
        return false;
    }

    const auto end = input.tellg();
    if (end < static_cast<std::streamoff>(sizeof(BundleFooter))) {
        std::wcerr << L"SPATIAL_SETUP_BUNDLE_TOO_SMALL\t1\n";
        return false;
    }

    const auto totalSize = static_cast<std::uint64_t>(end);
    input.seekg(static_cast<std::streamoff>(totalSize - sizeof(BundleFooter)), std::ios::beg);
    BundleFooter footer{};
    input.read(reinterpret_cast<char*>(&footer), sizeof(footer));
    if (!input || std::memcmp(footer.magic, kBundleMagic.data(), kBundleMagic.size()) != 0) {
        std::wcerr << L"SPATIAL_SETUP_BUNDLE_MAGIC_OK\t0\n";
        return false;
    }

    if (footer.msixSize == 0 || footer.certificateSize == 0 ||
        footer.msixSize > totalSize || footer.certificateSize > totalSize ||
        footer.msixSize > std::numeric_limits<std::uint64_t>::max() - footer.certificateSize) {
        std::wcerr << L"SPATIAL_SETUP_BUNDLE_LENGTHS_OK\t0\n";
        return false;
    }

    const std::uint64_t payloadSize = footer.msixSize + footer.certificateSize;
    if (payloadSize > totalSize - sizeof(BundleFooter)) {
        std::wcerr << L"SPATIAL_SETUP_BUNDLE_LENGTHS_OK\t0\n";
        return false;
    }

    layout.payloadOffset = totalSize - sizeof(BundleFooter) - payloadSize;
    layout.footer = footer;
    std::wcout << L"SPATIAL_SETUP_BUNDLE_MAGIC_OK\t1\n";
    std::wcout << L"SPATIAL_SETUP_BUNDLE_LENGTHS_OK\t1\n";
    std::wcout << L"SPATIAL_SETUP_EMBEDDED_MSIX_BYTES\t" << footer.msixSize << L"\n";
    std::wcout << L"SPATIAL_SETUP_EMBEDDED_CERT_BYTES\t" << footer.certificateSize << L"\n";
    return true;
}

bool CopyRange(
    std::ifstream& input,
    std::uint64_t offset,
    std::uint64_t size,
    const std::filesystem::path& destination) {
    input.clear();
    input.seekg(static_cast<std::streamoff>(offset), std::ios::beg);
    if (!input) {
        return false;
    }

    std::ofstream output(destination, std::ios::binary | std::ios::trunc);
    if (!output) {
        return false;
    }

    std::array<char, 64 * 1024> buffer{};
    std::uint64_t remaining = size;
    while (remaining > 0) {
        const auto chunk = static_cast<std::streamsize>(std::min<std::uint64_t>(remaining, buffer.size()));
        input.read(buffer.data(), chunk);
        if (input.gcount() != chunk) {
            return false;
        }
        output.write(buffer.data(), chunk);
        if (!output) {
            return false;
        }
        remaining -= static_cast<std::uint64_t>(chunk);
    }
    return true;
}

bool ExtractBundle(
    const std::filesystem::path& self,
    const BundleLayout& layout,
    const std::filesystem::path& directory,
    std::filesystem::path& msix,
    std::filesystem::path& certificate) {
    std::error_code ec;
    std::filesystem::create_directories(directory, ec);
    if (ec) {
        std::wcerr << L"SPATIAL_SETUP_EXTRACT_DIRECTORY_ERROR\t" << directory << L"\n";
        return false;
    }

    msix = directory / L"OmniphonySpatialCompanion.msix";
    certificate = directory / L"OmniphonySpatialCompanion.cer";

    std::ifstream input(self, std::ios::binary);
    if (!input) {
        return false;
    }

    if (!CopyRange(input, layout.payloadOffset, layout.footer.msixSize, msix)) {
        std::wcerr << L"SPATIAL_SETUP_EXTRACT_MSIX_OK\t0\n";
        return false;
    }
    if (!CopyRange(
            input,
            layout.payloadOffset + layout.footer.msixSize,
            layout.footer.certificateSize,
            certificate)) {
        std::wcerr << L"SPATIAL_SETUP_EXTRACT_CERT_OK\t0\n";
        return false;
    }

    std::wcout << L"SPATIAL_SETUP_EXTRACT_MSIX_OK\t1\n";
    std::wcout << L"SPATIAL_SETUP_EXTRACT_CERT_OK\t1\n";
    return true;
}

std::vector<std::uint8_t> ReadAllBytes(const std::filesystem::path& path) {
    std::ifstream input(path, std::ios::binary | std::ios::ate);
    if (!input) {
        return {};
    }
    const auto end = input.tellg();
    if (end <= 0 || static_cast<std::uint64_t>(end) > std::numeric_limits<DWORD>::max()) {
        return {};
    }
    std::vector<std::uint8_t> bytes(static_cast<std::size_t>(end));
    input.seekg(0, std::ios::beg);
    input.read(reinterpret_cast<char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
    if (!input) {
        return {};
    }
    return bytes;
}

bool TrustDevelopmentCertificate(const std::filesystem::path& certificatePath) {
    const auto bytes = ReadAllBytes(certificatePath);
    if (bytes.empty()) {
        std::wcerr << L"SPATIAL_SETUP_CERT_READ_OK\t0\n";
        return false;
    }

    PCCERT_CONTEXT certificate = CertCreateCertificateContext(
        X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
        bytes.data(),
        static_cast<DWORD>(bytes.size()));
    if (certificate == nullptr) {
        std::wcerr << L"SPATIAL_SETUP_CERT_PARSE_OK\t0\t" << Win32Message(GetLastError()) << L"\n";
        return false;
    }

    wchar_t displayName[256]{};
    const DWORD nameLength = CertGetNameStringW(
        certificate,
        CERT_NAME_SIMPLE_DISPLAY_TYPE,
        0,
        nullptr,
        displayName,
        static_cast<DWORD>(std::size(displayName)));
    if (nameLength <= 1 || std::wstring(displayName) != kCertificateDisplayName) {
        std::wcerr << L"SPATIAL_SETUP_CERT_SUBJECT_OK\t0\n";
        CertFreeCertificateContext(certificate);
        return false;
    }
    std::wcout << L"SPATIAL_SETUP_CERT_SUBJECT_OK\t1\n";

    HCERTSTORE store = CertOpenStore(
        CERT_STORE_PROV_SYSTEM_W,
        0,
        0,
        CERT_SYSTEM_STORE_LOCAL_MACHINE | CERT_STORE_OPEN_EXISTING_FLAG,
        L"TrustedPeople");
    if (store == nullptr) {
        std::wcerr << L"SPATIAL_SETUP_TRUST_STORE_OPEN_OK\t0\t" << Win32Message(GetLastError()) << L"\n";
        CertFreeCertificateContext(certificate);
        return false;
    }

    const BOOL added = CertAddCertificateContextToStore(
        store,
        certificate,
        CERT_STORE_ADD_REPLACE_EXISTING,
        nullptr);
    if (!added) {
        std::wcerr << L"SPATIAL_SETUP_CERT_TRUSTED\t0\t" << Win32Message(GetLastError()) << L"\n";
    }

    CertCloseStore(store, 0);
    CertFreeCertificateContext(certificate);
    if (!added) {
        return false;
    }

    std::wcout << L"SPATIAL_SETUP_CERT_TRUSTED\t1\n";
    return true;
}

std::wstring FileUri(const std::filesystem::path& path) {
    std::vector<wchar_t> buffer(32768);
    DWORD length = static_cast<DWORD>(buffer.size());
    const HRESULT hr = UrlCreateFromPathW(path.c_str(), buffer.data(), &length, 0);
    if (FAILED(hr)) {
        throw winrt::hresult_error(hr, L"UrlCreateFromPathW failed");
    }
    return std::wstring(buffer.data());
}

bool DeploymentSucceeded(
    const winrt::Windows::Management::Deployment::DeploymentResult& result,
    const wchar_t* marker) {
    const HRESULT extended = static_cast<HRESULT>(result.ExtendedErrorCode());
    if (FAILED(extended)) {
        std::wcerr << marker << L"\t0\t0x" << std::hex << static_cast<std::uint32_t>(extended)
                   << std::dec << L"\t" << result.ErrorText().c_str() << L"\n";
        return false;
    }
    std::wcout << marker << L"\t1\n";
    return true;
}

bool InstallPackage(const std::filesystem::path& msix) {
    using namespace winrt::Windows::Foundation;
    using namespace winrt::Windows::Management::Deployment;

    PackageManager manager;
    bool removedAny = false;
    for (const auto& package : manager.FindPackagesForUser(L"")) {
        if (std::wstring(package.Id().Name().c_str()) != kPackageName) {
            continue;
        }
        removedAny = true;
        const auto removeResult = manager.RemovePackageAsync(package.Id().FullName()).get();
        if (!DeploymentSucceeded(removeResult, L"SPATIAL_SETUP_PREVIOUS_PACKAGE_REMOVED")) {
            return false;
        }
    }
    if (!removedAny) {
        std::wcout << L"SPATIAL_SETUP_PREVIOUS_PACKAGE_PRESENT\t0\n";
    }

    AddPackageOptions options;
    options.ForceAppShutdown(true);
    options.ForceUpdateFromAnyVersion(true);
    const auto result = manager.AddPackageByUriAsync(Uri(FileUri(msix)), options).get();
    return DeploymentSucceeded(result, L"SPATIAL_SETUP_PACKAGE_INSTALLED");
}

std::wstring InstalledPackageFamilyName() {
    using namespace winrt::Windows::Management::Deployment;

    PackageManager manager;
    for (const auto& package : manager.FindPackagesForUser(L"")) {
        if (std::wstring(package.Id().Name().c_str()) == kPackageName) {
            return std::wstring(package.Id().FamilyName().c_str());
        }
    }
    throw std::runtime_error("Installed Omniphony spatial companion package family was not found.");
}

int RegisterMediaExtensionFromBootstrap() {
    using RegisterMediaExtensionPackageFn = HRESULT(WINAPI*)(PCWSTR);

    const auto familyName = InstalledPackageFamilyName();
    HMODULE module = LoadLibraryW(L"CompPkgSup.dll");
    if (module == nullptr) {
        const DWORD error = GetLastError();
        std::wcout << L"SPATIAL_SETUP_MEDIA_EXTENSION_REGISTER_API_AVAILABLE\t0\n";
        std::wcerr << L"SPATIAL_SETUP_MEDIA_EXTENSION_REGISTER_ERROR\t" << error
                   << L"\t" << Win32Message(error) << L"\n";
        return 23;
    }

    const auto registerMediaExtensionPackage = reinterpret_cast<RegisterMediaExtensionPackageFn>(
        GetProcAddress(module, "RegisterMediaExtensionPackage"));
    if (registerMediaExtensionPackage == nullptr) {
        std::wcout << L"SPATIAL_SETUP_MEDIA_EXTENSION_REGISTER_API_AVAILABLE\t0\n";
        std::wcout << L"SPATIAL_SETUP_MEDIA_EXTENSION_REGISTER_REQUIRES_WINDOWS_11_24H2\t1\n";
        FreeLibrary(module);
        return 23;
    }

    std::wcout << L"SPATIAL_SETUP_MEDIA_EXTENSION_REGISTER_API_AVAILABLE\t1\n";
    std::wcout << L"SPATIAL_SETUP_MEDIA_EXTENSION_PACKAGE_FAMILY\t" << familyName << L"\n";
    const HRESULT result = registerMediaExtensionPackage(familyName.c_str());
    FreeLibrary(module);

    std::wcout << L"SPATIAL_SETUP_MEDIA_EXTENSION_REGISTER_HRESULT\t0x"
               << std::hex << std::uppercase << static_cast<unsigned long>(result)
               << std::dec << L"\n";
    if (FAILED(result)) {
        std::wcout << L"SPATIAL_SETUP_MEDIA_EXTENSION_REGISTERED\t0\n";
        return 23;
    }

    std::wcout << L"SPATIAL_SETUP_MEDIA_EXTENSION_REGISTERED\t1\n";
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

    std::wcout << L"SPATIAL_SETUP_DEFAULT_RENDER_ROLE\teMultimedia\n";
    std::wcout << L"SPATIAL_SETUP_DEFAULT_RENDER_ENDPOINT_DISCOVERED\t1\n";
    std::wcout << L"SPATIAL_SETUP_DEFAULT_RENDER_ENDPOINT_ID\t" << endpointId << L"\n";
    return endpointId;
}

int LaunchPackagedCommand(const std::wstring& arguments, const wchar_t* exitMarker) {
    wchar_t localAppData[32768]{};
    const DWORD length = GetEnvironmentVariableW(
        L"LOCALAPPDATA",
        localAppData,
        static_cast<DWORD>(std::size(localAppData)));
    if (length == 0 || length >= std::size(localAppData)) {
        std::wcerr << L"SPATIAL_SETUP_WINDOWS_APPS_PATH_OK\t0\n";
        return 31;
    }

    const auto alias = std::filesystem::path(localAppData) /
        L"Microsoft" / L"WindowsApps" / L"OmniphonySpatialCompanion.exe";
    std::wstring commandLine = L"\"" + alias.wstring() + L"\" " + arguments;

    for (int attempt = 0; attempt != 20; ++attempt) {
        STARTUPINFOW startup{};
        startup.cb = sizeof(startup);
        PROCESS_INFORMATION process{};
        std::vector<wchar_t> mutableCommand(commandLine.begin(), commandLine.end());
        mutableCommand.push_back(L'\0');

        if (CreateProcessW(
                alias.c_str(),
                mutableCommand.data(),
                nullptr,
                nullptr,
                TRUE,
                0,
                nullptr,
                nullptr,
                &startup,
                &process)) {
            std::wcout << L"SPATIAL_SETUP_PACKAGED_ALIAS_LAUNCHED\t1\n";
            WaitForSingleObject(process.hProcess, INFINITE);
            DWORD exitCode = 1;
            GetExitCodeProcess(process.hProcess, &exitCode);
            CloseHandle(process.hThread);
            CloseHandle(process.hProcess);
            std::wcout << exitMarker << L"\t" << exitCode << L"\n";
            return static_cast<int>(exitCode);
        }

        const DWORD error = GetLastError();
        if (error != ERROR_FILE_NOT_FOUND && error != ERROR_PATH_NOT_FOUND) {
            std::wcerr << L"SPATIAL_SETUP_PACKAGED_ALIAS_LAUNCHED\t0\t"
                       << error << L"\t" << Win32Message(error) << L"\n";
            return 32;
        }
        Sleep(250);
    }

    std::wcerr << L"SPATIAL_SETUP_PACKAGED_ALIAS_LAUNCHED\t0\talias not available after install\n";
    return 33;
}

std::filesystem::path TemporaryDirectory() {
    wchar_t buffer[32768]{};
    const DWORD length = GetTempPathW(static_cast<DWORD>(std::size(buffer)), buffer);
    if (length == 0 || length >= std::size(buffer)) {
        throw std::runtime_error("GetTempPathW failed");
    }
    return std::filesystem::path(buffer) /
        (L"OmniphonySpatialSetup-" + std::to_wstring(GetCurrentProcessId()));
}

void MaybePauseForExplorerLaunch() {
    DWORD processIds[4]{};
    const DWORD count = GetConsoleProcessList(processIds, static_cast<DWORD>(std::size(processIds)));
    if (count <= 1) {
        std::wcout << L"\nPress Enter to close..." << std::flush;
        std::wstring ignored;
        std::getline(std::wcin, ignored);
    }
}

int InstallAndVerify(const std::filesystem::path& self, const BundleLayout& layout) {
    const auto temp = TemporaryDirectory();
    std::error_code ec;
    std::filesystem::remove_all(temp, ec);

    std::filesystem::path msix;
    std::filesystem::path certificate;
    if (!ExtractBundle(self, layout, temp, msix, certificate)) {
        return 20;
    }

    std::wcout << L"SPATIAL_SETUP_ACTION\tTrust embedded Omniphony development certificate in LocalMachine\\TrustedPeople\n";
    if (!TrustDevelopmentCertificate(certificate)) {
        std::filesystem::remove_all(temp, ec);
        return 21;
    }

    std::wcout << L"SPATIAL_SETUP_ACTION\tInstall embedded signed MSIX for current user\n";
    if (!InstallPackage(msix)) {
        std::filesystem::remove_all(temp, ec);
        return 22;
    }

    std::wcout << L"SPATIAL_SETUP_ACTION\tRegister installed media extension from elevated full-trust bootstrap\n";
    if (RegisterMediaExtensionFromBootstrap() != 0) {
        std::filesystem::remove_all(temp, ec);
        return 23;
    }

    const auto endpointId = DefaultRenderEndpointId();
    std::wcout << L"SPATIAL_SETUP_READY\t1\n";

    std::wcout << L"SPATIAL_SETUP_ACTION\tNotify spatial license/configuration change from package identity\n";
    const int notifyExit = LaunchPackagedCommand(L"notify", L"SPATIAL_SETUP_NOTIFY_EXIT_CODE");
    if (notifyExit != 0) {
        std::filesystem::remove_all(temp, ec);
        return notifyExit;
    }

    std::wcout << L"SPATIAL_SETUP_ACTION\tSelect Omniphony on the default multimedia render endpoint from package identity\n";
    const std::wstring selectArguments = L"select \"" + endpointId + L"\"";
    const int selectExit = LaunchPackagedCommand(selectArguments, L"SPATIAL_SETUP_SELECT_EXIT_CODE");
    if (selectExit == 0) {
        std::wcout << L"SPATIAL_SETUP_VERIFY_DEFAULT_OK\t1\n";
    } else {
        std::wcout << L"SPATIAL_SETUP_VERIFY_DEFAULT_OK\t0\n";
    }

    std::filesystem::remove_all(temp, ec);
    return selectExit;
}

}  // namespace

int wmain(int argc, wchar_t** argv) {
    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        const auto self = SelfPath();
        BundleLayout layout{};
        if (!InspectBundle(self, layout)) {
            MaybePauseForExplorerLaunch();
            return 10;
        }

        if (argc >= 2 && std::wstring(argv[1]) == L"--verify-bundle") {
            std::wcout << L"SPATIAL_SETUP_SINGLE_EXE_BUNDLE_OK\t1\n";
            return 0;
        }

        if (argc >= 3 && std::wstring(argv[1]) == L"--extract") {
            std::filesystem::path msix;
            std::filesystem::path certificate;
            const bool ok = ExtractBundle(self, layout, std::filesystem::path(argv[2]), msix, certificate);
            if (ok) {
                std::wcout << L"SPATIAL_SETUP_SINGLE_EXE_EXTRACT_OK\t1\n";
                return 0;
            }
            return 11;
        }

        const int result = InstallAndVerify(self, layout);
        MaybePauseForExplorerLaunch();
        return result;
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"SPATIAL_SETUP_HRESULT\t0x" << std::hex
                   << static_cast<std::uint32_t>(error.code().value) << std::dec
                   << L"\t" << error.message().c_str() << L"\n";
    } catch (const std::exception& error) {
        std::cerr << "SPATIAL_SETUP_EXCEPTION\t" << error.what() << "\n";
    }
    MaybePauseForExplorerLaunch();
    return 99;
}
