#pragma once

#include <collection.h>

namespace OmniphonySpatialLicenseService
{
    [Windows::Foundation::Metadata::WebHostHidden]
    public ref class SpatialLicenseService sealed :
        public Windows::ApplicationModel::Background::IBackgroundTask
    {
    public:
        SpatialLicenseService();

        virtual void Run(
            Windows::ApplicationModel::Background::IBackgroundTaskInstance^ taskInstance);

    private:
        void OnCanceled(
            Windows::ApplicationModel::Background::IBackgroundTaskInstance^ sender,
            Windows::ApplicationModel::Background::BackgroundTaskCancellationReason reason);
        void OnRequestReceived(
            Windows::ApplicationModel::AppService::AppServiceConnection^ sender,
            Windows::ApplicationModel::AppService::AppServiceRequestReceivedEventArgs^ args);

        Platform::Agile<Windows::ApplicationModel::Background::BackgroundTaskDeferral> _taskDeferral;
    };
}
