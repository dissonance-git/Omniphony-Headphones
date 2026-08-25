#define WIN32_LEAN_AND_MEAN
#include <windows.h>

namespace {

class ExplorerPause {
public:
    ~ExplorerPause() {
        DWORD processIds[4]{};
        const DWORD count = GetConsoleProcessList(
            processIds,
            static_cast<DWORD>(sizeof(processIds) / sizeof(processIds[0])));
        if (count > 1) {
            return;
        }

        HANDLE output = GetStdHandle(STD_OUTPUT_HANDLE);
        HANDLE input = GetStdHandle(STD_INPUT_HANDLE);
        if (output == nullptr || output == INVALID_HANDLE_VALUE ||
            input == nullptr || input == INVALID_HANDLE_VALUE) {
            return;
        }

        constexpr wchar_t message[] = L"\r\nPress Enter to close...";
        DWORD written = 0;
        WriteConsoleW(
            output,
            message,
            static_cast<DWORD>((sizeof(message) / sizeof(message[0])) - 1),
            &written,
            nullptr);

        wchar_t buffer[4]{};
        DWORD read = 0;
        ReadConsoleW(input, buffer, static_cast<DWORD>(sizeof(buffer) / sizeof(buffer[0])), &read, nullptr);
    }
};

ExplorerPause g_explorerPause;

} // namespace
