from pathlib import Path

root = Path.cwd()
source = root / 'omniphony-renderer/windows_installer/endpoint_apo/OmniphonyStreamAPO.cpp'
installer = root / 'omniphony-renderer/windows_installer/endpoint_apo/Install-OmniphonyAdaptive.ps1'


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f'{label}: expected exactly one anchor, found {count}')
    return text.replace(old, new, 1)


text = source.read_text(encoding='utf-8').replace('\r\n', '\n')

text = replace_once(
    text,
    '#include <atomic>\n#include <cstring>',
    '#include <atomic>\n#include <cstdio>\n#include <cstring>',
    'include',
)

text = replace_once(
    text,
    'HINSTANCE g_module = nullptr;\nvolatile LONG g_factoryLocks = 0;\n\n'
    'bool ReadAudioFormat(IAudioMediaType* mediaType, UNCOMPRESSEDAUDIOFORMAT& format) noexcept {',
    r'''HINSTANCE g_module = nullptr;
volatile LONG g_factoryLocks = 0;

constexpr char kTopologyLogPath[] = "C:\\ProgramData\\Omniphony\\apo-topology.log";

void AppendTopologyLog(const char* eventName, const void* instance, const char* detail) noexcept {
    if (!eventName || !detail) return;
    SYSTEMTIME now = {};
    GetSystemTime(&now);
    char line[1024] = {};
    const int count = sprintf_s(
        line,
        "%04u-%02u-%02uT%02u:%02u:%02u.%03uZ\tPID=%lu\tTID=%lu\tINSTANCE=%p\tEVENT=%s\t%s\r\n",
        static_cast<unsigned>(now.wYear),
        static_cast<unsigned>(now.wMonth),
        static_cast<unsigned>(now.wDay),
        static_cast<unsigned>(now.wHour),
        static_cast<unsigned>(now.wMinute),
        static_cast<unsigned>(now.wSecond),
        static_cast<unsigned>(now.wMilliseconds),
        static_cast<unsigned long>(GetCurrentProcessId()),
        static_cast<unsigned long>(GetCurrentThreadId()),
        instance,
        eventName,
        detail);
    if (count <= 0) return;

    const HANDLE file = CreateFileA(
        kTopologyLogPath,
        FILE_APPEND_DATA,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        nullptr,
        OPEN_ALWAYS,
        FILE_ATTRIBUTE_NORMAL,
        nullptr);
    if (file == INVALID_HANDLE_VALUE) return;
    DWORD written = 0;
    WriteFile(file, line, static_cast<DWORD>(count), &written, nullptr);
    CloseHandle(file);
}

bool ReadAudioFormat(IAudioMediaType* mediaType, UNCOMPRESSEDAUDIOFORMAT& format) noexcept {''',
    'topology helper',
)

text = replace_once(
    text,
    r'''    explicit OmniphonyStreamAPO(IUnknown* outer)
        : CBaseAudioProcessingObject(registration),
          outer_(outer ? outer : reinterpret_cast<IUnknown*>(static_cast<INonDelegatingUnknown*>(this))) {
        InterlockedIncrement(&instanceCount);
    }

    ~OmniphonyStreamAPO() override {
        resetProcessing();
        InterlockedDecrement(&instanceCount);
    }''',
    r'''    explicit OmniphonyStreamAPO(IUnknown* outer)
        : CBaseAudioProcessingObject(registration),
          outer_(outer ? outer : reinterpret_cast<IUnknown*>(static_cast<INonDelegatingUnknown*>(this))) {
        const LONG live = InterlockedIncrement(&instanceCount);
        char detail[96] = {};
        sprintf_s(detail, "LIVE=%ld", live);
        AppendTopologyLog("CONSTRUCT", this, detail);
    }

    ~OmniphonyStreamAPO() override {
        char detail[160] = {};
        sprintf_s(
            detail,
            "CALLS=%llu FRAMES=%llu LIVE_BEFORE=%ld",
            static_cast<unsigned long long>(processCalls_.load(std::memory_order_relaxed)),
            static_cast<unsigned long long>(processedFrames_.load(std::memory_order_relaxed)),
            InterlockedCompareExchange(&instanceCount, 0, 0));
        AppendTopologyLog("DESTROY", this, detail);
        resetProcessing();
        InterlockedDecrement(&instanceCount);
    }''',
    'constructor',
)

text = replace_once(
    text,
    r'''        rawBypass_ = IsEqualGUID(processingMode, AUDIO_SIGNALPROCESSINGMODE_RAW);
        m_bIsInitialized = true;
        return S_OK;''',
    r'''        rawBypass_ = IsEqualGUID(processingMode, AUDIO_SIGNALPROCESSINGMODE_RAW);
        char detail[192] = {};
        sprintf_s(
            detail,
            "SIZE=%u RAW=%u MODE_D1=0x%08lX",
            static_cast<unsigned>(dataSize),
            rawBypass_ ? 1u : 0u,
            static_cast<unsigned long>(processingMode.Data1));
        AppendTopologyLog("INITIALIZE", this, detail);
        m_bIsInitialized = true;
        return S_OK;''',
    'initialize',
)

