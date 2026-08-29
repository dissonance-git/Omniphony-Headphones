#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <audioclient.h>
#include <mmdeviceapi.h>
#include <mmreg.h>
#include <propidl.h>
#include <spatialaudioclient.h>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cwchar>
#include <iomanip>
#include <iostream>
#include <string>

namespace {

constexpr UINT32 kSampleRate = 48'000;
constexpr UINT32 kDefaultDurationMs = 1'500;
constexpr UINT32 kMinimumDurationMs = 250;
constexpr UINT32 kMaximumDurationMs = 5'000;
constexpr DWORD kSourceRequestTimeoutMs = 2'000;
constexpr DWORD kPostRollMs = 150;
constexpr float kToneHz = 550.0f;
constexpr float kToneAmplitude = 0.012f;
constexpr double kPi = 3.14159265358979323846;

template <typename T>
class ComPtr final {
public:
    ComPtr() = default;
    ~ComPtr() { Reset(); }

    ComPtr(const ComPtr&) = delete;
    ComPtr& operator=(const ComPtr&) = delete;

    T* Get() const noexcept { return value_; }
    T* operator->() const noexcept { return value_; }

    T** Put() noexcept {
        Reset();
        return &value_;
    }

    void Reset() noexcept {
        if (value_) {
            value_->Release();
            value_ = nullptr;
        }
    }

private:
    T* value_ = nullptr;
};

class ScopedHandle final {
public:
    explicit ScopedHandle(HANDLE value = nullptr) noexcept : value_(value) {}
    ~ScopedHandle() {
        if (value_) {
            CloseHandle(value_);
        }
    }

    ScopedHandle(const ScopedHandle&) = delete;
    ScopedHandle& operator=(const ScopedHandle&) = delete;

    HANDLE Get() const noexcept { return value_; }
    bool Valid() const noexcept { return value_ != nullptr; }

private:
    HANDLE value_ = nullptr;
};

class CoInit final {
public:
    CoInit() noexcept : hr_(CoInitializeEx(nullptr, COINIT_MULTITHREADED)) {}
    ~CoInit() {
        if (SUCCEEDED(hr_)) {
            CoUninitialize();
        }
    }

    HRESULT Result() const noexcept { return hr_; }

private:
    HRESULT hr_ = E_FAIL;
};

int Fail(const wchar_t* stage, HRESULT hr) {
    std::wcerr << L"SPATIAL_REAL_STATIC_CLIENT_FAIL stage=" << stage
               << L" hr=0x" << std::hex << std::uppercase
               << static_cast<unsigned long>(hr) << std::dec << L"\n";
    return 1;
}

int FailText(const wchar_t* stage, const wchar_t* detail) {
    std::wcerr << L"SPATIAL_REAL_STATIC_CLIENT_FAIL stage=" << stage
               << L" detail=" << detail << L"\n";
    return 1;
}

bool ParseDuration(const wchar_t* text, UINT32& durationMs) {
    if (!text || !*text) {
        return false;
    }
    wchar_t* end = nullptr;
    const unsigned long parsed = std::wcstoul(text, &end, 10);
    if (end == text || !end || *end != L'\0' ||
        parsed < kMinimumDurationMs || parsed > kMaximumDurationMs) {
        return false;
    }
    durationMs = static_cast<UINT32>(parsed);
    return true;
}

void FillObjectFormat(WAVEFORMATEX& format) {
    ZeroMemory(&format, sizeof(format));
    format.wFormatTag = WAVE_FORMAT_IEEE_FLOAT;
    format.nChannels = 1;
    format.nSamplesPerSec = kSampleRate;
    format.wBitsPerSample = 32;
    format.nBlockAlign = sizeof(float);
    format.nAvgBytesPerSec = kSampleRate * sizeof(float);
    format.cbSize = 0;
}

float ToneSample(std::uint64_t frame, std::uint64_t totalFrames) {
    const std::uint64_t fadeFrames = 240; // 5 ms at 48 kHz.
    float envelope = 1.0f;
    if (frame < fadeFrames) {
        envelope = static_cast<float>(frame) / static_cast<float>(fadeFrames);
    }
    if (frame + fadeFrames >= totalFrames) {
        const std::uint64_t remaining = totalFrames > frame ? totalFrames - frame - 1 : 0;
        envelope = std::min(
            envelope,
            static_cast<float>(remaining) / static_cast<float>(fadeFrames));
    }

    const double phase =
        2.0 * kPi * static_cast<double>(kToneHz) *
        static_cast<double>(frame) / static_cast<double>(kSampleRate);
    return kToneAmplitude * envelope * static_cast<float>(std::sin(phase));
}

void PrintContract() {
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_CONTRACT_OK 1\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_ROUTE IMMDEVICE_ISPATIALAUDIOCLIENT\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_ROLE TOP_FRONT_LEFT\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_OBJECT_TYPE "
               << static_cast<unsigned long>(AudioObjectType_TopFrontLeft) << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_FORMAT FLOAT32_48000_MONO\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_TONE_HZ " << kToneHz << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_DEFAULT_DURATION_MS "
               << kDefaultDurationMs << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_REQUIRES_SELECTED_PROVIDER 1\n";
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc == 2 && std::wstring(argv[1]) == L"--contract") {
        PrintContract();
        return 0;
    }

