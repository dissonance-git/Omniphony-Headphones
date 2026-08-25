#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <winstring.h>

#include <winrt/base.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>

#include <algorithm>
#include <cstdint>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

namespace {

using winrt::Windows::Foundation::IInspectable;
using winrt::Windows::Foundation::IPropertyValue;
using winrt::Windows::Foundation::PropertyType;
using winrt::Windows::Foundation::Collections::IPropertySet;
using winrt::Windows::Foundation::Collections::IVector;

constexpr wchar_t kPackageFamilyName[] = L"Omniphony.SpatialCompanion_1nv7pqmcjcq0w";
constexpr wchar_t kFormatGuid[] = L"{4BD75423-A66C-4586-B782-1FCBBDF2AE74}";

std::wstring ScalarText(const IInspectable& value) {
    if (!value) return L"<null>";
    const auto property = value.try_as<IPropertyValue>();
    if (!property) return L"<" + std::wstring(winrt::get_class_name(value).c_str()) + L">";

    switch (property.Type()) {
    case PropertyType::String: return std::wstring(property.GetString().c_str());
    case PropertyType::Guid: return std::wstring(winrt::to_hstring(property.GetGuid()).c_str());
    case PropertyType::Boolean: return property.GetBoolean() ? L"true" : L"false";
    case PropertyType::Int16: return std::to_wstring(property.GetInt16());
    case PropertyType::UInt16: return std::to_wstring(property.GetUInt16());
    case PropertyType::Int32: return std::to_wstring(property.GetInt32());
    case PropertyType::UInt32: return std::to_wstring(property.GetUInt32());
    case PropertyType::Int64: return std::to_wstring(property.GetInt64());
    case PropertyType::UInt64: return std::to_wstring(property.GetUInt64());
    case PropertyType::Single: return std::to_wstring(property.GetSingle());
    case PropertyType::Double: return std::to_wstring(property.GetDouble());
    default: return L"<property-type-" + std::to_wstring(static_cast<int>(property.Type())) + L">";
    }
}

std::wstring FindScalar(const IPropertySet& set, const wchar_t* key) {
    for (const auto& pair : set) {
        if (_wcsicmp(pair.Key().c_str(), key) == 0) return ScalarText(pair.Value());
    }
    return {};
}

bool ContainsInsensitive(const std::wstring& text, const wchar_t* needle) {
    std::wstring lhs = text;
    std::wstring rhs = needle;
    std::transform(lhs.begin(), lhs.end(), lhs.begin(), towlower);
    std::transform(rhs.begin(), rhs.end(), rhs.begin(), towlower);
    return lhs.find(rhs) != std::wstring::npos;
}

struct InterestingState {
    bool appServiceName = false;
    bool packageRequiresRegistration = false;
    bool subType = false;
    bool name = false;
    bool inProcess = false;
    bool formatGuidSeen = false;
};

void DumpInspectable(const std::wstring& path, const IInspectable& value, int depth, InterestingState& state) {
    if (!value || depth > 8) return;

    if (const auto property = value.try_as<IPropertyValue>()) {
        const std::wstring text = ScalarText(value);
        std::wcout << L"MEDIA_V16_PROPERTY\t" << path << L"\t" << text << L'\n';
        if (ContainsInsensitive(path, L"@AppServiceName")) state.appServiceName = true;
        if (ContainsInsensitive(path, L"@PackageRequiresRegistration")) state.packageRequiresRegistration = true;
        if (ContainsInsensitive(path, L"@SubType")) state.subType = true;
        if (ContainsInsensitive(path, L"@Name")) state.name = true;
        if (ContainsInsensitive(path, L"inProcessMediaExtension")) state.inProcess = true;
        if (ContainsInsensitive(text, kFormatGuid)) state.formatGuidSeen = true;
        return;
    }

    if (const auto set = value.try_as<IPropertySet>()) {
        std::wcout << L"MEDIA_V16_PROPERTYSET\t" << path << L"\t" << set.Size() << L'\n';
        for (const auto& pair : set) {
            const std::wstring child = path.empty()
                ? std::wstring(pair.Key().c_str())
                : path + L"." + pair.Key().c_str();
            DumpInspectable(child, pair.Value(), depth + 1, state);
        }
        return;
    }

    if (const auto vector = value.try_as<IVector<IPropertySet>>()) {
        std::wcout << L"MEDIA_V16_VECTOR_PROPERTYSET\t" << path << L"\t" << vector.Size() << L'\n';
        for (std::uint32_t i = 0; i < vector.Size(); ++i) {
            DumpInspectable(path + L"[" + std::to_wstring(i) + L"]", vector.GetAt(i), depth + 1, state);
        }
        return;
    }

    if (const auto vector = value.try_as<IVector<IInspectable>>()) {
        std::wcout << L"MEDIA_V16_VECTOR_INSPECTABLE\t" << path << L"\t" << vector.Size() << L'\n';
        for (std::uint32_t i = 0; i < vector.Size(); ++i) {
            DumpInspectable(path + L"[" + std::to_wstring(i) + L"]", vector.GetAt(i), depth + 1, state);
        }
        return;
    }

    std::wcout << L"MEDIA_V16_OPAQUE\t" << path << L"\t"
               << winrt::get_class_name(value).c_str() << L'\n';
}

void DumpInstalledManifest(const std::wstring& installLocation) {
    if (installLocation.empty()) {
        std::wcout << L"MEDIA_V16_MANIFEST_AVAILABLE\t0\n";
        return;
    }

    std::wstring path = installLocation;
    if (!path.empty() && path.back() != L'\\') path.push_back(L'\\');
    path += L"AppxManifest.xml";

    std::ifstream input(path);
    if (!input) {
        std::wcout << L"MEDIA_V16_MANIFEST_AVAILABLE\t0\n";
        return;
    }

    std::wcout << L"MEDIA_V16_MANIFEST_AVAILABLE\t1\n";
    std::string line;
    while (std::getline(input, line)) {
        std::string lower = line;
        std::transform(lower.begin(), lower.end(), lower.begin(), [](unsigned char c) {
            return static_cast<char>(std::tolower(c));
        });
        if (lower.find("mediaplayback") != std::string::npos ||
            lower.find("<uap:codec") != std::string::npos ||
            lower.find("appservice") != std::string::npos ||
            lower.find("entrypoint") != std::string::npos) {
            std::cout << "MEDIA_V16_MANIFEST_LINE\t" << line << '\n';
        }
    }
}

bool Query(bool trustedOnly, bool dumpOmniphony) {
    using GetMediaComponentPackageInfoFn = HRESULT(WINAPI*)(bool, HSTRING, void**);

    HMODULE module = LoadLibraryW(L"CompPkgSup.dll");
    if (module == nullptr) {
        std::wcout << L"MEDIA_V16_COMPPKGSUP_AVAILABLE\t0\n";
        return false;
    }
    std::wcout << L"MEDIA_V16_COMPPKGSUP_AVAILABLE\t1\n";

    const auto getInfo = reinterpret_cast<GetMediaComponentPackageInfoFn>(
        GetProcAddress(module, "GetMediaComponentPackageInfo"));
    if (getInfo == nullptr) {
        std::wcout << L"MEDIA_V16_QUERY_API_AVAILABLE\t0\n";
        FreeLibrary(module);
        return false;
    }
    std::wcout << L"MEDIA_V16_QUERY_API_AVAILABLE\t1\n";

    winrt::hstring category{L"windows.mediaPlayback"};
    void* rawVector = nullptr;
    const HRESULT hr = getInfo(trustedOnly, reinterpret_cast<HSTRING>(winrt::get_abi(category)), &rawVector);
    FreeLibrary(module);

    std::wcout << L"MEDIA_V16_QUERY_TRUSTED_ONLY\t" << (trustedOnly ? 1 : 0) << L'\n';
    std::wcout << L"MEDIA_V16_QUERY_HRESULT\t0x" << std::hex << std::uppercase
               << static_cast<std::uint32_t>(hr) << std::dec << L'\n';
    if (FAILED(hr) || rawVector == nullptr) {
        std::wcout << L"MEDIA_V16_QUERY_COUNT\t0\n";
        return false;
    }

    IVector<IPropertySet> packages{rawVector, winrt::take_ownership_from_abi};
    std::wcout << L"MEDIA_V16_QUERY_COUNT\t" << packages.Size() << L'\n';

    bool found = false;
    for (std::uint32_t i = 0; i < packages.Size(); ++i) {
        const auto set = packages.GetAt(i);
        const std::wstring pfn = FindScalar(set, L"@PackageFamilyName");
        const std::wstring name = FindScalar(set, L"@Name");
        const std::wstring subtype = FindScalar(set, L"@SubType");
        const std::wstring appService = FindScalar(set, L"@AppServiceName");
        const std::wstring requiresReg = FindScalar(set, L"@PackageRequiresRegistration");
        const std::wstring inProcess = FindScalar(set, L"inProcessMediaExtension");

        std::wcout << L"MEDIA_V16_ENTRY\tINDEX=" << i
                   << L"\tPFN=" << (pfn.empty() ? L"<missing>" : pfn)
                   << L"\tNAME=" << (name.empty() ? L"<missing>" : name)
                   << L"\tSUBTYPE=" << (subtype.empty() ? L"<missing>" : subtype)
                   << L"\tAPPSERVICE=" << (appService.empty() ? L"<missing>" : appService)
                   << L"\tREQUIRES_REG=" << (requiresReg.empty() ? L"<missing>" : requiresReg)
                   << L"\tINPROCESS=" << (inProcess.empty() ? L"<missing>" : inProcess)
                   << L'\n';

        if (_wcsicmp(pfn.c_str(), kPackageFamilyName) != 0) continue;
        found = true;
        std::wcout << L"MEDIA_V16_OMNIPHONY_INDEX\t" << i << L'\n';

        if (dumpOmniphony) {
            InterestingState state{};
            for (const auto& pair : set) {
                DumpInspectable(std::wstring(pair.Key().c_str()), pair.Value(), 0, state);
            }
            std::wcout << L"MEDIA_V16_APP_SERVICE_NAME_PRESENT\t" << (state.appServiceName ? 1 : 0) << L'\n';
            std::wcout << L"MEDIA_V16_PACKAGE_REQUIRES_REGISTRATION_PRESENT\t" << (state.packageRequiresRegistration ? 1 : 0) << L'\n';
            std::wcout << L"MEDIA_V16_SUBTYPE_PRESENT\t" << (state.subType ? 1 : 0) << L'\n';
            std::wcout << L"MEDIA_V16_NAME_PRESENT\t" << (state.name ? 1 : 0) << L'\n';
            std::wcout << L"MEDIA_V16_INPROCESS_PRESENT\t" << (state.inProcess ? 1 : 0) << L'\n';
            std::wcout << L"MEDIA_V16_FORMAT_GUID_SEEN\t" << (state.formatGuidSeen ? 1 : 0) << L'\n';
            DumpInstalledManifest(FindScalar(set, L"@PackageInstallLocation"));
        }
    }

    std::wcout << L"MEDIA_V16_OMNIPHONY_FOUND\t" << (found ? 1 : 0) << L'\n';
    return found;
}

} // namespace

int wmain() {
    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        std::wcout << L"MEDIA_V16_PROBE_BEGIN\t1\n";
        std::wcout << L"MEDIA_V16_CATEGORY\twindows.mediaPlayback\n";
        std::wcout << L"MEDIA_V16_EXPECTED_FORMAT_GUID\t" << kFormatGuid << L'\n';
        const bool untrustedFound = Query(false, true);
        const bool trustedFound = Query(true, false);
        std::wcout << L"MEDIA_V16_FOUND_TRUSTED_FALSE\t" << (untrustedFound ? 1 : 0) << L'\n';
        std::wcout << L"MEDIA_V16_FOUND_TRUSTED_TRUE\t" << (trustedFound ? 1 : 0) << L'\n';
        std::wcout << L"MEDIA_V16_READ_ONLY\t1\n";
        std::wcout << L"MEDIA_V16_PROBE_END\t1\n";
        return untrustedFound ? 0 : 2;
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"MEDIA_V16_HRESULT\t0x" << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(error.code().value) << std::dec
                   << L"\t" << error.message().c_str() << L'\n';
    } catch (const std::exception& error) {
        std::cerr << "MEDIA_V16_EXCEPTION\t" << error.what() << '\n';
    }
    return 99;
}
