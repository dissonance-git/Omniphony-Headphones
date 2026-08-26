#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <winstring.h>

#include <winrt/base.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>

#include <algorithm>
#include <cstdint>
#include <iostream>
#include <string>

namespace {

using winrt::Windows::Foundation::IInspectable;
using winrt::Windows::Foundation::IPropertyValue;
using winrt::Windows::Foundation::PropertyType;
using winrt::Windows::Foundation::Collections::IPropertySet;
using winrt::Windows::Foundation::Collections::IVector;

constexpr wchar_t kPackageFamilyName[] = L"Omniphony.SpatialCompanion_1nv7pqmcjcq0w";
constexpr wchar_t kFormatGuid[] = L"{4BD75423-A66C-4586-B782-1FCBBDF2AE74}";
constexpr wchar_t kManifestCategory[] = L"windows.mediaPlayback";
constexpr wchar_t kSpatialSubtypeCategory[] = L"Windows.Media.Audio.SpatialAudioFormatSubtype";

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

bool ContainsInsensitive(std::wstring text, std::wstring needle) {
    std::transform(text.begin(), text.end(), text.begin(), towlower);
    std::transform(needle.begin(), needle.end(), needle.begin(), towlower);
    return text.find(needle) != std::wstring::npos;
}

void DumpInspectable(const std::wstring& category, std::uint32_t index,
                     const std::wstring& path, const IInspectable& value, int depth,
                     bool& formatGuidSeen) {
    if (!value || depth > 8) return;

    if (const auto property = value.try_as<IPropertyValue>()) {
        const std::wstring text = ScalarText(value);
        std::wcout << L"DISCOVERY_V23_PROPERTY\tCATEGORY=" << category
                   << L"\tINDEX=" << index << L"\tPATH=" << path
                   << L"\tVALUE=" << text << L'\n';
        if (ContainsInsensitive(text, kFormatGuid)) formatGuidSeen = true;
        return;
    }

    if (const auto set = value.try_as<IPropertySet>()) {
        for (const auto& pair : set) {
            const std::wstring child = path.empty()
                ? std::wstring(pair.Key().c_str())
                : path + L"." + pair.Key().c_str();
            DumpInspectable(category, index, child, pair.Value(), depth + 1, formatGuidSeen);
        }
        return;
    }

    if (const auto vector = value.try_as<IVector<IPropertySet>>()) {
        for (std::uint32_t i = 0; i < vector.Size(); ++i) {
            DumpInspectable(category, index, path + L"[" + std::to_wstring(i) + L"]",
                            vector.GetAt(i), depth + 1, formatGuidSeen);
        }
        return;
    }

    if (const auto vector = value.try_as<IVector<IInspectable>>()) {
        for (std::uint32_t i = 0; i < vector.Size(); ++i) {
            DumpInspectable(category, index, path + L"[" + std::to_wstring(i) + L"]",
                            vector.GetAt(i), depth + 1, formatGuidSeen);
        }
    }
}

struct QueryResult {
    bool apiSucceeded = false;
    bool omniphonyFound = false;
    bool formatGuidSeen = false;
    std::uint32_t count = 0;
};

QueryResult Query(const wchar_t* categoryText, bool trustedOnly) {
    using GetMediaComponentPackageInfoFn = HRESULT(WINAPI*)(bool, HSTRING, void**);
    QueryResult result{};

    HMODULE module = LoadLibraryW(L"CompPkgSup.dll");
    if (module == nullptr) {
        std::wcout << L"DISCOVERY_V23_COMPPKGSUP_AVAILABLE\t0\n";
        return result;
    }
    std::wcout << L"DISCOVERY_V23_COMPPKGSUP_AVAILABLE\t1\n";

    const auto getInfo = reinterpret_cast<GetMediaComponentPackageInfoFn>(
        GetProcAddress(module, "GetMediaComponentPackageInfo"));
    if (getInfo == nullptr) {
        std::wcout << L"DISCOVERY_V23_QUERY_API_AVAILABLE\t0\n";
        FreeLibrary(module);
        return result;
    }
    std::wcout << L"DISCOVERY_V23_QUERY_API_AVAILABLE\t1\n";

    winrt::hstring category{categoryText};
    void* rawVector = nullptr;
    const HRESULT hr = getInfo(trustedOnly, reinterpret_cast<HSTRING>(winrt::get_abi(category)), &rawVector);
    FreeLibrary(module);

    std::wcout << L"DISCOVERY_V23_QUERY\tCATEGORY=" << categoryText
               << L"\tTRUSTED_ONLY=" << (trustedOnly ? 1 : 0)
               << L"\tHRESULT=0x" << std::hex << std::uppercase
               << static_cast<std::uint32_t>(hr) << std::dec << L'\n';
    if (FAILED(hr) || rawVector == nullptr) {
        std::wcout << L"DISCOVERY_V23_QUERY_COUNT\tCATEGORY=" << categoryText
                   << L"\tTRUSTED_ONLY=" << (trustedOnly ? 1 : 0) << L"\tCOUNT=0\n";
        return result;
    }

    result.apiSucceeded = true;
    IVector<IPropertySet> packages{rawVector, winrt::take_ownership_from_abi};
    result.count = packages.Size();
    std::wcout << L"DISCOVERY_V23_QUERY_COUNT\tCATEGORY=" << categoryText
               << L"\tTRUSTED_ONLY=" << (trustedOnly ? 1 : 0)
               << L"\tCOUNT=" << result.count << L'\n';

    for (std::uint32_t i = 0; i < packages.Size(); ++i) {
        const auto set = packages.GetAt(i);
        const std::wstring pfn = FindScalar(set, L"@PackageFamilyName");
        const std::wstring name = FindScalar(set, L"@Name");
        const std::wstring subtype = FindScalar(set, L"@SubType");
        const std::wstring appService = FindScalar(set, L"@AppServiceName");
        const std::wstring requiresReg = FindScalar(set, L"@PackageRequiresRegistration");

        std::wcout << L"DISCOVERY_V23_ENTRY\tCATEGORY=" << categoryText
                   << L"\tTRUSTED_ONLY=" << (trustedOnly ? 1 : 0)
                   << L"\tINDEX=" << i
                   << L"\tPFN=" << (pfn.empty() ? L"<missing>" : pfn)
                   << L"\tNAME=" << (name.empty() ? L"<missing>" : name)
                   << L"\tSUBTYPE=" << (subtype.empty() ? L"<missing>" : subtype)
                   << L"\tAPPSERVICE=" << (appService.empty() ? L"<missing>" : appService)
                   << L"\tREQUIRES_REG=" << (requiresReg.empty() ? L"<missing>" : requiresReg)
                   << L'\n';

        bool entryGuidSeen = false;
        for (const auto& pair : set) {
            DumpInspectable(categoryText, i, std::wstring(pair.Key().c_str()), pair.Value(), 0, entryGuidSeen);
        }
        result.formatGuidSeen = result.formatGuidSeen || entryGuidSeen;
        if (_wcsicmp(pfn.c_str(), kPackageFamilyName) == 0) {
            result.omniphonyFound = true;
            std::wcout << L"DISCOVERY_V23_OMNIPHONY_ENTRY\tCATEGORY=" << categoryText
                       << L"\tTRUSTED_ONLY=" << (trustedOnly ? 1 : 0)
                       << L"\tINDEX=" << i
                       << L"\tFORMAT_GUID_SEEN=" << (entryGuidSeen ? 1 : 0) << L'\n';
        }
    }

    std::wcout << L"DISCOVERY_V23_OMNIPHONY_FOUND\tCATEGORY=" << categoryText
               << L"\tTRUSTED_ONLY=" << (trustedOnly ? 1 : 0)
               << L"\tVALUE=" << (result.omniphonyFound ? 1 : 0) << L'\n';
    std::wcout << L"DISCOVERY_V23_FORMAT_GUID_SEEN\tCATEGORY=" << categoryText
               << L"\tTRUSTED_ONLY=" << (trustedOnly ? 1 : 0)
               << L"\tVALUE=" << (result.formatGuidSeen ? 1 : 0) << L'\n';
    return result;
}

} // namespace

