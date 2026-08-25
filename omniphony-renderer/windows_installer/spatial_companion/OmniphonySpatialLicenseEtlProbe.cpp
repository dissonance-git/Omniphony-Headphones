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
#include <sstream>
#include <string>
#include <vector>

namespace {

constexpr GUID kLicenseProvider =
    {0x030fa765, 0xf1ae, 0x448e, {0x95, 0x5b, 0x44, 0xd7, 0x60, 0x73, 0xe1, 0xdc}};
constexpr wchar_t kAliasName[] = L"OmniphonySpatialCompanion.exe";

std::wstring HexValue(std::uint64_t value) {
    std::wostringstream out;
    out << L"0x" << std::hex << std::uppercase << value;
    return out.str();
}

std::wstring BytesToHex(const BYTE* bytes, ULONG size) {
    std::wostringstream out;
    for (ULONG i = 0; i < size; ++i) {
        if (i != 0) out << L' ';
        out << std::hex << std::uppercase << std::setw(2) << std::setfill(L'0')
            << static_cast<unsigned int>(bytes[i]);
    }
    return out.str();
}

std::wstring PropertyValueToText(USHORT inType, const BYTE* bytes, ULONG size) {
    if (bytes == nullptr || size == 0) return L"<empty>";
    switch (inType) {
    case TDH_INTYPE_UNICODESTRING: {
        const auto* text = reinterpret_cast<const wchar_t*>(bytes);
        const size_t maxChars = size / sizeof(wchar_t);
        size_t length = 0;
        while (length < maxChars && text[length] != L'\0') ++length;
        return std::wstring(text, length);
    }
    case TDH_INTYPE_ANSISTRING: {
        const auto* text = reinterpret_cast<const char*>(bytes);
        size_t length = 0;
        while (length < size && text[length] != '\0') ++length;
        if (length == 0) return {};
        const int needed = MultiByteToWideChar(CP_UTF8, 0, text, static_cast<int>(length), nullptr, 0);
        if (needed <= 0) return L"<ansi>";
        std::wstring wide(static_cast<size_t>(needed), L'\0');
        MultiByteToWideChar(CP_UTF8, 0, text, static_cast<int>(length), wide.data(), needed);
        return wide;
    }
    case TDH_INTYPE_INT8: return std::to_wstring(*reinterpret_cast<const std::int8_t*>(bytes));
    case TDH_INTYPE_UINT8: return std::to_wstring(*reinterpret_cast<const std::uint8_t*>(bytes));
    case TDH_INTYPE_INT16: if (size >= 2) return std::to_wstring(*reinterpret_cast<const std::int16_t*>(bytes)); break;
    case TDH_INTYPE_UINT16: if (size >= 2) return std::to_wstring(*reinterpret_cast<const std::uint16_t*>(bytes)); break;
    case TDH_INTYPE_INT32: if (size >= 4) return std::to_wstring(*reinterpret_cast<const std::int32_t*>(bytes)); break;
    case TDH_INTYPE_UINT32: if (size >= 4) return std::to_wstring(*reinterpret_cast<const std::uint32_t*>(bytes)); break;
    case TDH_INTYPE_INT64: if (size >= 8) return std::to_wstring(*reinterpret_cast<const std::int64_t*>(bytes)); break;
    case TDH_INTYPE_UINT64: if (size >= 8) return std::to_wstring(*reinterpret_cast<const std::uint64_t*>(bytes)); break;
    case TDH_INTYPE_HEXINT32: if (size >= 4) return HexValue(*reinterpret_cast<const std::uint32_t*>(bytes)); break;
    case TDH_INTYPE_HEXINT64: if (size >= 8) return HexValue(*reinterpret_cast<const std::uint64_t*>(bytes)); break;
    case TDH_INTYPE_BOOLEAN: if (size >= 4) return *reinterpret_cast<const std::uint32_t*>(bytes) ? L"1" : L"0"; break;
    case TDH_INTYPE_FILETIME: if (size >= 8) return std::to_wstring(*reinterpret_cast<const std::uint64_t*>(bytes)); break;
    case TDH_INTYPE_GUID:
        if (size >= sizeof(GUID)) {
            wchar_t text[64]{};
            StringFromGUID2(*reinterpret_cast<const GUID*>(bytes), text, static_cast<int>(std::size(text)));
            return text;
        }
        break;
    default: break;
    }
    return L"HEX:" + BytesToHex(bytes, size);
}

void WINAPI OnEventRecord(PEVENT_RECORD record) {
    if (record == nullptr) return;
    ULONG infoSize = 0;
    ULONG status = TdhGetEventInformation(record, 0, nullptr, nullptr, &infoSize);
    if (status != ERROR_INSUFFICIENT_BUFFER || infoSize == 0) {
        std::wcout << L"LICENSE_ETL_TDH_INFO_ERROR\t" << status << L'\n';
        return;
    }

    std::vector<BYTE> infoBuffer(infoSize);
    auto* info = reinterpret_cast<PTRACE_EVENT_INFO>(infoBuffer.data());
    status = TdhGetEventInformation(record, 0, nullptr, info, &infoSize);
    if (status != ERROR_SUCCESS) {
        std::wcout << L"LICENSE_ETL_TDH_INFO_ERROR\t" << status << L'\n';
        return;
    }

    const wchar_t* eventName = L"<unnamed>";
    if (info->EventNameOffset != 0 && info->EventNameOffset < infoSize) {
        eventName = reinterpret_cast<const wchar_t*>(infoBuffer.data() + info->EventNameOffset);
    } else if (info->TaskNameOffset != 0 && info->TaskNameOffset < infoSize) {
        eventName = reinterpret_cast<const wchar_t*>(infoBuffer.data() + info->TaskNameOffset);
    }
    std::wcout << L"LICENSE_ETL_EVENT\t" << eventName << L'\n';

    const ULONG propertyCount = std::min(info->TopLevelPropertyCount, info->PropertyCount);
    for (ULONG index = 0; index < propertyCount; ++index) {
        const auto& property = info->EventPropertyInfoArray[index];
        if ((property.Flags & PropertyStruct) != 0 || property.NameOffset == 0 || property.NameOffset >= infoSize) continue;
        const auto* propertyName = reinterpret_cast<const wchar_t*>(infoBuffer.data() + property.NameOffset);
        PROPERTY_DATA_DESCRIPTOR descriptor{};
        descriptor.PropertyName = reinterpret_cast<ULONGLONG>(propertyName);
        descriptor.ArrayIndex = ULONG_MAX;

        ULONG propertySize = 0;
        const ULONG sizeStatus = TdhGetPropertySize(record, 0, nullptr, 1, &descriptor, &propertySize);
        if (sizeStatus != ERROR_SUCCESS) {
            std::wcout << L"LICENSE_ETL_FIELD_ERROR\t" << propertyName << L"\tSIZE_STATUS=" << sizeStatus << L'\n';
            continue;
        }
        std::vector<BYTE> value(propertySize == 0 ? 1 : propertySize);
        const ULONG valueStatus = TdhGetProperty(record, 0, nullptr, 1, &descriptor, propertySize, value.data());
        if (valueStatus != ERROR_SUCCESS) {
            std::wcout << L"LICENSE_ETL_FIELD_ERROR\t" << propertyName << L"\tVALUE_STATUS=" << valueStatus << L'\n';
            continue;
        }
        std::wcout << L"LICENSE_ETL_FIELD\t" << propertyName << L"\t"
                   << PropertyValueToText(property.nonStructType.InType, value.data(), propertySize) << L'\n';
    }
}

std::wstring CurrentExecutablePath() {
    std::vector<wchar_t> buffer(32768);
    const DWORD length = GetModuleFileNameW(nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
    if (length == 0 || length >= buffer.size()) return {};
    return std::wstring(buffer.data(), length);
}

std::wstring ExecutionAliasPath() {
    DWORD required = GetEnvironmentVariableW(L"LOCALAPPDATA", nullptr, 0);
    if (required == 0) return {};
    std::wstring local(required, L'\0');
    const DWORD written = GetEnvironmentVariableW(L"LOCALAPPDATA", local.data(), required);
    if (written == 0 || written >= required) return {};
    local.resize(written);
    return local + L"\\Microsoft\\WindowsApps\\" + kAliasName;
}

std::wstring DefaultRenderEndpointId() {
    winrt::com_ptr<IMMDeviceEnumerator> enumerator;
    winrt::check_hresult(CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
                                         __uuidof(IMMDeviceEnumerator), enumerator.put_void()));
    winrt::com_ptr<IMMDevice> device;
    winrt::check_hresult(enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, device.put()));
    LPWSTR raw = nullptr;
    winrt::check_hresult(device->GetId(&raw));
    std::wstring id = raw == nullptr ? L"" : raw;
    if (raw != nullptr) CoTaskMemFree(raw);
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
    const BOOL created = CreateProcessW(alias.c_str(), mutableCommand.data(), nullptr, nullptr, TRUE, 0,
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

void CopyTextFileToConsole(const std::wstring& path) {
    std::wifstream input(path);
    std::wstring line;
    while (input && std::getline(input, line)) std::wcout << line << L'\n';
}

int CaptureChild(const std::wstring& readyName,
                 const std::wstring& stopName,
                 const std::wstring& sessionName,
                 const std::wstring& etlPath,
                 const std::wstring& statusPath) {
    HANDLE readyEvent = OpenEventW(EVENT_MODIFY_STATE, FALSE, readyName.c_str());
    HANDLE stopEvent = OpenEventW(SYNCHRONIZE, FALSE, stopName.c_str());
    if (readyEvent == nullptr || stopEvent == nullptr) return 21;

    std::wofstream statusFile(statusPath, std::ios::out | std::ios::trunc);
    if (!statusFile) return 22;

    const ULONG propertiesSize = static_cast<ULONG>(sizeof(EVENT_TRACE_PROPERTIES) +
        (sessionName.size() + 1 + etlPath.size() + 1) * sizeof(wchar_t));
    std::vector<BYTE> buffer(propertiesSize, 0);
    auto* properties = reinterpret_cast<EVENT_TRACE_PROPERTIES*>(buffer.data());
    properties->Wnode.BufferSize = propertiesSize;
    properties->Wnode.Flags = WNODE_FLAG_TRACED_GUID;
    properties->Wnode.ClientContext = 1;
    properties->LogFileMode = EVENT_TRACE_FILE_MODE_SEQUENTIAL;
    properties->MaximumFileSize = 16;
    properties->LoggerNameOffset = sizeof(EVENT_TRACE_PROPERTIES);
    properties->LogFileNameOffset = properties->LoggerNameOffset +
        static_cast<ULONG>((sessionName.size() + 1) * sizeof(wchar_t));

    auto* loggerStorage = reinterpret_cast<wchar_t*>(buffer.data() + properties->LoggerNameOffset);
    memcpy(loggerStorage, sessionName.c_str(), (sessionName.size() + 1) * sizeof(wchar_t));
    auto* fileStorage = reinterpret_cast<wchar_t*>(buffer.data() + properties->LogFileNameOffset);
    memcpy(fileStorage, etlPath.c_str(), (etlPath.size() + 1) * sizeof(wchar_t));

    TRACEHANDLE sessionHandle = 0;
    ULONG traceStatus = StartTraceW(&sessionHandle, sessionName.c_str(), properties);
    statusFile << L"LICENSE_ETL_START_STATUS\t" << traceStatus << L'\n';
    if (traceStatus != ERROR_SUCCESS) {
        statusFile.flush();
        SetEvent(readyEvent);
        CloseHandle(readyEvent);
        CloseHandle(stopEvent);
        return static_cast<int>(traceStatus);
    }

    traceStatus = EnableTraceEx2(sessionHandle, &kLicenseProvider, EVENT_CONTROL_CODE_ENABLE_PROVIDER,
                                 TRACE_LEVEL_VERBOSE, ~0ULL, 0, 0, nullptr);
    statusFile << L"LICENSE_ETL_ENABLE_STATUS\t" << traceStatus << L'\n';
    statusFile << L"LICENSE_ETL_READY\t" << (traceStatus == ERROR_SUCCESS ? 1 : 0) << L'\n';
    statusFile.flush();
    SetEvent(readyEvent);

    if (traceStatus == ERROR_SUCCESS) {
        WaitForSingleObject(stopEvent, 30000);
    }

    const ULONG stopStatus = ControlTraceW(sessionHandle, sessionName.c_str(), properties, EVENT_TRACE_CONTROL_STOP);
    statusFile << L"LICENSE_ETL_STOP_STATUS\t" << stopStatus << L'\n';
    statusFile << L"LICENSE_ETL_CAPTURE_END\t1\n";
    statusFile.flush();
    CloseHandle(readyEvent);
    CloseHandle(stopEvent);
    return traceStatus == ERROR_SUCCESS ? 0 : static_cast<int>(traceStatus);
}

bool DecodeEtl(const std::wstring& etlPath) {
    EVENT_TRACE_LOGFILEW logger{};
    logger.LogFileName = const_cast<LPWSTR>(etlPath.c_str());
    logger.ProcessTraceMode = PROCESS_TRACE_MODE_EVENT_RECORD;
    logger.EventRecordCallback = OnEventRecord;

    TRACEHANDLE trace = OpenTraceW(&logger);
    if (trace == INVALID_PROCESSTRACE_HANDLE) {
        std::wcout << L"LICENSE_ETL_DECODE_OPEN_ERROR\t" << GetLastError() << L'\n';
        return false;
    }
    const ULONG processStatus = ProcessTrace(&trace, 1, nullptr, nullptr);
    std::wcout << L"LICENSE_ETL_DECODE_PROCESS_STATUS\t" << processStatus << L'\n';
    CloseTrace(trace);
    return processStatus == ERROR_SUCCESS;
}

int ParentProbe() {
    winrt::init_apartment(winrt::apartment_type::multi_threaded);
    std::wcout << L"LICENSE_ETL_PROBE_BEGIN\t1\n";
    std::wcout << L"LICENSE_ETL_PROBE_PROVIDER_GUID\t{030FA765-F1AE-448E-955B-44D76073E1DC}\n";

    const auto endpointId = DefaultRenderEndpointId();
    std::wcout << L"LICENSE_ETL_PROBE_ENDPOINT_ID\t" << endpointId << L'\n';
    if (endpointId.empty()) return 30;

    const std::wstring suffix = std::to_wstring(GetCurrentProcessId()) + L"-" + std::to_wstring(GetTickCount64());
    const std::wstring readyName = L"Local\\OmniphonyLicenseEtlReady-" + suffix;
    const std::wstring stopName = L"Local\\OmniphonyLicenseEtlStop-" + suffix;
    const std::wstring sessionName = L"OmniphonySpatialLicenseEtl-" + suffix;
    HANDLE readyEvent = CreateEventW(nullptr, TRUE, FALSE, readyName.c_str());
    HANDLE stopEvent = CreateEventW(nullptr, TRUE, FALSE, stopName.c_str());
    if (readyEvent == nullptr || stopEvent == nullptr) return 31;

    wchar_t tempPath[MAX_PATH]{};
    if (GetTempPathW(MAX_PATH, tempPath) == 0) return 32;
    const std::wstring base = std::wstring(tempPath) + L"OmniphonyLicenseEtl-" + suffix;
    const std::wstring etlPath = base + L".etl";
    const std::wstring statusPath = base + L".txt";
    DeleteFileW(etlPath.c_str());
    DeleteFileW(statusPath.c_str());

    const auto executable = CurrentExecutablePath();
    const std::wstring parameters = L"--capture \"" + readyName + L"\" \"" + stopName +
        L"\" \"" + sessionName + L"\" \"" + etlPath + L"\" \"" + statusPath + L"\"";

    SHELLEXECUTEINFOW shell{};
    shell.cbSize = sizeof(shell);
    shell.fMask = SEE_MASK_NOCLOSEPROCESS;
    shell.lpVerb = L"runas";
    shell.lpFile = executable.c_str();
    shell.lpParameters = parameters.c_str();
    shell.nShow = SW_HIDE;

    std::wcout << L"LICENSE_ETL_PROBE_ELEVATION_REQUESTED\t1\n";
    const BOOL launched = ShellExecuteExW(&shell);
    std::wcout << L"LICENSE_ETL_PROBE_ELEVATED_CHILD_LAUNCHED\t" << (launched ? 1 : 0) << L'\n';
    if (!launched) return static_cast<int>(GetLastError());

    HANDLE waits[2] = {readyEvent, shell.hProcess};
    const DWORD readyWait = WaitForMultipleObjects(2, waits, FALSE, 15000);
    std::wcout << L"LICENSE_ETL_PROBE_READY_WAIT\t" << readyWait << L'\n';
    CopyTextFileToConsole(statusPath);
    if (readyWait != WAIT_OBJECT_0) {
        SetEvent(stopEvent);
        WaitForSingleObject(shell.hProcess, 5000);
        return 34;
    }

    Sleep(300);
    const DWORD notifyExit = RunPackagedCommand(L"notify", L"LICENSE_ETL_PROBE_NOTIFY");
    const std::wstring selectArgs = L"select \"" + endpointId + L"\"";
    const DWORD selectExit = RunPackagedCommand(selectArgs, L"LICENSE_ETL_PROBE_SELECT");
    std::wcout << L"LICENSE_ETL_PROBE_NOTIFY_EXIT\t" << notifyExit << L'\n';
    std::wcout << L"LICENSE_ETL_PROBE_SELECT_EXIT\t" << selectExit << L'\n';

    Sleep(1500);
    SetEvent(stopEvent);
    DWORD childWait = WaitForSingleObject(shell.hProcess, 10000);
    std::wcout << L"LICENSE_ETL_PROBE_CHILD_WAIT\t" << childWait << L'\n';
    DWORD childExit = STILL_ACTIVE;
    GetExitCodeProcess(shell.hProcess, &childExit);
    std::wcout << L"LICENSE_ETL_PROBE_CHILD_EXIT\t" << childExit << L'\n';
    CopyTextFileToConsole(statusPath);

    const auto fileSize = std::filesystem::exists(etlPath) ? std::filesystem::file_size(etlPath) : 0;
    std::wcout << L"LICENSE_ETL_PROBE_FILE_BYTES\t" << fileSize << L'\n';
    const bool decoded = fileSize > 0 && DecodeEtl(etlPath);
    std::wcout << L"LICENSE_ETL_PROBE_DECODED\t" << (decoded ? 1 : 0) << L'\n';

    CloseHandle(shell.hProcess);
    CloseHandle(readyEvent);
    CloseHandle(stopEvent);
    DeleteFileW(etlPath.c_str());
    DeleteFileW(statusPath.c_str());
    std::wcout << L"LICENSE_ETL_PROBE_END\t1\n";
    return static_cast<int>(selectExit);
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc == 7 && _wcsicmp(argv[1], L"--capture") == 0) {
        return CaptureChild(argv[2], argv[3], argv[4], argv[5], argv[6]);
    }
    try {
        return ParentProbe();
    } catch (const winrt::hresult_error& error) {
        std::wcerr << L"LICENSE_ETL_PROBE_HRESULT\t0x" << std::hex << std::uppercase
                   << static_cast<std::uint32_t>(error.code().value) << std::dec
                   << L"\t" << error.message().c_str() << L'\n';
    } catch (const std::exception& error) {
        std::cerr << "LICENSE_ETL_PROBE_EXCEPTION\t" << error.what() << '\n';
    }
    return 99;
}
