#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <mmdeviceapi.h>
#include <propsys.h>
#include <functiondiscoverykeys_devpkey.h>

#include <winrt/base.h>
#include <winrt/Windows.Media.Audio.h>

#include <cstdint>
#include <filesystem>
#include <iomanip>
#include <iostream>
#include <string>
#include <vector>

namespace {

using winrt::Windows::Media::Audio::SpatialAudioDeviceConfiguration;

constexpr wchar_t kFormatGuid[] = L"{4BD75423-A66C-4586-B782-1FCBBDF2AE74}";
constexpr wchar_t kAliasName[] = L"OmniphonySpatialCompanion.exe";

struct EndpointInfo {
    std::wstring id;
    std::wstring name;
    bool isDefault = false;
    bool spatialSupported = false;
    bool omniphonySupported = false;
    HRESULT queryResult = S_OK;
};

std::wstring DeviceId(IMMDevice* device) {
    LPWSTR raw = nullptr;
    if (device == nullptr || FAILED(device->GetId(&raw)) || raw == nullptr) {
        if (raw != nullptr) {
            CoTaskMemFree(raw);
        }
        return {};
    }
    std::wstring value(raw);
    CoTaskMemFree(raw);
    return value;
}

std::wstring FriendlyName(IMMDevice* device) {
    if (device == nullptr) {
        return L"<unknown>";
    }

    winrt::com_ptr<IPropertyStore> store;
    if (FAILED(device->OpenPropertyStore(STGM_READ, store.put()))) {
        return L"<unknown>";
    }

    PROPVARIANT value{};
    PropVariantInit(&value);
    const HRESULT hr = store->GetValue(PKEY_Device_FriendlyName, &value);
    std::wstring name = L"<unknown>";
    if (SUCCEEDED(hr) && value.vt == VT_LPWSTR && value.pwszVal != nullptr) {
        name.assign(value.pwszVal);
    }
    PropVariantClear(&value);
    return name;
}

std::wstring DefaultEndpointId(IMMDeviceEnumerator* enumerator) {
    if (enumerator == nullptr) {
        return {};
    }
    winrt::com_ptr<IMMDevice> device;
    if (FAILED(enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, device.put()))) {
        return {};
    }
    return DeviceId(device.get());
}

std::vector<EndpointInfo> EnumerateActiveRenderEndpoints() {
    winrt::com_ptr<IMMDeviceEnumerator> enumerator;
    winrt::check_hresult(CoCreateInstance(
        __uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
        __uuidof(IMMDeviceEnumerator), enumerator.put_void()));

    const auto defaultId = DefaultEndpointId(enumerator.get());
    std::wcout << L"ENDPOINT_PROBE_DEFAULT_ENDPOINT_DISCOVERED\t" << (!defaultId.empty() ? 1 : 0) << L'\n';
    if (!defaultId.empty()) {
        std::wcout << L"ENDPOINT_PROBE_DEFAULT_ENDPOINT_ID\t" << defaultId << L'\n';
    }

    winrt::com_ptr<IMMDeviceCollection> collection;
    winrt::check_hresult(enumerator->EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE, collection.put()));

    UINT count = 0;
    winrt::check_hresult(collection->GetCount(&count));
    std::wcout << L"ENDPOINT_PROBE_ACTIVE_RENDER_COUNT\t" << count << L'\n';

    std::vector<EndpointInfo> endpoints;
    endpoints.reserve(count);
    for (UINT index = 0; index < count; ++index) {
        winrt::com_ptr<IMMDevice> device;
        if (FAILED(collection->Item(index, device.put()))) {
            continue;
        }

        EndpointInfo info;
        info.id = DeviceId(device.get());
        info.name = FriendlyName(device.get());
        info.isDefault = !defaultId.empty() && _wcsicmp(info.id.c_str(), defaultId.c_str()) == 0;

        if (!info.id.empty()) {
            try {
                const auto config = SpatialAudioDeviceConfiguration::GetForDeviceId(winrt::hstring{info.id});
                info.spatialSupported = config.IsSpatialAudioSupported();
                info.omniphonySupported = config.IsSpatialAudioFormatSupported(winrt::hstring{kFormatGuid});
            } catch (const winrt::hresult_error& error) {
                info.queryResult = error.code().value;
            }
        } else {
            info.queryResult = E_FAIL;
        }

        std::wcout << L"ENDPOINT_PROBE_ENDPOINT_BEGIN\t" << index << L'\n';
        std::wcout << L"ENDPOINT_PROBE_ENDPOINT_NAME\t" << info.name << L'\n';
        std::wcout << L"ENDPOINT_PROBE_ENDPOINT_ID\t" << info.id << L'\n';
        std::wcout << L"ENDPOINT_PROBE_ENDPOINT_IS_DEFAULT\t" << (info.isDefault ? 1 : 0) << L'\n';
        std::wcout << L"ENDPOINT_PROBE_ENDPOINT_QUERY_HRESULT\t0x"
                   << std::hex << std::uppercase << static_cast<std::uint32_t>(info.queryResult)
                   << std::dec << L'\n';
        std::wcout << L"ENDPOINT_PROBE_ENDPOINT_SPATIAL_SUPPORTED\t" << (info.spatialSupported ? 1 : 0) << L'\n';
        std::wcout << L"ENDPOINT_PROBE_ENDPOINT_OMNIPHONY_SUPPORTED\t" << (info.omniphonySupported ? 1 : 0) << L'\n';
        std::wcout << L"ENDPOINT_PROBE_ENDPOINT_END\t" << index << L'\n';
        endpoints.push_back(std::move(info));
    }
    return endpoints;
}

