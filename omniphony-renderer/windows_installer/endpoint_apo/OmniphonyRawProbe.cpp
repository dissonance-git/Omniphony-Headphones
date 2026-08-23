#include <windows.h>
#include <audioclient.h>
#include <mmdeviceapi.h>
#include <propsys.h>
#include <propvarutil.h>
#include <wrl/client.h>

#include <iomanip>
#include <iostream>

using Microsoft::WRL::ComPtr;

namespace {

// System.Devices.AudioDevice.RawProcessingSupported
// FMTID 8943B373-388C-4395-B557-BC6DBAFFAFDB, PID 2.
constexpr PROPERTYKEY kRawProcessingSupported = {
    {0x8943b373, 0x388c, 0x4395, {0xb5, 0x57, 0xbc, 0x6d, 0xba, 0xff, 0xaf, 0xdb}},
    2};

int Fail(const wchar_t* stage, HRESULT hr) {
    std::wcerr << L"RAW_STEREO_PROBE_FAIL stage=" << stage
               << L" hr=0x" << std::hex << std::uppercase
               << static_cast<unsigned long>(hr) << std::dec << L"\n";
    return 1;
}

bool PropVariantBool(const PROPVARIANT& value, bool& result) noexcept {
    switch (value.vt) {
    case VT_BOOL:
        result = value.boolVal != VARIANT_FALSE;
        return true;
    case VT_UI4:
        result = value.ulVal != 0;
        return true;
    case VT_I4:
        result = value.lVal != 0;
        return true;
    default:
        return false;
    }
}

} // namespace

int wmain() {
    const HRESULT coHr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    if (FAILED(coHr)) {
        return Fail(L"CoInitializeEx", coHr);
    }

    ComPtr<IMMDeviceEnumerator> enumerator;
    HRESULT hr = CoCreateInstance(
        __uuidof(MMDeviceEnumerator),
        nullptr,
        CLSCTX_ALL,
        IID_PPV_ARGS(enumerator.GetAddressOf()));
    if (FAILED(hr)) {
        CoUninitialize();
        return Fail(L"CoCreateInstance(MMDeviceEnumerator)", hr);
    }

    ComPtr<IMMDevice> endpoint;
    hr = enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, endpoint.GetAddressOf());
    if (FAILED(hr)) {
        CoUninitialize();
        return Fail(L"GetDefaultAudioEndpoint", hr);
    }

    LPWSTR endpointId = nullptr;
    if (SUCCEEDED(endpoint->GetId(&endpointId)) && endpointId) {
        std::wcout << L"RAW_ENDPOINT_ID " << endpointId << L"\n";
        CoTaskMemFree(endpointId);
    }

    bool rawPropertyPresent = false;
    bool rawPropertySupported = false;
    ComPtr<IPropertyStore> store;
    hr = endpoint->OpenPropertyStore(STGM_READ, store.GetAddressOf());
    if (SUCCEEDED(hr)) {
        PROPVARIANT value;
        PropVariantInit(&value);
        const HRESULT propHr = store->GetValue(kRawProcessingSupported, &value);
        if (SUCCEEDED(propHr)) {
            rawPropertyPresent = PropVariantBool(value, rawPropertySupported);
        }
        PropVariantClear(&value);
    }
    std::wcout << L"RAW_PROCESSING_PROPERTY_PRESENT " << (rawPropertyPresent ? 1 : 0) << L"\n";
    std::wcout << L"RAW_PROCESSING_SUPPORTED " << (rawPropertySupported ? 1 : 0) << L"\n";

    ComPtr<IAudioClient> baseClient;
    hr = endpoint->Activate(
        __uuidof(IAudioClient),
        CLSCTX_ALL,
        nullptr,
        reinterpret_cast<void**>(baseClient.GetAddressOf()));
    if (FAILED(hr)) {
        CoUninitialize();
        return Fail(L"IMMDevice::Activate(IAudioClient)", hr);
    }

    ComPtr<IAudioClient2> client;
    hr = baseClient.As(&client);
    if (FAILED(hr)) {
        CoUninitialize();
        return Fail(L"QueryInterface(IAudioClient2)", hr);
    }

    WAVEFORMATEX* mixFormat = nullptr;
    hr = client->GetMixFormat(&mixFormat);
    if (FAILED(hr) || !mixFormat) {
        if (mixFormat) {
            CoTaskMemFree(mixFormat);
        }
        CoUninitialize();
        return Fail(L"GetMixFormat", FAILED(hr) ? hr : E_FAIL);
    }

    std::wcout << L"RAW_ENDPOINT_MIX_FORMAT "
               << mixFormat->nSamplesPerSec << L"Hz,"
               << mixFormat->nChannels << L"ch,"
               << mixFormat->wBitsPerSample << L"bit,tag=0x"
               << std::hex << std::uppercase << mixFormat->wFormatTag
               << std::dec << L"\n";

    if (mixFormat->nChannels != 2) {
        CoTaskMemFree(mixFormat);
        CoUninitialize();
        std::wcerr << L"RAW_STEREO_PROBE_FAIL stage=endpoint-not-stereo\n";
        return 1;
    }

    AudioClientProperties properties{};
    properties.cbSize = sizeof(properties);
    properties.bIsOffload = FALSE;
    properties.eCategory = AudioCategory_Media;
    properties.Options = AUDCLNT_STREAMOPTIONS_RAW;
    hr = client->SetClientProperties(&properties);
    if (FAILED(hr)) {
        CoTaskMemFree(mixFormat);
        CoUninitialize();
        return Fail(L"IAudioClient2::SetClientProperties(RAW)", hr);
    }
    std::wcout << L"RAW_CLIENT_PROPERTIES_OK 1\n";

    WAVEFORMATEX* closest = nullptr;
    hr = client->IsFormatSupported(AUDCLNT_SHAREMODE_SHARED, mixFormat, &closest);
    if (closest) {
        CoTaskMemFree(closest);
    }
    if (hr != S_OK) {
        CoTaskMemFree(mixFormat);
        CoUninitialize();
        return Fail(L"IsFormatSupported(shared RAW endpoint mix)", hr);
    }
    std::wcout << L"RAW_STEREO_FORMAT_SUPPORTED 1\n";

    hr = client->Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        AUDCLNT_STREAMFLAGS_NOPERSIST,
        0,
        0,
        mixFormat,
        nullptr);
    CoTaskMemFree(mixFormat);
    if (FAILED(hr)) {
        CoUninitialize();
        return Fail(L"IAudioClient::Initialize(shared RAW stereo)", hr);
    }

    UINT32 bufferFrames = 0;
    hr = client->GetBufferSize(&bufferFrames);
    if (FAILED(hr)) {
        CoUninitialize();
        return Fail(L"IAudioClient::GetBufferSize", hr);
    }

    std::wcout << L"RAW_STEREO_CLIENT_INITIALIZE_OK 1\n";
    std::wcout << L"RAW_STEREO_BUFFER_FRAMES " << bufferFrames << L"\n";
    std::wcout << L"RAW_STEREO_STREAM_STARTED 0\n";
    std::wcout << L"RAW_PROVIDER_EGRESS_PROBE_OK 1\n";

    CoUninitialize();
    return 0;
}
