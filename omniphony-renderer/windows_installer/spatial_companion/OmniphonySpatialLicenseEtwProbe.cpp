#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shellapi.h>
#include <evntrace.h>
#include <evntcons.h>
#include <tdh.h>
#include <mmdeviceapi.h>

#include <winrt/base.h>

#include <algorithm>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <mutex>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

namespace {

constexpr GUID kLicenseProvider =
    {0x030fa765, 0xf1ae, 0x448e, {0x95, 0x5b, 0x44, 0xd7, 0x60, 0x73, 0xe1, 0xdc}};
constexpr wchar_t kAliasName[] = L"OmniphonySpatialCompanion.exe";

struct TraceOutput {
    std::wofstream stream;
    std::mutex mutex;
};

std::wstring HexValue(std::uint64_t value) {
    std::wostringstream out;
    out << L"0x" << std::hex << std::uppercase << value;
    return out.str();
}

std::wstring BytesToHex(const BYTE* bytes, ULONG size) {
    std::wostringstream out;
    for (ULONG i = 0; i < size; ++i) {
        if (i != 0) {
            out << L' ';
        }
        out << std::hex << std::uppercase << std::setw(2) << std::setfill(L'0')
            << static_cast<unsigned int>(bytes[i]);
    }
    return out.str();
}

std::wstring PropertyValueToText(USHORT inType, const BYTE* bytes, ULONG size) {
    if (bytes == nullptr || size == 0) {
        return L"<empty>";
    }

    switch (inType) {
    case TDH_INTYPE_UNICODESTRING: {
        const auto* text = reinterpret_cast<const wchar_t*>(bytes);
        const size_t maxChars = size / sizeof(wchar_t);
        size_t length = 0;
        while (length < maxChars && text[length] != L'\0') {
            ++length;
        }
        return std::wstring(text, length);
    }
    case TDH_INTYPE_ANSISTRING: {
        const auto* text = reinterpret_cast<const char*>(bytes);
        size_t length = 0;
        while (length < size && text[length] != '\0') {
            ++length;
        }
        if (length == 0) {
            return {};
        }
        const int needed = MultiByteToWideChar(CP_UTF8, 0, text, static_cast<int>(length), nullptr, 0);
        if (needed <= 0) {
            return L"<ansi>";
        }
        std::wstring wide(static_cast<size_t>(needed), L'\0');
        MultiByteToWideChar(CP_UTF8, 0, text, static_cast<int>(length), wide.data(), needed);
        return wide;
    }
    case TDH_INTYPE_INT8:
        return std::to_wstring(*reinterpret_cast<const std::int8_t*>(bytes));
    case TDH_INTYPE_UINT8:
        return std::to_wstring(*reinterpret_cast<const std::uint8_t*>(bytes));
    case TDH_INTYPE_INT16:
        if (size >= sizeof(std::int16_t)) return std::to_wstring(*reinterpret_cast<const std::int16_t*>(bytes));
        break;
    case TDH_INTYPE_UINT16:
        if (size >= sizeof(std::uint16_t)) return std::to_wstring(*reinterpret_cast<const std::uint16_t*>(bytes));
        break;
    case TDH_INTYPE_INT32:
        if (size >= sizeof(std::int32_t)) return std::to_wstring(*reinterpret_cast<const std::int32_t*>(bytes));
        break;
    case TDH_INTYPE_UINT32:
        if (size >= sizeof(std::uint32_t)) return std::to_wstring(*reinterpret_cast<const std::uint32_t*>(bytes));
        break;
    case TDH_INTYPE_INT64:
        if (size >= sizeof(std::int64_t)) return std::to_wstring(*reinterpret_cast<const std::int64_t*>(bytes));
        break;
    case TDH_INTYPE_UINT64:
        if (size >= sizeof(std::uint64_t)) return std::to_wstring(*reinterpret_cast<const std::uint64_t*>(bytes));
        break;
    case TDH_INTYPE_HEXINT32:
        if (size >= sizeof(std::uint32_t)) return HexValue(*reinterpret_cast<const std::uint32_t*>(bytes));
        break;
    case TDH_INTYPE_HEXINT64:
        if (size >= sizeof(std::uint64_t)) return HexValue(*reinterpret_cast<const std::uint64_t*>(bytes));
        break;
    case TDH_INTYPE_BOOLEAN:
        if (size >= sizeof(std::uint32_t)) return (*reinterpret_cast<const std::uint32_t*>(bytes) != 0) ? L"1" : L"0";
        break;
    case TDH_INTYPE_FILETIME:
        if (size >= sizeof(std::uint64_t)) return std::to_wstring(*reinterpret_cast<const std::uint64_t*>(bytes));
        break;
    case TDH_INTYPE_GUID:
        if (size >= sizeof(GUID)) {
            wchar_t guidText[64]{};
            StringFromGUID2(*reinterpret_cast<const GUID*>(bytes), guidText, static_cast<int>(std::size(guidText)));
            return guidText;
        }
        break;
    default:
        break;
    }

    return L"HEX:" + BytesToHex(bytes, size);
}

void WINAPI OnEventRecord(PEVENT_RECORD record) {
    if (record == nullptr || record->UserContext == nullptr) {
        return;
    }
    auto* output = static_cast<TraceOutput*>(record->UserContext);

    ULONG infoSize = 0;
    ULONG status = TdhGetEventInformation(record, 0, nullptr, nullptr, &infoSize);
    if (status != ERROR_INSUFFICIENT_BUFFER || infoSize == 0) {
        std::lock_guard<std::mutex> lock(output->mutex);
        output->stream << L"LICENSE_ETW_TDH_INFO_ERROR\t" << status << L'\n';
        output->stream.flush();
        return;
    }

    std::vector<BYTE> infoBuffer(infoSize);
    auto* info = reinterpret_cast<PTRACE_EVENT_INFO>(infoBuffer.data());
    status = TdhGetEventInformation(record, 0, nullptr, info, &infoSize);
    if (status != ERROR_SUCCESS) {
        std::lock_guard<std::mutex> lock(output->mutex);
        output->stream << L"LICENSE_ETW_TDH_INFO_ERROR\t" << status << L'\n';
        output->stream.flush();
        return;
    }

    const wchar_t* eventName = L"<unnamed>";
    if (info->EventNameOffset != 0 && info->EventNameOffset < infoSize) {
        eventName = reinterpret_cast<const wchar_t*>(infoBuffer.data() + info->EventNameOffset);
    } else if (info->TaskNameOffset != 0 && info->TaskNameOffset < infoSize) {
        eventName = reinterpret_cast<const wchar_t*>(infoBuffer.data() + info->TaskNameOffset);
    }

    std::lock_guard<std::mutex> lock(output->mutex);
    output->stream << L"LICENSE_ETW_EVENT\t" << eventName << L'\n';

    const ULONG propertyCount = std::min(info->TopLevelPropertyCount, info->PropertyCount);
    for (ULONG index = 0; index < propertyCount; ++index) {
        const auto& property = info->EventPropertyInfoArray[index];
        if ((property.Flags & PropertyStruct) != 0 ||
            property.NameOffset == 0 || property.NameOffset >= infoSize) {
            continue;
        }

        const auto* propertyName = reinterpret_cast<const wchar_t*>(infoBuffer.data() + property.NameOffset);
        PROPERTY_DATA_DESCRIPTOR descriptor{};
        descriptor.PropertyName = reinterpret_cast<ULONGLONG>(propertyName);
        descriptor.ArrayIndex = ULONG_MAX;

        ULONG propertySize = 0;
        const ULONG sizeStatus = TdhGetPropertySize(record, 0, nullptr, 1, &descriptor, &propertySize);
        if (sizeStatus != ERROR_SUCCESS) {
            output->stream << L"LICENSE_ETW_FIELD_ERROR\t" << propertyName
                           << L"\tSIZE_STATUS=" << sizeStatus << L'\n';
            continue;
        }

        std::vector<BYTE> value(propertySize == 0 ? 1 : propertySize);
        const ULONG valueStatus = TdhGetProperty(
            record, 0, nullptr, 1, &descriptor, propertySize, value.data());
        if (valueStatus != ERROR_SUCCESS) {
            output->stream << L"LICENSE_ETW_FIELD_ERROR\t" << propertyName
                           << L"\tVALUE_STATUS=" << valueStatus << L'\n';
            continue;
        }

        output->stream << L"LICENSE_ETW_FIELD\t" << propertyName << L"\t"
                       << PropertyValueToText(property.nonStructType.InType, value.data(), propertySize)
                       << L'\n';
    }
    output->stream.flush();
}

std::wstring CurrentExecutablePath() {
    std::vector<wchar_t> buffer(32768);
    const DWORD length = GetModuleFileNameW(nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
    if (length == 0 || length >= buffer.size()) {
        return {};
    }
    return std::wstring(buffer.data(), length);
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

std::wstring DefaultRenderEndpointId() {
    winrt::com_ptr<IMMDeviceEnumerator> enumerator;
    winrt::check_hresult(CoCreateInstance(
        __uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
        __uuidof(IMMDeviceEnumerator), enumerator.put_void()));

    winrt::com_ptr<IMMDevice> device;
    winrt::check_hresult(enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, device.put()));

    LPWSTR raw = nullptr;
    winrt::check_hresult(device->GetId(&raw));
    std::wstring id = raw == nullptr ? L"" : raw;
    if (raw != nullptr) {
        CoTaskMemFree(raw);
    }
    return id;
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

int CaptureChild(const std::wstring& readyName,
                 const std::wstring& stopName,
                 const std::wstring& outputPath) {
    HANDLE readyEvent = OpenEventW(EVENT_MODIFY_STATE, FALSE, readyName.c_str());
    HANDLE stopEvent = OpenEventW(SYNCHRONIZE, FALSE, stopName.c_str());
    if (readyEvent == nullptr || stopEvent == nullptr) {
        if (readyEvent != nullptr) CloseHandle(readyEvent);
        if (stopEvent != nullptr) CloseHandle(stopEvent);
        return 21;
    }

    TraceOutput output;
    output.stream.open(outputPath, std::ios::out | std::ios::trunc);
    if (!output.stream) {
        CloseHandle(readyEvent);
        CloseHandle(stopEvent);
        return 22;
    }

    const std::wstring sessionName = L"OmniphonySpatialLicense-" + std::to_wstring(GetCurrentProcessId());
    const ULONG propertiesSize = static_cast<ULONG>(
        sizeof(EVENT_TRACE_PROPERTIES) + (sessionName.size() + 1) * sizeof(wchar_t));
    std::vector<BYTE> propertiesBuffer(propertiesSize, 0);
    auto* properties = reinterpret_cast<EVENT_TRACE_PROPERTIES*>(propertiesBuffer.data());
    properties->Wnode.BufferSize = propertiesSize;
    properties->Wnode.Flags = WNODE_FLAG_TRACED_GUID;
    properties->Wnode.ClientContext = 1;
    properties->LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
    properties->FlushTimer = 1;
    properties->LoggerNameOffset = sizeof(EVENT_TRACE_PROPERTIES);
    auto* nameStorage = reinterpret_cast<wchar_t*>(propertiesBuffer.data() + properties->LoggerNameOffset);
    memcpy(nameStorage, sessionName.c_str(), (sessionName.size() + 1) * sizeof(wchar_t));

    TRACEHANDLE sessionHandle = 0;
    ULONG status = StartTraceW(&sessionHandle, sessionName.c_str(), properties);
    output.stream << L"LICENSE_ETW_START_STATUS\t" << status << L'\n';
    if (status != ERROR_SUCCESS) {
        output.stream.flush();
        SetEvent(readyEvent);
        CloseHandle(readyEvent);
        CloseHandle(stopEvent);
        return static_cast<int>(status);
    }

    status = EnableTraceEx2(
        sessionHandle,
        &kLicenseProvider,
        EVENT_CONTROL_CODE_ENABLE_PROVIDER,
        TRACE_LEVEL_VERBOSE,
        ~0ULL,
        0,
        0,
        nullptr);
    output.stream << L"LICENSE_ETW_ENABLE_STATUS\t" << status << L'\n';
    if (status != ERROR_SUCCESS) {
        ControlTraceW(sessionHandle, sessionName.c_str(), properties, EVENT_TRACE_CONTROL_STOP);
        output.stream.flush();
        SetEvent(readyEvent);
        CloseHandle(readyEvent);
        CloseHandle(stopEvent);
        return static_cast<int>(status);
    }

    EVENT_TRACE_LOGFILEW logger{};
    logger.LoggerName = const_cast<LPWSTR>(sessionName.c_str());
    logger.ProcessTraceMode = PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
    logger.EventRecordCallback = OnEventRecord;
    logger.Context = &output;

    TRACEHANDLE traceHandle = OpenTraceW(&logger);
    if (traceHandle == INVALID_PROCESSTRACE_HANDLE) {
        const DWORD error = GetLastError();
        output.stream << L"LICENSE_ETW_OPEN_STATUS\t" << error << L'\n';
        ControlTraceW(sessionHandle, sessionName.c_str(), properties, EVENT_TRACE_CONTROL_STOP);
        output.stream.flush();
        SetEvent(readyEvent);
        CloseHandle(readyEvent);
        CloseHandle(stopEvent);
        return static_cast<int>(error);
    }

    output.stream << L"LICENSE_ETW_OPEN_STATUS\t0\n";
    output.stream << L"LICENSE_ETW_READY\t1\n";
    output.stream.flush();

    std::thread processingThread([&]() {
        const ULONG processStatus = ProcessTrace(&traceHandle, 1, nullptr, nullptr);
        std::lock_guard<std::mutex> lock(output.mutex);
        output.stream << L"LICENSE_ETW_PROCESS_STATUS\t" << processStatus << L'\n';
        output.stream.flush();
    });

    SetEvent(readyEvent);
    WaitForSingleObject(stopEvent, 30000);

    ControlTraceW(sessionHandle, sessionName.c_str(), properties, EVENT_TRACE_CONTROL_STOP);
    CloseTrace(traceHandle);
    if (processingThread.joinable()) {
        processingThread.join();
    }

    output.stream << L"LICENSE_ETW_CAPTURE_END\t1\n";
    output.stream.flush();
    CloseHandle(readyEvent);
    CloseHandle(stopEvent);
    return 0;
}

bool CopyTraceOutputToConsole(const std::wstring& outputPath) {
    std::wifstream input(outputPath);
    if (!input) {
        return false;
    }
    std::wstring line;
    while (std::getline(input, line)) {
        std::wcout << line << L'\n';
    }
    return true;
}

int ParentProbe() {
    winrt::init_apartment(winrt::apartment_type::multi_threaded);
    std::wcout << L"LICENSE_PROBE_BEGIN\t1\n";
    std::wcout << L"LICENSE_PROBE_PROVIDER_GUID\t{030FA765-F1AE-448E-955B-44D76073E1DC}\n";

    const auto endpointId = DefaultRenderEndpointId();
    std::wcout << L"LICENSE_PROBE_ENDPOINT_ID\t" << endpointId << L'\n';
    if (endpointId.empty()) {
        std::wcout << L"LICENSE_PROBE_ENDPOINT_AVAILABLE\t0\n";
        return 30;
    }
    std::wcout << L"LICENSE_PROBE_ENDPOINT_AVAILABLE\t1\n";

    const DWORD parentPid = GetCurrentProcessId();
    const std::wstring suffix = std::to_wstring(parentPid) + L"-" + std::to_wstring(GetTickCount64());
    const std::wstring readyName = L"Local\\OmniphonyLicenseReady-" + suffix;
    const std::wstring stopName = L"Local\\OmniphonyLicenseStop-" + suffix;

    HANDLE readyEvent = CreateEventW(nullptr, TRUE, FALSE, readyName.c_str());
    HANDLE stopEvent = CreateEventW(nullptr, TRUE, FALSE, stopName.c_str());
    if (readyEvent == nullptr || stopEvent == nullptr) {
        if (readyEvent != nullptr) CloseHandle(readyEvent);
        if (stopEvent != nullptr) CloseHandle(stopEvent);
        std::wcout << L"LICENSE_PROBE_EVENT_CREATE_ERROR\t" << GetLastError() << L'\n';
        return 31;
    }

    wchar_t tempPathBuffer[MAX_PATH]{};
    if (GetTempPathW(MAX_PATH, tempPathBuffer) == 0) {
        CloseHandle(readyEvent);
        CloseHandle(stopEvent);
        return 32;
    }
    const std::wstring outputPath = std::wstring(tempPathBuffer) + L"OmniphonyLicenseEtw-" + suffix + L".txt";
    DeleteFileW(outputPath.c_str());

    const auto executable = CurrentExecutablePath();
    if (executable.empty()) {
        CloseHandle(readyEvent);
        CloseHandle(stopEvent);
        return 33;
    }

    const std::wstring parameters =
        L"--capture \"" + readyName + L"\" \"" + stopName + L"\" \"" + outputPath + L"\"";

    SHELLEXECUTEINFOW shell{};
    shell.cbSize = sizeof(shell);
    shell.fMask = SEE_MASK_NOCLOSEPROCESS;
    shell.lpVerb = L"runas";
    shell.lpFile = executable.c_str();
    shell.lpParameters = parameters.c_str();
    shell.nShow = SW_HIDE;

    std::wcout << L"LICENSE_PROBE_ETW_ELEVATION_REQUESTED\t1\n";
    const BOOL launched = ShellExecuteExW(&shell);
    std::wcout << L"LICENSE_PROBE_ETW_ELEVATED_CHILD_LAUNCHED\t" << (launched ? 1 : 0) << L'\n';
    if (!launched) {
        const DWORD error = GetLastError();
        std::wcout << L"LICENSE_PROBE_ETW_ELEVATION_ERROR\t" << error << L'\n';
        CloseHandle(readyEvent);
        CloseHandle(stopEvent);
        return static_cast<int>(error);
    }

    HANDLE waitHandles[2] = {readyEvent, shell.hProcess};
    const DWORD readyWait = WaitForMultipleObjects(2, waitHandles, FALSE, 15000);
    std::wcout << L"LICENSE_PROBE_ETW_READY_WAIT\t" << readyWait << L'\n';
    if (readyWait != WAIT_OBJECT_0) {
        SetEvent(stopEvent);
        WaitForSingleObject(shell.hProcess, 5000);
        CloseHandle(shell.hProcess);
        CloseHandle(readyEvent);
        CloseHandle(stopEvent);
        CopyTraceOutputToConsole(outputPath);
        DeleteFileW(outputPath.c_str());
        return 34;
    }

    Sleep(300);
    const DWORD notifyExit = RunPackagedCommand(L"notify", L"LICENSE_PROBE_NOTIFY");
    const std::wstring selectArgs = L"select \"" + endpointId + L"\"";
    const DWORD selectExit = RunPackagedCommand(selectArgs, L"LICENSE_PROBE_SELECT");
    std::wcout << L"LICENSE_PROBE_NOTIFY_EXIT\t" << notifyExit << L'\n';
    std::wcout << L"LICENSE_PROBE_SELECT_EXIT\t" << selectExit << L'\n';

    Sleep(1500);
    SetEvent(stopEvent);
    WaitForSingleObject(shell.hProcess, 10000);
    DWORD childExit = STILL_ACTIVE;
    GetExitCodeProcess(shell.hProcess, &childExit);
    std::wcout << L"LICENSE_PROBE_ETW_CHILD_EXIT\t" << childExit << L'\n';

    CloseHandle(shell.hProcess);
    CloseHandle(readyEvent);
    CloseHandle(stopEvent);

    const bool copied = CopyTraceOutputToConsole(outputPath);
    std::wcout << L"LICENSE_PROBE_ETW_OUTPUT_READ\t" << (copied ? 1 : 0) << L'\n';
    DeleteFileW(outputPath.c_str());
    std::wcout << L"LICENSE_PROBE_END\t1\n";
    return static_cast<int>(selectExit);
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc == 5 && _wcsicmp(argv[1], L"--capture") == 0) {
        return CaptureChild(argv[2], argv[3], argv[4]);
    }

    try {
        return ParentProbe();
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"LICENSE_PROBE_HRESULT\t0x" << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(error.code().value) << std::dec
                   << L"\t" << error.message().c_str() << L'\n';
    } catch (const std::exception& error) {
        std::cerr << "LICENSE_PROBE_EXCEPTION\t" << error.what() << '\n';
    }
    return 99;
}
