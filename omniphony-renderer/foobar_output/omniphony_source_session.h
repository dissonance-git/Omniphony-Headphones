#pragma once

#include "../source_ffi/include/omniphony_source.h"

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum {
    OMNIPHONY_FOOBAR_SOURCE_SESSION_ABI_MAJOR = 0,
    OMNIPHONY_FOOBAR_SOURCE_SESSION_ABI_MINOR = 1,
};

/*
 * Process-local source-session seam owned by Output: Omniphony.
 *
 * Retro VGM Compiler inputs discover these exports from the already-loaded
 * foo_out_omniphony.dll. The decoder keeps source truth and causal evidence;
 * the output component owns the final FullSphere render and therefore the
 * single-render decision at the headphone boundary.
 *
 * reference_stereo is the protected 48 kHz stereo block that foobar is expected
 * to deliver to the output. The output substitutes rendered FullSphere audio
 * only when the actually delivered stereo still matches this control. A DSP,
 * resampler, or other transform between decoder and output therefore fails
 * closed to the ordinary stereo Current path instead of bypassing that transform
 * or double-spatializing a pre-rendered binaural block.
 */
__declspec(dllexport) uint32_t omniphony_foobar_source_session_abi_major(void);
__declspec(dllexport) uint32_t omniphony_foobar_source_session_abi_minor(void);
__declspec(dllexport) uint32_t omniphony_foobar_source_session_output_active(void);

__declspec(dllexport) int32_t omniphony_foobar_source_session_reset(
    uint64_t session_epoch);

__declspec(dllexport) int32_t omniphony_foobar_source_session_publish_v1(
    const OmniphonySourceConfig *config,
    uint64_t session_epoch,
    const OmniphonySourceMixBudgetV1 *mix_budget,
    const float *source_input,
    const OmniphonySourceEvidenceV1 *sources,
    size_t source_count,
    const OmniphonySourceEvidenceEventV1 *events,
    size_t event_count,
    size_t frames,
    uint64_t sample_pos,
    uint32_t ramp_frames,
    const float *reference_stereo);

#ifdef __cplusplus
}

// Internal output-callback surface. These symbols are not part of the decoder ABI.
bool omniphony_source_session_try_consume(
    const float *delivered_stereo,
    float *rendered_stereo,
    size_t frames,
    uint32_t sample_rate_hz) noexcept;
void omniphony_source_session_set_output_active(bool active) noexcept;
void omniphony_source_session_flush_output() noexcept;
#endif