std::wstring ExecutionAliasPath() {
    DWORD required = GetEnvironmentVariableW(L"LOCALAPPDATA", nullptr, 0);
    if (required == 0) {
        return {};
    }
    std::wstring local(required, L'\0');
    const DWORD written = GetEnvironmentVariableW(L"LOCALAPPDATA", local.data(), required);
    if (written == 0 || written >= required) {
        return {};
    }
    local.resize(written);
    return local + L"\\Microsoft\\WindowsApps\\" + kAliasName;
}

DWORD RunPackagedCommand(const std::wstring& arguments, const wchar_t* marker) {
    const auto alias = ExecutionAliasPath();
    if (alias.empty()) {
        std::wcout << marker << L"_LAUNCHED\t0\n";
        return ERROR_PATH_NOT_FOUND;
    }

    std::wstring commandLine = L"\"" + alias + L"\" " + arguments;
    std::vector<wchar_t> mutableCommand(commandLine.begin(), commandLine.end());
    mutableCommand.push_back(L'\0');

    STARTUPINFOW startup{};
    startup.cb = sizeof(startup);
    PROCESS_INFORMATION process{};
    const BOOL created = CreateProcessW(
        alias.c_str(), mutableCommand.data(), nullptr, nullptr, TRUE, 0,
        nullptr, nullptr, &startup, &process);
    std::wcout << marker << L"_LAUNCHED\t" << (created ? 1 : 0) << L'\n';
    if (!created) {
        const DWORD error = GetLastError();
        std::wcout << marker << L"_CREATE_ERROR\t" << error << L'\n';
        return error;
    }

    CloseHandle(process.hThread);
    WaitForSingleObject(process.hProcess, 30000);
    DWORD exitCode = STILL_ACTIVE;
    GetExitCodeProcess(process.hProcess, &exitCode);
    CloseHandle(process.hProcess);
    std::wcout << marker << L"_EXIT_CODE\t" << exitCode << L'\n';
    return exitCode;
}

} // namespace

int wmain() {
    try {
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        std::wcout << L"ENDPOINT_PROBE_BEGIN\t1\n";
        std::wcout << L"ENDPOINT_PROBE_FORMAT_GUID\t" << kFormatGuid << L'\n';

        const auto endpoints = EnumerateActiveRenderEndpoints();
        const EndpointInfo* target = nullptr;
        for (const auto& endpoint : endpoints) {
            if (endpoint.isDefault && endpoint.spatialSupported) {
                target = &endpoint;
                break;
            }
        }
        if (target == nullptr) {
            for (const auto& endpoint : endpoints) {
                if (endpoint.spatialSupported) {
                    target = &endpoint;
                    break;
                }
            }
        }

        std::wcout << L"ENDPOINT_PROBE_SPATIAL_CAPABLE_ENDPOINT_FOUND\t" << (target != nullptr ? 1 : 0) << L'\n';
        if (target == nullptr) {
            std::wcout << L"ENDPOINT_PROBE_NO_VALID_SETTER_TARGET\t1\n";
            std::wcout << L"ENDPOINT_PROBE_END\t1\n";
            return 12;
        }

        std::wcout << L"ENDPOINT_PROBE_TARGET_NAME\t" << target->name << L'\n';
        std::wcout << L"ENDPOINT_PROBE_TARGET_ID\t" << target->id << L'\n';
        std::wcout << L"ENDPOINT_PROBE_TARGET_IS_DEFAULT\t" << (target->isDefault ? 1 : 0) << L'\n';
        std::wcout << L"ENDPOINT_PROBE_TARGET_OMNIPHONY_SUPPORTED_BEFORE_SET\t"
                   << (target->omniphonySupported ? 1 : 0) << L'\n';

        const DWORD notifyExit = RunPackagedCommand(L"notify", L"ENDPOINT_PROBE_NOTIFY");
        std::wcout << L"ENDPOINT_PROBE_NOTIFY_OK\t" << (notifyExit == 0 ? 1 : 0) << L'\n';
        if (notifyExit != 0) {
            return static_cast<int>(notifyExit);
        }

        const std::wstring selectArgs = L"select \"" + target->id + L"\"";
        const DWORD selectExit = RunPackagedCommand(selectArgs, L"ENDPOINT_PROBE_SELECT");
        std::wcout << L"ENDPOINT_PROBE_SETTER_ACCEPTED\t" << (selectExit == 0 ? 1 : 0) << L'\n';
        std::wcout << L"ENDPOINT_PROBE_END\t1\n";
        return static_cast<int>(selectExit);
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"ENDPOINT_PROBE_HRESULT\t0x" << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(error.code().value) << std::dec
                   << L"\t" << error.message().c_str() << L'\n';
    } catch (const std::exception& error) {
        std::cerr << "ENDPOINT_PROBE_EXCEPTION\t" << error.what() << '\n';
    }
    return 99;
}