int wmain() {
    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        std::wcout << L"DISCOVERY_V23_PROBE_BEGIN\t1\n";
        std::wcout << L"DISCOVERY_V23_EXPECTED_PFN\t" << kPackageFamilyName << L'\n';
        std::wcout << L"DISCOVERY_V23_EXPECTED_FORMAT_GUID\t" << kFormatGuid << L'\n';

        const auto mediaUntrusted = Query(kManifestCategory, false);
        const auto mediaTrusted = Query(kManifestCategory, true);
        const auto subtypeUntrusted = Query(kSpatialSubtypeCategory, false);
        const auto subtypeTrusted = Query(kSpatialSubtypeCategory, true);

        std::wcout << L"DISCOVERY_V23_MEDIAPLAYBACK_UNTRUSTED_OMNIPHONY_FOUND\t"
                   << (mediaUntrusted.omniphonyFound ? 1 : 0) << L'\n';
        std::wcout << L"DISCOVERY_V23_MEDIAPLAYBACK_TRUSTED_OMNIPHONY_FOUND\t"
                   << (mediaTrusted.omniphonyFound ? 1 : 0) << L'\n';
        std::wcout << L"DISCOVERY_V23_SPATIAL_SUBTYPE_UNTRUSTED_OMNIPHONY_FOUND\t"
                   << (subtypeUntrusted.omniphonyFound ? 1 : 0) << L'\n';
        std::wcout << L"DISCOVERY_V23_SPATIAL_SUBTYPE_TRUSTED_OMNIPHONY_FOUND\t"
                   << (subtypeTrusted.omniphonyFound ? 1 : 0) << L'\n';
        std::wcout << L"DISCOVERY_V23_READ_ONLY\t1\n";
        std::wcout << L"DISCOVERY_V23_PROBE_END\t1\n";

        return (mediaUntrusted.apiSucceeded && subtypeUntrusted.apiSucceeded) ? 0 : 2;
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"DISCOVERY_V23_HRESULT\t0x" << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(error.code().value) << std::dec
                   << L"\t" << error.message().c_str() << L'\n';
    } catch (const std::exception& error) {
        std::cerr << "DISCOVERY_V23_EXCEPTION\t" << error.what() << '\n';
    }
    std::wcout << L"DISCOVERY_V23_PROBE_END\t1\n";
    return 99;
}
