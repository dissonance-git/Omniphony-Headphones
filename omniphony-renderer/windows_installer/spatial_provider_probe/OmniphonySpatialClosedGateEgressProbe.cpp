#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <avrt.h>
#include <mmreg.h>
#include <spatialaudioclient.h>

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <cwchar>
#include <iostream>
#include <memory>
#include <system_error>
#include <thread>

#include "OmniphonySpatialRawOutputPump.h"
#include "OmniphonySpatialRealtimeBridge.h"
#include "OmniphonySpatialRoles.h"
#include "OmniphonySpatialStereoQueue.h"

namespace {

constexpr std::uint32_t kSampleRate = 48'000;
constexpr std::uint32_t kFramesPerQuantum = 480;
constexpr std::size_t kDiagnosticQueueFrames = kFramesPerQuantum * 8;
constexpr std::uint32_t kPrefillQuanta = 4;
constexpr double kPi = 3.14159265358979323846;

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

class ProducerQuantumClock final {
public:
    ProducerQuantumClock() = default;
    ~ProducerQuantumClock() {
        if (timer_) {
            CancelWaitableTimer(timer_);
            CloseHandle(timer_);
        }
    }

    ProducerQuantumClock(const ProducerQuantumClock&) = delete;
    ProducerQuantumClock& operator=(const ProducerQuantumClock&) = delete;

    HRESULT Open(DWORD periodMs) noexcept {
        timer_ = CreateWaitableTimerExW(
            nullptr,
            nullptr,
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
            TIMER_MODIFY_STATE | SYNCHRONIZE);
        if (timer_) {
            highResolution_ = true;
        } else {
            // The reference host is Windows 11, but keep the diagnostic usable
            // on older hosts/SDK combinations without globally changing the
            // system timer resolution.
            timer_ = CreateWaitableTimerExW(
                nullptr,
                nullptr,
                0,
                TIMER_MODIFY_STATE | SYNCHRONIZE);
            highResolution_ = false;
        }
        if (!timer_) {
            return HRESULT_FROM_WIN32(GetLastError());
        }

        LARGE_INTEGER due{};
        due.QuadPart = -static_cast<LONGLONG>(periodMs) * 10'000LL;
        if (!SetWaitableTimerEx(
                timer_,
                &due,
                static_cast<LONG>(periodMs),
                nullptr,
                nullptr,
                nullptr,
                0)) {
            const HRESULT result = HRESULT_FROM_WIN32(GetLastError());
            CloseHandle(timer_);
            timer_ = nullptr;
            return result;
        }
        return S_OK;
    }

    HRESULT Wait() noexcept {
        if (!timer_) {
            return E_HANDLE;
        }
        const DWORD waitResult = WaitForSingleObject(timer_, 1'000);
        if (waitResult == WAIT_OBJECT_0) {
            return S_OK;
        }
        const DWORD error = waitResult == WAIT_FAILED
            ? GetLastError()
            : ERROR_TIMEOUT;
        return HRESULT_FROM_WIN32(
            error == ERROR_SUCCESS ? ERROR_GEN_FAILURE : error);
    }

    bool HighResolution() const noexcept { return highResolution_; }

private:
    HANDLE timer_ = nullptr;
    bool highResolution_ = false;
};

int Fail(const wchar_t* stage, HRESULT hr) {
    std::wcerr << L"SPATIAL_CLOSED_GATE_EGRESS_FAIL stage=" << stage
               << L" hr=0x" << std::hex << std::uppercase
               << static_cast<unsigned long>(hr) << std::dec << L"\n";
    return 1;
}

WAVEFORMATEX ObjectFormat() noexcept {
    WAVEFORMATEX format{};
    format.wFormatTag = WAVE_FORMAT_IEEE_FLOAT;
    format.nChannels = 1;
    format.nSamplesPerSec = kSampleRate;
    format.wBitsPerSample = 32;
    format.nBlockAlign = sizeof(float);
    format.nAvgBytesPerSec = format.nSamplesPerSec * format.nBlockAlign;
    return format;
}

bool ParseDuration(const wchar_t* text, std::uint32_t& durationMs) noexcept {
    if (!text || !text[0]) {
        return false;
    }
    wchar_t* end = nullptr;
    const unsigned long value = std::wcstoul(text, &end, 10);
    if (!end || *end != L'\0' || value < 250 || value > 5'000) {
        return false;
    }
    durationMs = static_cast<std::uint32_t>(value);
    return true;
}

HRESULT FillObjectQuantum(
    ISpatialAudioObjectRenderStream* stream,
    ISpatialAudioObject* front,
    ISpatialAudioObject* top,
    std::uint64_t absoluteFrame) noexcept {
    UINT32 availableDynamic = 0;
    UINT32 frames = 0;
    HRESULT result = stream->BeginUpdatingAudioObjects(
        &availableDynamic,
        &frames);
    if (FAILED(result)) {
        return result;
    }
    if (availableDynamic != 0 || frames != kFramesPerQuantum) {
        return E_UNEXPECTED;
    }

    BYTE* frontBytes = nullptr;
    UINT32 frontLength = 0;
    result = front->GetBuffer(&frontBytes, &frontLength);
    if (FAILED(result) || !frontBytes ||
        frontLength != frames * sizeof(float)) {
        return FAILED(result) ? result : E_UNEXPECTED;
    }

    BYTE* topBytes = nullptr;
    UINT32 topLength = 0;
    result = top->GetBuffer(&topBytes, &topLength);
    if (FAILED(result) || !topBytes || topLength != frames * sizeof(float)) {
        return FAILED(result) ? result : E_UNEXPECTED;
    }

    auto* frontSamples = reinterpret_cast<float*>(frontBytes);
    auto* topSamples = reinterpret_cast<float*>(topBytes);
    for (UINT32 frame = 0; frame < frames; ++frame) {
        const double t = static_cast<double>(absoluteFrame + frame) /
                         static_cast<double>(kSampleRate);
        // Low-amplitude diagnostic only. Distinct frequencies make it obvious
        // if the two authored static roles collapse before Current.
        frontSamples[frame] = static_cast<float>(
            0.018 * std::sin(2.0 * kPi * 330.0 * t));
        topSamples[frame] = static_cast<float>(
            0.012 * std::sin(2.0 * kPi * 550.0 * t));
    }

    return stream->EndUpdatingAudioObjects();
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc < 3 || argc > 4) {
        std::wcerr << L"usage: OmniphonySpatialClosedGateEgressProbe.exe "
                   << L"C:\\absolute\\path\\omniphony_realtime.dll "
                   << L"<physical-endpoint-id> [duration-ms:250..5000]\n";
        return 2;
    }

