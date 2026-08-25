#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <mmdeviceapi.h>

#include <winrt/base.h>
#include <winrt/Windows.ApplicationModel.AppService.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>

#include <cstdint>
#include <iomanip>
#include <iostream>
#include <string>

namespace {

using namespace winrt;
using namespace Windows::ApplicationModel::AppService;
using namespace Windows::Foundation;
using namespace Windows::Foundation::Collections;

constexpr wchar_t kPackageFamilyName[] = L"Omniphony.SpatialCompanion_1nv7pqmcjcq0w";
constexpr wchar_t kAppServiceName[] = L"OmniphonySpatialLicense";
constexpr wchar_t kFormatGuid[] = L"{4BD75423-A66C-4586-B782-1FCBBDF2AE74}";

std::wstring DefaultRenderEndpointId() {
    com_ptr<IMMDeviceEnumerator> enumerator;
    check_hresult(CoCreateInstance(
        __uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
        __uuidof(IMMDeviceEnumerator), enumerator.put_void()));
    com_ptr<IMMDevice> device;
    check_hresult(enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, device.put()));
    LPWSTR raw = nullptr;
    check_hresult(device->GetId(&raw));
    std::wstring id = raw == nullptr ? L"" : raw;
    if (raw != nullptr) CoTaskMemFree(raw);
    return id;
}

std::wstring PropertyValueText(const IInspectable& value) {
    if (!value) return L"<null>";
    const auto property = value.try_as<IPropertyValue>();
    if (!property) return L"<" + std::wstring(get_class_name(value).c_str()) + L">";

    switch (property.Type()) {
    case PropertyType::String: return std::wstring(property.GetString().c_str());
    case PropertyType::Guid: return std::wstring(to_hstring(property.GetGuid()).c_str());
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

void DumpResponse(const wchar_t* command, const AppServiceResponse& response) {
    std::wcout << L"APPSERVICE_V17_RESPONSE_STATUS\t" << command << L"\t"
               << static_cast<int>(response.Status()) << L'\n';
    const auto message = response.Message();
    std::wcout << L"APPSERVICE_V17_RESPONSE_COUNT\t" << command << L"\t" << message.Size() << L'\n';
    for (const auto& pair : message) {
        int type = -1;
        if (const auto property = pair.Value().try_as<IPropertyValue>()) {
            type = static_cast<int>(property.Type());
        }
        std::wcout << L"APPSERVICE_V17_RESPONSE_FIELD\t" << command
                   << L"\t" << pair.Key().c_str()
                   << L"\tTYPE=" << type
                   << L"\tVALUE=" << PropertyValueText(pair.Value()) << L'\n';
    }
}

bool SendCommand(const AppServiceConnection& connection,
                 const wchar_t* command,
                 const std::wstring& endpointId) {
    ValueSet request;
    request.Insert(L"Command", box_value(hstring{command}));
    request.Insert(L"DeviceID", box_value(hstring{endpointId}));
    request.Insert(L"MediaCodecName", box_value(hstring{L"Omniphony"}));
    request.Insert(L"SpatialAudioSubtype", box_value(hstring{kFormatGuid}));

    std::wcout << L"APPSERVICE_V17_SEND_BEGIN\t" << command << L'\n';
    const auto response = connection.SendMessageAsync(request).get();
    DumpResponse(command, response);
    return response.Status() == AppServiceResponseStatus::Success;
}

} // namespace

int wmain() {
    try {
        init_apartment(apartment_type::multi_threaded);
        std::wcout << L"APPSERVICE_V17_PROBE_BEGIN\t1\n";
        std::wcout << L"APPSERVICE_V17_PACKAGE_FAMILY\t" << kPackageFamilyName << L'\n';
        std::wcout << L"APPSERVICE_V17_SERVICE_NAME\t" << kAppServiceName << L'\n';
        std::wcout << L"APPSERVICE_V17_FORMAT_GUID\t" << kFormatGuid << L'\n';

        const auto endpointId = DefaultRenderEndpointId();
        std::wcout << L"APPSERVICE_V17_ENDPOINT_ID\t" << endpointId << L'\n';

        AppServiceConnection connection;
        connection.AppServiceName(kAppServiceName);
        connection.PackageFamilyName(kPackageFamilyName);

        std::wcout << L"APPSERVICE_V17_OPEN_BEGIN\t1\n";
        const auto openStatus = connection.OpenAsync().get();
        std::wcout << L"APPSERVICE_V17_OPEN_STATUS\t" << static_cast<int>(openStatus) << L'\n';
        std::wcout << L"APPSERVICE_V17_OPEN_SUCCESS\t"
                   << (openStatus == AppServiceConnectionStatus::Success ? 1 : 0) << L'\n';

        if (openStatus != AppServiceConnectionStatus::Success) {
            std::wcout << L"APPSERVICE_V17_CALLER_PACKAGE_IDENTITY_NOTE\tUNPACKAGED_PROBE_MAY_BE_REJECTED\n";
            std::wcout << L"APPSERVICE_V17_PROBE_END\t1\n";
            return 2;
        }

        const bool licenseOk = SendCommand(connection, L"GetLicenseInfo", endpointId);
        const bool runtimeOk = SendCommand(connection, L"GetRuntimeParameters", endpointId);
        connection.Close();

        std::wcout << L"APPSERVICE_V17_GET_LICENSE_INFO_OK\t" << (licenseOk ? 1 : 0) << L'\n';
        std::wcout << L"APPSERVICE_V17_GET_RUNTIME_PARAMETERS_OK\t" << (runtimeOk ? 1 : 0) << L'\n';
        std::wcout << L"APPSERVICE_V17_PROBE_END\t1\n";
        return licenseOk && runtimeOk ? 0 : 3;
    } catch (const hresult_error& error) {
        std::wcerr << L"APPSERVICE_V17_HRESULT\t0x" << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(error.code().value) << std::dec
                   << L"\t" << error.message().c_str() << L'\n';
    } catch (const std::exception& error) {
        std::cerr << "APPSERVICE_V17_EXCEPTION\t" << error.what() << '\n';
    }
    std::wcout << L"APPSERVICE_V17_PROBE_END\t1\n";
    return 99;
}