text = replace_once(
    text,
    r'''        inputBytesPerFrame_ = static_cast<size_t>(inputChannels_) * sizeof(float);
        outputBytesPerFrame_ = static_cast<size_t>(outputChannels_) * sizeof(float);

        // RAW mode is an identity transform. Do not allocate a worker-facing''',
    r'''        inputBytesPerFrame_ = static_cast<size_t>(inputChannels_) * sizeof(float);
        outputBytesPerFrame_ = static_cast<size_t>(outputChannels_) * sizeof(float);
        processCalls_.store(0, std::memory_order_relaxed);
        processedFrames_.store(0, std::memory_order_relaxed);
        char detail[256] = {};
        sprintf_s(
            detail,
            "RAW=%u IN_CH=%u OUT_CH=%u RATE=%.0f IN_MASK=0x%08lX OUT_MASK=0x%08lX MAX_FRAMES=%u LIVE=%ld",
            rawBypass_ ? 1u : 0u,
            static_cast<unsigned>(inputFormat.dwSamplesPerFrame),
            static_cast<unsigned>(outputFormat.dwSamplesPerFrame),
            static_cast<double>(inputFormat.fFramesPerSecond),
            static_cast<unsigned long>(inputFormat.dwChannelMask),
            static_cast<unsigned long>(outputFormat.dwChannelMask),
            static_cast<unsigned>(inputs[0]->u32MaxFrameCount),
            InterlockedCompareExchange(&instanceCount, 0, 0));
        AppendTopologyLog("LOCK", this, detail);

        // RAW mode is an identity transform. Do not allocate a worker-facing''',
    'lock',
)

text = replace_once(
    text,
    r'''    HRESULT STDMETHODCALLTYPE UnlockForProcess() override {
        resetProcessing();
        return CBaseAudioProcessingObject::UnlockForProcess();
    }''',
    r'''    HRESULT STDMETHODCALLTYPE UnlockForProcess() override {
        char detail[160] = {};
        sprintf_s(
            detail,
            "CALLS=%llu FRAMES=%llu LIVE=%ld",
            static_cast<unsigned long long>(processCalls_.load(std::memory_order_relaxed)),
            static_cast<unsigned long long>(processedFrames_.load(std::memory_order_relaxed)),
            InterlockedCompareExchange(&instanceCount, 0, 0));
        AppendTopologyLog("UNLOCK", this, detail);
        resetProcessing();
        return CBaseAudioProcessingObject::UnlockForProcess();
    }''',
    'unlock',
)

text = replace_once(
    text,
    r'''        auto* input = inputs[0];
        auto* output = outputs[0];
        const UINT32 frames = input->u32ValidFrameCount;
        if (inputBytesPerFrame_ == 0 || outputBytesPerFrame_ == 0) {''',
    r'''        auto* input = inputs[0];
        auto* output = outputs[0];
        const UINT32 frames = input->u32ValidFrameCount;
        processCalls_.fetch_add(1, std::memory_order_relaxed);
        processedFrames_.fetch_add(frames, std::memory_order_relaxed);
        if (inputBytesPerFrame_ == 0 || outputBytesPerFrame_ == 0) {''',
    'process counters',
)

text = replace_once(
    text,
    r'''    std::vector<float> silentInput_;
    bool rawBypass_ = false;
    IUnknown* outer_ = nullptr;
    RealtimeBridge realtime_;''',
    r'''    std::vector<float> silentInput_;
    std::atomic<unsigned long long> processCalls_{0};
    std::atomic<unsigned long long> processedFrames_{0};
    bool rawBypass_ = false;
    IUnknown* outer_ = nullptr;
    RealtimeBridge realtime_;''',
    'members',
)

source.write_text(text, encoding='utf-8', newline='\n')

ps = installer.read_text(encoding='utf-8').replace('\r\n', '\n')
ps = replace_once(
    ps,
    "$logPath = Join-Path $stateRoot 'install-last.log'",
    "$logPath = Join-Path $stateRoot 'install-last.log'\n$topologyLogPath = Join-Path $stateRoot 'apo-topology.log'",
    'installer log variable',
)

baseline = '& $baselineInstaller -PackageRoot $PackageRoot -AppRoot $AppRoot -AllowUnprotectedAudioDG:$AllowUnprotectedAudioDG'
trace = baseline + r'''

try {
    New-Item -ItemType Directory -Force -Path $stateRoot | Out-Null
    Set-Content -LiteralPath $topologyLogPath -Value "OMNIPHONY_APO_TOPOLOGY_TRACE`tVERSION=1" -Encoding Ascii
    & icacls.exe $stateRoot /grant '*S-1-5-19:(RX)' | Out-Null
    & icacls.exe $topologyLogPath /grant '*S-1-5-19:(M)' | Out-Null
    Write-Host "APO_TOPOLOGY_TRACE_READY PATH=$topologyLogPath"
}
catch {
    Write-Warning "Could not prepare APO topology trace: $($_.Exception.Message)"
}'''
ps = replace_once(ps, baseline, trace, 'installer baseline')
installer.write_text(ps, encoding='utf-8', newline='\n')

print('SONIC_TOPOLOGY_DIAGNOSTIC_PATCH_OK 1')