    std::uint32_t durationMs = 1'500;
    if (argc == 4 && !ParseDuration(argv[3], durationMs)) {
        std::wcerr << L"SPATIAL_CLOSED_GATE_EGRESS_BAD_DURATION\n";
        return 2;
    }

    CoInit co;
    if (FAILED(co.Result()) && co.Result() != RPC_E_CHANGED_MODE) {
        return Fail(L"CoInitializeEx", co.Result());
    }

    auto queue = std::make_shared<OmniphonySpatialStereoQueue>();
    if (!queue->Open(kDiagnosticQueueFrames)) {
        return Fail(L"queue-open", E_OUTOFMEMORY);
    }

    auto format = ObjectFormat();
    SpatialAudioObjectRenderStreamActivationParams params{};
    params.ObjectFormat = &format;
    params.StaticObjectTypeMask = static_cast<AudioObjectType>(
        OmniphonySpatialObjectBits(AudioObjectType_FrontLeft) |
        OmniphonySpatialObjectBits(AudioObjectType_TopFrontLeft));
    params.MinDynamicObjectCount = 0;
    params.MaxDynamicObjectCount = 0;
    params.Category = AudioCategory_GameEffects;
    params.EventHandle = nullptr;
    params.NotifyObject = nullptr;

    ISpatialAudioObjectRenderStream* stream = nullptr;
    HRESULT result = CreateOmniphonyStaticProbeStreamWithRealtimeBridgeAndQueue(
        params,
        argv[1],
        queue,
        &stream);
    if (FAILED(result) || !stream) {
        return Fail(L"CreateComCurrentQueueStream", FAILED(result) ? result : E_FAIL);
    }

    ISpatialAudioObject* front = nullptr;
    ISpatialAudioObject* top = nullptr;
    result = stream->ActivateSpatialAudioObject(AudioObjectType_FrontLeft, &front);
    if (FAILED(result) || !front) {
        stream->Release();
        return Fail(L"ActivateFrontLeft", FAILED(result) ? result : E_FAIL);
    }
    result = stream->ActivateSpatialAudioObject(AudioObjectType_TopFrontLeft, &top);
    if (FAILED(result) || !top) {
        front->Release();
        stream->Release();
        return Fail(L"ActivateTopFrontLeft", FAILED(result) ? result : E_FAIL);
    }

    OmniphonySpatialRawOutputPump pump;
    result = pump.Open(argv[2], queue);
    if (FAILED(result)) {
        top->Release();
        front->Release();
        stream->Release();
        return Fail(L"OpenExactRawEndpoint", result);
    }

    result = stream->Start();
    if (FAILED(result)) {
        pump.Close();
        top->Release();
        front->Release();
        stream->Release();
        return Fail(L"StartInternalStaticStream", result);
    }

    std::uint64_t absoluteFrame = 0;
    for (std::uint32_t quantum = 0; quantum < kPrefillQuanta; ++quantum) {
        result = FillObjectQuantum(stream, front, top, absoluteFrame);
        if (FAILED(result)) {
            stream->Stop();
            pump.Close();
            top->Release();
            front->Release();
            stream->Release();
            return Fail(L"PrefillCurrentQueue", result);
        }
        absoluteFrame += kFramesPerQuantum;
        Sleep(10);
    }

    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_WARNING audible-low-level-test-tone\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_ENDPOINT_PERIOD_FRAMES "
               << pump.PeriodFrames() << L"\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_QUEUE_CAPACITY_FRAMES "
               << queue->CapacityFrames() << L"\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_PRODUCER_TIMER_HIGH_RESOLUTION "
               << (producerClock.HighResolution() ? 1 : 0) << L"\n";