    if (argc < 2 || argc > 3 || !argv[1] || !*argv[1]) {
        std::wcerr
            << L"usage: OmniphonySpatialStaticObjectClientProbe <endpoint-id> [duration-ms]\n"
            << L"       OmniphonySpatialStaticObjectClientProbe --contract\n";
        return 2;
    }

    UINT32 durationMs = kDefaultDurationMs;
    if (argc == 3 && !ParseDuration(argv[2], durationMs)) {
        return FailText(L"duration", L"duration must be 250..5000 ms");
    }

    CoInit co;
    if (FAILED(co.Result()) && co.Result() != RPC_E_CHANGED_MODE) {
        return Fail(L"CoInitializeEx", co.Result());
    }

    ComPtr<IMMDeviceEnumerator> enumerator;
    HRESULT hr = CoCreateInstance(
        __uuidof(MMDeviceEnumerator),
        nullptr,
        CLSCTX_INPROC_SERVER,
        __uuidof(IMMDeviceEnumerator),
        reinterpret_cast<void**>(enumerator.Put()));
    if (FAILED(hr)) {
        return Fail(L"CoCreateInstance(MMDeviceEnumerator)", hr);
    }

    ComPtr<IMMDevice> device;
    hr = enumerator->GetDevice(argv[1], device.Put());
    if (FAILED(hr)) {
        return Fail(L"IMMDeviceEnumerator::GetDevice", hr);
    }

    ComPtr<ISpatialAudioClient> spatialClient;
    hr = device->Activate(
        __uuidof(ISpatialAudioClient),
        CLSCTX_INPROC_SERVER,
        nullptr,
        reinterpret_cast<void**>(spatialClient.Put()));
    if (FAILED(hr)) {
        return Fail(L"IMMDevice::Activate(ISpatialAudioClient)", hr);
    }

    WAVEFORMATEX format{};
    FillObjectFormat(format);

    hr = spatialClient->IsAudioObjectFormatSupported(&format);
    if (FAILED(hr)) {
        return Fail(L"ISpatialAudioClient::IsAudioObjectFormatSupported", hr);
    }

    hr = spatialClient->IsSpatialAudioStreamAvailable(
        __uuidof(ISpatialAudioObjectRenderStream),
        nullptr);
    if (FAILED(hr)) {
        return Fail(L"ISpatialAudioClient::IsSpatialAudioStreamAvailable", hr);
    }

    ScopedHandle event(CreateEventW(nullptr, FALSE, FALSE, nullptr));
    if (!event.Valid()) {
        return Fail(L"CreateEventW", HRESULT_FROM_WIN32(GetLastError()));
    }

    SpatialAudioObjectRenderStreamActivationParams streamParams{};
    streamParams.ObjectFormat = &format;
    streamParams.StaticObjectTypeMask = AudioObjectType_TopFrontLeft;
    streamParams.MinDynamicObjectCount = 0;
    streamParams.MaxDynamicObjectCount = 0;
    streamParams.Category = AudioCategory_SoundEffects;
    streamParams.EventHandle = event.Get();
    streamParams.NotifyObject = nullptr;

    PROPVARIANT activation{};
    activation.vt = VT_BLOB;
    activation.blob.cbSize = sizeof(streamParams);
    activation.blob.pBlobData = reinterpret_cast<BYTE*>(&streamParams);

    ComPtr<ISpatialAudioObjectRenderStream> stream;
    hr = spatialClient->ActivateSpatialAudioStream(
        &activation,
        __uuidof(ISpatialAudioObjectRenderStream),
        reinterpret_cast<void**>(stream.Put()));
    if (FAILED(hr)) {
        return Fail(L"ISpatialAudioClient::ActivateSpatialAudioStream", hr);
    }

    bool streamStarted = false;
    auto failRunning = [&](const wchar_t* stage, HRESULT error) -> int {
        if (streamStarted) {
            (void)stream->Stop();
            streamStarted = false;
        }
        (void)stream->Reset();
        return Fail(stage, error);
    };

    hr = stream->Start();
    if (FAILED(hr)) {
        return Fail(L"ISpatialAudioObjectRenderStream::Start", hr);
    }
    streamStarted = true;

    const std::uint64_t totalFrames =
        static_cast<std::uint64_t>(kSampleRate) *
        static_cast<std::uint64_t>(durationMs) / 1'000ull;
    std::uint64_t framesSubmitted = 0;
    UINT32 updatePasses = 0;
    bool objectActivated = false;
    bool endOfStreamSubmitted = false;
    ComPtr<ISpatialAudioObject> object;

