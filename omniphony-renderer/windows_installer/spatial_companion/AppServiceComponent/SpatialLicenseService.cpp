#include "SpatialLicenseService.h"

#include <ppltasks.h>

using namespace Concurrency;
using namespace Platform;
using namespace Windows::ApplicationModel::AppService;
using namespace Windows::ApplicationModel::Background;
using namespace Windows::Foundation;
using namespace Windows::Foundation::Collections;
using namespace Windows::Storage;
using namespace OmniphonySpatialLicenseService;

namespace {

constexpr long long kDevelopmentExpirationFileTime = 441481536000000000LL; // 3000-01-01 UTC.

String^ ReadText(ValueSet^ values, String^ key) {
    if (values == nullptr || !values->HasKey(key)) {
        return ref new String(L"");
    }
    auto value = values->Lookup(key);
    return value == nullptr ? ref new String(L"") : value->ToString();
}

unsigned int ReadUInt32(IPropertySet^ values, String^ key) {
    if (values == nullptr || !values->HasKey(key)) {
        return 0u;
    }
    auto property = dynamic_cast<IPropertyValue^>(values->Lookup(key));
    if (property == nullptr || property->Type != PropertyType::UInt32) {
        return 0u;
    }
    return property->GetUInt32();
}

void RecordBrokerRequest(
    String^ command,
    String^ deviceId,
    String^ mediaCodecName,
    String^ spatialAudioSubtype) {
    auto values = ApplicationData::Current->LocalSettings->Values;
    const unsigned int requestCount = ReadUInt32(values, L"SpatialLicenseBroker.RequestCount") + 1u;
    values->Insert(L"SpatialLicenseBroker.RequestCount", PropertyValue::CreateUInt32(requestCount));
    values->Insert(L"SpatialLicenseBroker.LastCommand", PropertyValue::CreateString(command));
    values->Insert(L"SpatialLicenseBroker.LastDeviceID", PropertyValue::CreateString(deviceId));
    values->Insert(L"SpatialLicenseBroker.LastMediaCodecName", PropertyValue::CreateString(mediaCodecName));
    values->Insert(L"SpatialLicenseBroker.LastSpatialAudioSubtype", PropertyValue::CreateString(spatialAudioSubtype));
}

void InsertLicenseResponse(ValueSet^ response, unsigned int status) {
    // SpatialAudioLicenseSrv's ETW contract exposes the response status code as
    // UINT32. Keep the ValueSet wire type aligned with that broker contract.
    // Status 0 is the companion's free/non-expiring development policy;
    // Windows still retains authority to reject the package, format, or endpoint.
    response->Insert(L"Status", PropertyValue::CreateUInt32(status));
    response->Insert(L"STATUS", PropertyValue::CreateUInt32(status));
    response->Insert(L"ExpirationDate", PropertyValue::CreateInt64(kDevelopmentExpirationFileTime));
    response->Insert(L"IsAudioRendererCapable", PropertyValue::CreateBoolean(true));
    response->Insert(L"SpeakerProtectionOverride", PropertyValue::CreateBoolean(false));
    response->Insert(L"LaunchUri", PropertyValue::CreateString(L""));
    response->Insert(L"MediaCodecName", PropertyValue::CreateString(L"Omniphony"));
}

} // namespace

SpatialLicenseService::SpatialLicenseService()
{
}

void SpatialLicenseService::Run(IBackgroundTaskInstance^ taskInstance)
{
    _taskDeferral = Platform::Agile<BackgroundTaskDeferral>(taskInstance->GetDeferral());
    taskInstance->Canceled += ref new BackgroundTaskCanceledEventHandler(
        this, &SpatialLicenseService::OnCanceled);

    auto details = dynamic_cast<AppServiceTriggerDetails^>(taskInstance->TriggerDetails);
    if (details == nullptr || details->AppServiceConnection == nullptr) {
        _taskDeferral->Complete();
        return;
    }

    details->AppServiceConnection->RequestReceived +=
        ref new TypedEventHandler<AppServiceConnection^, AppServiceRequestReceivedEventArgs^>(
            this, &SpatialLicenseService::OnRequestReceived);
}

void SpatialLicenseService::OnCanceled(
    IBackgroundTaskInstance^ /*sender*/,
    BackgroundTaskCancellationReason /*reason*/)
{
    if (_taskDeferral.Get() != nullptr) {
        _taskDeferral->Complete();
    }
}

void SpatialLicenseService::OnRequestReceived(
    AppServiceConnection^ /*sender*/,
    AppServiceRequestReceivedEventArgs^ args)
{
    auto messageDeferral = args->GetDeferral();
    try {
        auto request = args->Request;
        auto message = request->Message;
        auto command = ReadText(message, L"Command");
        auto deviceId = ReadText(message, L"DeviceID");
        auto mediaCodecName = ReadText(message, L"MediaCodecName");
        auto spatialAudioSubtype = ReadText(message, L"SpatialAudioSubtype");

        OutputDebugStringW((L"OmniphonySpatialLicenseService Command=" + command +
            L" DeviceID=" + deviceId + L" MediaCodecName=" + mediaCodecName +
            L" SpatialAudioSubtype=" + spatialAudioSubtype + L"\n")->Data());

        // This breadcrumb is deliberately package-local and diagnostic only. It
        // distinguishes our own direct AppService self-test from a request that
        // Windows' spatial-license path actually routes into this broker.
        try {
            RecordBrokerRequest(command, deviceId, mediaCodecName, spatialAudioSubtype);
        } catch (...) {
            OutputDebugStringW(L"OmniphonySpatialLicenseService observation write failed.\n");
        }

        auto response = ref new ValueSet();
        if (command == L"GetLicenseInfo" || command == L"GetRuntimeParameters") {
            InsertLicenseResponse(response, 0u);
        } else {
            InsertLicenseResponse(response, 1u);
        }

        const auto status = create_task(request->SendResponseAsync(response)).get();
        if (status != AppServiceResponseStatus::Success) {
            OutputDebugStringW(L"OmniphonySpatialLicenseService SendResponseAsync failed.\n");
        }
    } catch (...) {
        OutputDebugStringW(L"OmniphonySpatialLicenseService request handling failed.\n");
    }
    messageDeferral->Complete();
}
