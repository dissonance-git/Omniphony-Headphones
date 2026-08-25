#include "SpatialLicenseService.h"

#include <ppltasks.h>

using namespace Concurrency;
using namespace Platform;
using namespace Windows::ApplicationModel::AppService;
using namespace Windows::ApplicationModel::Background;
using namespace Windows::Foundation;
using namespace Windows::Foundation::Collections;
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

void InsertLicenseResponse(ValueSet^ response, int status) {
    // Windows' SpatialAudioLicenseSrv binary exposes these exact response field
    // names. Status 0 is the companion's free/non-expiring development policy;
    // Windows still retains authority to reject the package, format, or endpoint.
    response->Insert(L"Status", PropertyValue::CreateInt32(status));
    response->Insert(L"STATUS", PropertyValue::CreateInt32(status));
    response->Insert(L"ExpirationDate", PropertyValue::CreateInt64(kDevelopmentExpirationFileTime));
    response->Insert(L"IsAudioRendererCapable", PropertyValue::CreateBoolean(true));
    response->Insert(L"SpeakerProtectionOverride", PropertyValue::CreateBoolean(false));
    response->Insert(L"LaunchUri", PropertyValue::CreateString(L""));
    response->Insert(L"MediaCodecName", PropertyValue::CreateString(L"Omniphony"));
}

} // namespace

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

        OutputDebugStringW((L"OmniphonySpatialLicenseService Command=" + command +
            L" DeviceID=" + deviceId + L" MediaCodecName=" + mediaCodecName + L"\n")->Data());

        auto response = ref new ValueSet();
        if (command == L"GetLicenseInfo" || command == L"GetRuntimeParameters") {
            InsertLicenseResponse(response, 0);
        } else {
            InsertLicenseResponse(response, 1);
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
