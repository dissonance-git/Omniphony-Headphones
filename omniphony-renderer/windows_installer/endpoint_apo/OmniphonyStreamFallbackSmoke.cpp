#include "OmniphonyStreamFallback.h"

#include <cmath>
#include <iostream>
#include <limits>

int wmain() {
    constexpr DWORD mask =
        SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT | SPEAKER_FRONT_CENTER |
        SPEAKER_LOW_FREQUENCY | SPEAKER_BACK_LEFT | SPEAKER_BACK_RIGHT |
        SPEAKER_SIDE_LEFT | SPEAKER_SIDE_RIGHT;

    const float input[] = {
        0.25f, -0.125f, 0.2f, 0.1f, 0.05f, -0.05f, 0.04f, -0.04f,
        std::numeric_limits<float>::quiet_NaN(), 0.0f, 0.0f, 0.0f,
        0.0f, 0.0f, 0.0f, 0.0f,
        4.0f, 4.0f, 4.0f, 4.0f, 4.0f, 4.0f, 4.0f, 4.0f,
    };
    float output[6] = {};
    omniphony::SafetyFoldDown(input, output, 3, 8, mask);

    if (!(output[0] > output[1]) || !std::isfinite(output[0]) || !std::isfinite(output[1])) {
        std::wcerr << L"STREAM_FALLBACK_FAIL stage=directional_identity\n";
        return 1;
    }
    if (output[2] != 0.0f || output[3] != 0.0f) {
        std::wcerr << L"STREAM_FALLBACK_FAIL stage=nonfinite_sanitization\n";
        return 2;
    }
    if ((std::max)(std::abs(output[4]), std::abs(output[5])) > 1.0f) {
        std::wcerr << L"STREAM_FALLBACK_FAIL stage=peak_normalization\n";
        return 3;
    }

    std::wcout << L"STREAM_FALLBACK_OK CHANNELS=8 FRAMES=3 FINITE=1 PEAK_SAFE=1\n";
    return 0;
}