    while (framesSubmitted < totalFrames) {
        const DWORD wait = WaitForSingleObject(event.Get(), kSourceRequestTimeoutMs);
        if (wait != WAIT_OBJECT_0) {
            const HRESULT waitHr = wait == WAIT_FAILED
                ? HRESULT_FROM_WIN32(GetLastError())
                : HRESULT_FROM_WIN32(ERROR_TIMEOUT);
            return failRunning(L"WaitForSingleObject(source-request)", waitHr);
        }

        UINT32 availableDynamicObjects = 0;
        UINT32 frameCount = 0;
        hr = stream->BeginUpdatingAudioObjects(
            &availableDynamicObjects,
            &frameCount);
        if (FAILED(hr)) {
            return failRunning(L"BeginUpdatingAudioObjects", hr);
        }
        ++updatePasses;

        if (!objectActivated) {
            hr = stream->ActivateSpatialAudioObject(
                AudioObjectType_TopFrontLeft,
                object.Put());
            if (FAILED(hr)) {
                (void)stream->EndUpdatingAudioObjects();
                return failRunning(L"ActivateSpatialAudioObject(TopFrontLeft)", hr);
            }
            objectActivated = true;

            hr = object->SetVolume(1.0f);
            if (FAILED(hr)) {
                (void)stream->EndUpdatingAudioObjects();
                return failRunning(L"ISpatialAudioObject::SetVolume", hr);
            }
        }

        BYTE* bufferBytes = nullptr;
        UINT32 bufferLength = 0;
        hr = object->GetBuffer(&bufferBytes, &bufferLength);
        if (FAILED(hr)) {
            (void)stream->EndUpdatingAudioObjects();
            return failRunning(L"ISpatialAudioObject::GetBuffer", hr);
        }
        if (!bufferBytes ||
            bufferLength != frameCount * static_cast<UINT32>(sizeof(float))) {
            (void)stream->EndUpdatingAudioObjects();
            return failRunning(L"object-buffer-contract", E_UNEXPECTED);
        }

        auto* buffer = reinterpret_cast<float*>(bufferBytes);
        std::fill(buffer, buffer + frameCount, 0.0f);

        const std::uint64_t remaining = totalFrames - framesSubmitted;
        const UINT32 validFrames = static_cast<UINT32>(
            std::min<std::uint64_t>(remaining, frameCount));

        for (UINT32 frame = 0; frame < validFrames; ++frame) {
            buffer[frame] = ToneSample(framesSubmitted + frame, totalFrames);
        }

        const bool finalPass = remaining <= frameCount;
        if (finalPass) {
            hr = object->SetEndOfStream(validFrames);
            if (FAILED(hr)) {
                (void)stream->EndUpdatingAudioObjects();
                return failRunning(L"ISpatialAudioObject::SetEndOfStream", hr);
            }
            endOfStreamSubmitted = true;
        }

        hr = stream->EndUpdatingAudioObjects();
        if (FAILED(hr)) {
            return failRunning(L"EndUpdatingAudioObjects", hr);
        }

        framesSubmitted += validFrames;
        if (finalPass) {
            object.Reset();
            break;
        }
    }

    // The provider owns a bounded stereo queue between the object callback and
    // the physical endpoint. Give the real endpoint clock a short bounded
    // post-roll to drain the final submitted object frames before Stop().
    Sleep(kPostRollMs);

    hr = stream->Stop();
    streamStarted = false;
    if (FAILED(hr)) {
        (void)stream->Reset();
        return Fail(L"ISpatialAudioObjectRenderStream::Stop", hr);
    }

    hr = stream->Reset();
    if (FAILED(hr)) {
        return Fail(L"ISpatialAudioObjectRenderStream::Reset", hr);
    }

    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_OK 1\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_ENDPOINT_ID " << argv[1] << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_ROUTE IMMDEVICE_ISPATIALAUDIOCLIENT\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_STREAM_AVAILABLE 1\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_STREAM_ACTIVATED 1\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_STREAM_STARTED 1\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_ROLE TOP_FRONT_LEFT\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_OBJECT_ACTIVATED "
               << (objectActivated ? 1 : 0) << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_FORMAT FLOAT32_48000_MONO\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_TONE_HZ " << kToneHz << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_DURATION_MS " << durationMs << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_UPDATE_PASSES " << updatePasses << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_FRAMES_SUBMITTED "
               << framesSubmitted << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_END_OF_STREAM "
               << (endOfStreamSubmitted ? 1 : 0) << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_POSTROLL_MS " << kPostRollMs << L"\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_STREAM_STOPPED 1\n";
    std::wcout << L"SPATIAL_REAL_STATIC_CLIENT_STREAM_RESET 1\n";
    return 0;
}
