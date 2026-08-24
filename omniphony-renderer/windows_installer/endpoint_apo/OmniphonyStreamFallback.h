#pragma once

#include <windows.h>
#include <ksmedia.h>

#include <algorithm>
#include <cmath>
#include <cstddef>

namespace omniphony {

inline void SafetyFoldDown(
    const float* input,
    float* output,
    size_t frames,
    UINT32 channels,
    DWORD channelMask) noexcept {
    if (!input || !output || channels == 0) return;

    for (size_t frame = 0; frame < frames; ++frame) {
        float left = 0.0f;
        float right = 0.0f;
        UINT32 channel = 0;
        for (DWORD bit = 1; bit != 0 && channel < channels; bit <<= 1) {
            if ((channelMask & bit) == 0) continue;
            const float value = input[frame * channels + channel++];
            const float sample = std::isfinite(value) ? value : 0.0f;
            switch (bit) {
            case SPEAKER_FRONT_LEFT:
                left += sample;
                break;
            case SPEAKER_FRONT_RIGHT:
                right += sample;
                break;
            case SPEAKER_FRONT_CENTER:
                left += sample * 0.70710677f;
                right += sample * 0.70710677f;
                break;
            case SPEAKER_LOW_FREQUENCY:
                left += sample * 0.5f;
                right += sample * 0.5f;
                break;
            case SPEAKER_BACK_LEFT:
            case SPEAKER_SIDE_LEFT:
                left += sample * 0.70710677f;
                break;
            case SPEAKER_BACK_RIGHT:
            case SPEAKER_SIDE_RIGHT:
                right += sample * 0.70710677f;
                break;
            case SPEAKER_BACK_CENTER:
                left += sample * 0.5f;
                right += sample * 0.5f;
                break;
            case SPEAKER_FRONT_LEFT_OF_CENTER:
            case SPEAKER_TOP_FRONT_LEFT:
            case SPEAKER_TOP_BACK_LEFT:
                left += sample * 0.5f;
                break;
            case SPEAKER_FRONT_RIGHT_OF_CENTER:
            case SPEAKER_TOP_FRONT_RIGHT:
            case SPEAKER_TOP_BACK_RIGHT:
                right += sample * 0.5f;
                break;
            case SPEAKER_TOP_CENTER:
            case SPEAKER_TOP_FRONT_CENTER:
            case SPEAKER_TOP_BACK_CENTER:
                left += sample * 0.35355338f;
                right += sample * 0.35355338f;
                break;
            default:
                left += sample * 0.35355338f;
                right += sample * 0.35355338f;
                break;
            }
        }
        while (channel < channels) {
            const float value = input[frame * channels + channel++];
            const float sample = std::isfinite(value) ? value : 0.0f;
            left += sample * 0.35355338f;
            right += sample * 0.35355338f;
        }

        const float peak = (std::max)(std::abs(left), std::abs(right));
        if (peak > 1.0f) {
            left /= peak;
            right /= peak;
        }
        output[frame * 2] = left;
        output[frame * 2 + 1] = right;
    }
}

} // namespace omniphony