    result = pump.Start();
    if (FAILED(result)) {
        stream->Stop();
        pump.Close();
        top->Release();
        front->Release();
        stream->Release();
        return Fail(L"StartRawEndpoint", result);
    }

    ProducerQuantumClock producerClock;
    result = producerClock.Open(10);
    if (FAILED(result)) {
        (void)pump.Stop();
        stream->Stop();
        pump.Close();
        top->Release();
        front->Release();
        stream->Release();
        return Fail(L"OpenProducerQuantumClock", result);
    }

    std::atomic<bool> stopConsumer{false};
    std::atomic<HRESULT> consumerResult{S_OK};
    std::thread consumer;
    try {
        consumer = std::thread([&]() {
            DWORD taskIndex = 0;
            HANDLE mmcss = AvSetMmThreadCharacteristicsW(L"Pro Audio", &taskIndex);

            while (!stopConsumer.load(std::memory_order_acquire)) {
                const DWORD waitResult = WaitForSingleObject(
                    pump.SampleReadyEvent(),
                    100);
                if (stopConsumer.load(std::memory_order_acquire)) {
                    break;
                }
                if (waitResult == WAIT_TIMEOUT) {
                    continue;
                }
                if (waitResult != WAIT_OBJECT_0) {
                    const DWORD error = waitResult == WAIT_FAILED
                        ? GetLastError()
                        : ERROR_GEN_FAILURE;
                    consumerResult.store(
                        HRESULT_FROM_WIN32(error == ERROR_SUCCESS
                            ? ERROR_GEN_FAILURE
                            : error),
                        std::memory_order_release);
                    break;
                }

                const HRESULT drainResult = pump.DrainOnce();
                if (FAILED(drainResult)) {
                    consumerResult.store(drainResult, std::memory_order_release);
                    break;
                }
            }

            if (mmcss) {
                AvRevertMmThreadCharacteristics(mmcss);
            }
        });
    }
    catch (const std::system_error&) {
        (void)pump.Stop();
        stream->Stop();
        pump.Close();
        top->Release();
        front->Release();
        stream->Release();
        return Fail(L"CreateEndpointEventThread", E_FAIL);
    }

    const std::uint32_t totalQuanta = std::max<std::uint32_t>(
        1,
        durationMs / 10);
    for (std::uint32_t quantum = 0; quantum < totalQuanta; ++quantum) {
        const HRESULT asyncResult = consumerResult.load(std::memory_order_acquire);
        if (FAILED(asyncResult)) {
            result = asyncResult;
            break;
        }

        result = FillObjectQuantum(stream, front, top, absoluteFrame);
        if (FAILED(result)) {
            break;
        }
        absoluteFrame += kFramesPerQuantum;

        result = producerClock.Wait();
        if (FAILED(result)) {
            break;
        }
    }

    stopConsumer.store(true, std::memory_order_release);
    if (pump.SampleReadyEvent()) {
        SetEvent(pump.SampleReadyEvent());
    }
    if (consumer.joinable()) {
        consumer.join();
    }

    const HRESULT asyncResult = consumerResult.load(std::memory_order_acquire);
    if (SUCCEEDED(result) && FAILED(asyncResult)) {
        result = asyncResult;
    }

    const HRESULT pumpStop = pump.Stop();
    if (SUCCEEDED(result) && FAILED(pumpStop)) {
        result = pumpStop;
    }
    const HRESULT streamStop = stream->Stop();
    if (SUCCEEDED(result) && FAILED(streamStop)) {
        result = streamStop;
    }

    const auto drainCycles = pump.DrainCycles();
    const auto realFrames = pump.RealFramesWritten();
    const auto silenceFrames = pump.SilenceFramesWritten();
    const auto droppedFrames = queue->DroppedFrames();
    const auto underrunFrames = queue->UnderrunFrames();

    pump.Close();
    top->Release();
    front->Release();
    stream->Release();

    if (FAILED(result)) {
        return Fail(L"RunClosedGateEgress", result);
    }
    if (drainCycles == 0 || realFrames == 0 || droppedFrames != 0) {
        return Fail(L"EgressObservability", E_FAIL);
    }

    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_OK 1\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_COM_TO_CURRENT 1\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_CURRENT_TO_QUEUE 1\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_ENDPOINT_EVENT_CLOCK 1\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_RAW_RENDER_CLIENT 1\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_DRAIN_CYCLES " << drainCycles << L"\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_REAL_FRAMES " << realFrames << L"\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_SILENCE_FRAMES " << silenceFrames << L"\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_QUEUE_DROPPED_FRAMES " << droppedFrames << L"\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_QUEUE_UNDERRUN_FRAMES " << underrunFrames << L"\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_PROVIDER_REGISTERED 0\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_PROVIDER_SELECTED 0\n";
    std::wcout << L"SPATIAL_CLOSED_GATE_EGRESS_PUBLIC_PROVIDER_GATE_OPENED 0\n";
    return 0;
}
