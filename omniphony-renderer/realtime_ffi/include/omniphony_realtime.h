#ifndef OMNIPHONY_REALTIME_H
#define OMNIPHONY_REALTIME_H

#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define OMNIPHONY_REALTIME_ABI_MAJOR 0
#define OMNIPHONY_REALTIME_ABI_MINOR 7

#define OMNIPHONY_REALTIME_MODE_IDENTITY 0u
#define OMNIPHONY_REALTIME_MODE_CURRENT 1u

typedef struct OmniphonyRealtimeProcessor OmniphonyRealtimeProcessor;
typedef struct OmniphonyNativeBedProcessor OmniphonyNativeBedProcessor;
typedef struct OmniphonySpatialStaticProcessor OmniphonySpatialStaticProcessor;
typedef struct OmniphonySpatialObjectProcessor OmniphonySpatialObjectProcessor;

typedef struct OmniphonyRealtimeConfig {
    uint32_t sample_rate_hz;
    uint32_t channels;
} OmniphonyRealtimeConfig;

typedef struct OmniphonyNativeBedConfig {
    uint32_t sample_rate_hz;
    uint32_t channels;
    uint32_t channel_mask;
} OmniphonyNativeBedConfig;

/*
 * Canonical Omniphony static-scene role indices. These are semantic roles,
 * not WAVEFORMATEXTENSIBLE channel bits and not AudioObjectType bit values.
 */
#define OMNIPHONY_SPATIAL_STATIC_FRONT_LEFT          0u
#define OMNIPHONY_SPATIAL_STATIC_FRONT_RIGHT         1u
#define OMNIPHONY_SPATIAL_STATIC_FRONT_CENTER        2u
#define OMNIPHONY_SPATIAL_STATIC_LOW_FREQUENCY       3u
#define OMNIPHONY_SPATIAL_STATIC_SIDE_LEFT            4u
#define OMNIPHONY_SPATIAL_STATIC_SIDE_RIGHT           5u
#define OMNIPHONY_SPATIAL_STATIC_BACK_LEFT            6u
#define OMNIPHONY_SPATIAL_STATIC_BACK_RIGHT           7u
#define OMNIPHONY_SPATIAL_STATIC_BACK_CENTER          8u
#define OMNIPHONY_SPATIAL_STATIC_TOP_FRONT_LEFT       9u
#define OMNIPHONY_SPATIAL_STATIC_TOP_FRONT_RIGHT     10u
#define OMNIPHONY_SPATIAL_STATIC_TOP_BACK_LEFT       11u
#define OMNIPHONY_SPATIAL_STATIC_TOP_BACK_RIGHT      12u
#define OMNIPHONY_SPATIAL_STATIC_BOTTOM_FRONT_LEFT   13u
#define OMNIPHONY_SPATIAL_STATIC_BOTTOM_FRONT_RIGHT  14u
#define OMNIPHONY_SPATIAL_STATIC_BOTTOM_BACK_LEFT    15u
#define OMNIPHONY_SPATIAL_STATIC_BOTTOM_BACK_RIGHT   16u

typedef struct OmniphonySpatialStaticObjectDescriptor {
    uint32_t role;
    /* Windows listener-relative coordinates in metres: +X right, +Y up, +Z behind. */
    float x_right_m;
    float y_up_m;
    float z_back_m;
} OmniphonySpatialStaticObjectDescriptor;

typedef struct OmniphonySpatialStaticConfig {
    uint32_t sample_rate_hz;
    uint32_t frames_per_quantum;
    uint32_t object_count;
    const OmniphonySpatialStaticObjectDescriptor *objects;
} OmniphonySpatialStaticConfig;

/*
 * ABI 0.6 dynamic-object metadata. `stable_id` must remain unchanged for the
 * lifetime of one Windows dynamic object and must not be reused for a later
 * allocation in the same stream. Position is Windows listener-relative XYZ:
 * +X right, +Y up, +Z behind. Position may change every update quantum and is
 * consumed continuously rather than quantized to a static speaker role.
 */
typedef struct OmniphonySpatialDynamicObjectDescriptor {
    uint64_t stable_id;
    float x_right_m;
    float y_up_m;
    float z_back_m;
} OmniphonySpatialDynamicObjectDescriptor;

typedef struct OmniphonySpatialObjectConfig {
    uint32_t sample_rate_hz;
    uint32_t frames_per_quantum;
    uint32_t static_object_count;
    const OmniphonySpatialStaticObjectDescriptor *static_objects;
    uint32_t max_dynamic_objects;
} OmniphonySpatialObjectConfig;

uint32_t omniphony_realtime_abi_major(void);
uint32_t omniphony_realtime_abi_minor(void);

OmniphonyRealtimeProcessor *omniphony_realtime_create(
    const OmniphonyRealtimeConfig *config);

void omniphony_realtime_destroy(OmniphonyRealtimeProcessor *processor);

int32_t omniphony_realtime_set_mode(
    OmniphonyRealtimeProcessor *processor,
    uint32_t mode);

uint32_t omniphony_realtime_mode(
    const OmniphonyRealtimeProcessor *processor);

uint64_t omniphony_realtime_processed_blocks(
    const OmniphonyRealtimeProcessor *processor);

/*
 * Number of stereo frames actually consumed from Current's rendered-output
 * ring. Unlike processed_blocks, this proves rendered PCM crossed back into
 * the host callback instead of remaining on the delayed-dry safety lane.
 */
uint64_t omniphony_realtime_rendered_frames(
    const OmniphonyRealtimeProcessor *processor);

/*
 * Fixed host delay, in frames, for the active processing mode. Identity is 0.
 * Current uses a bounded delayed-dry safety lane so worker underruns never turn
 * into time-shifted immediate dry audio.
 */
size_t omniphony_realtime_latency_frames(
    const OmniphonyRealtimeProcessor *processor);

int32_t omniphony_realtime_reset(OmniphonyRealtimeProcessor *processor);

/*
 * Process interleaved float32 PCM. Input/output may alias for in-place audio
 * processing. Returns 0 on success and a negative error code for invalid input.
 *
 * Mode 0 is exact identity and remains the deterministic transport oracle.
 * Mode 1 runs the retained stereo Current model on a dedicated worker thread;
 * the host callback itself only performs bounded PCM movement through
 * preallocated rings.
 *
 * Current's native spatial model owns its vertical extent. Frequency-aware,
 * sample-coherent elevation transfer occurs before the 22-direction HRTF
 * renderer; it is not a user preference and it does not create a second wet
 * copy. The Windows listening layer may independently select headphone/
 * renderer EQ and listener-specific right-channel compensation after the
 * spatial sum. Those tonal controls do not create a second renderer either.
 */
int32_t omniphony_realtime_process_f32(
    OmniphonyRealtimeProcessor *processor,
    const float *input,
    float *output,
    size_t frames);

uint32_t omniphony_realtime_sample_rate_hz(
    const OmniphonyRealtimeProcessor *processor);
uint32_t omniphony_realtime_channels(
    const OmniphonyRealtimeProcessor *processor);

/*
 * Authored Windows speaker-bed path. `channel_mask` uses WAVEFORMATEXTENSIBLE
 * speaker bits, and the interleaved input order follows those set bits from
 * least-significant to most-significant. Real speaker coordinates are rendered
 * directly through Omniphony's source-aware 22-direction binaural topology.
 * LFE is kept out of directional HRTF placement and mixed coherently after a
 * defensive low-pass. Output is always interleaved stereo float32 and must not
 * alias the multichannel input buffer.
 */
OmniphonyNativeBedProcessor *omniphony_native_bed_create(
    const OmniphonyNativeBedConfig *config);
void omniphony_native_bed_destroy(OmniphonyNativeBedProcessor *processor);
size_t omniphony_native_bed_latency_frames(
    const OmniphonyNativeBedProcessor *processor);
uint64_t omniphony_native_bed_processed_blocks(
    const OmniphonyNativeBedProcessor *processor);
int32_t omniphony_native_bed_process_f32(
    OmniphonyNativeBedProcessor *processor,
    const float *input,
    float *output_stereo,
    size_t frames);
uint32_t omniphony_native_bed_sample_rate_hz(
    const OmniphonyNativeBedProcessor *processor);
uint32_t omniphony_native_bed_channels(
    const OmniphonyNativeBedProcessor *processor);
uint32_t omniphony_native_bed_channel_mask(
    const OmniphonyNativeBedProcessor *processor);

/*
 * Fixed-topology Windows Spatial Audio static-object path retained from ABI 0.5.
 *
 * Creation receives the static role set and the exact listener-relative Windows
 * positions for that stream. The role set is immutable for the processor's
 * lifetime. Directional coordinates are authoritative source geometry; LFE is
 * explicitly non-directional and its position fields are ignored.
 *
 * The process input is planar mono float32 in descriptor order:
 *
 *   object0[frames] | object1[frames] | ...
 *
 * Output is interleaved stereo float32. Input/output must not alias. The host
 * callback only performs bounded PCM movement and safety fold-down; the source
 * renderer runs on a dedicated worker thread.
 */
OmniphonySpatialStaticProcessor *omniphony_spatial_static_create(
    const OmniphonySpatialStaticConfig *config);
void omniphony_spatial_static_destroy(
    OmniphonySpatialStaticProcessor *processor);
size_t omniphony_spatial_static_latency_frames(
    const OmniphonySpatialStaticProcessor *processor);
uint64_t omniphony_spatial_static_processed_blocks(
    const OmniphonySpatialStaticProcessor *processor);
int32_t omniphony_spatial_static_process_f32(
    OmniphonySpatialStaticProcessor *processor,
    const float *input_planar,
    float *output_stereo,
    size_t frames);
uint32_t omniphony_spatial_static_sample_rate_hz(
    const OmniphonySpatialStaticProcessor *processor);
uint32_t omniphony_spatial_static_frames_per_quantum(
    const OmniphonySpatialStaticProcessor *processor);
uint32_t omniphony_spatial_static_object_count(
    const OmniphonySpatialStaticProcessor *processor);

/*
 * Combined Windows Spatial Audio object path introduced in ABI 0.6.
 *
 * Static descriptors are fixed at creation. `max_dynamic_objects` reserves the
 * callback/worker transport capacity. Every process call supplies only the
 * currently active dynamic descriptors, in the same order as
 * `dynamic_input_planar`. A dynamic descriptor absent from a later quantum is
 * inactive; a later new object must receive a new stable_id.
 *
 * PCM layout:
 *
 *   static_input_planar:
 *     static0[frames] | static1[frames] | ...
 *
 *   dynamic_input_planar:
 *     dynamic0[frames] | dynamic1[frames] | ...
 *
 * Static and dynamic objects enter one authored source scene and one binaural
 * rendering pass. The callback performs bounded copies into preallocated
 * storage; renderer allocation remains on a dedicated worker thread.
 */
OmniphonySpatialObjectProcessor *omniphony_spatial_objects_create(
    const OmniphonySpatialObjectConfig *config);
void omniphony_spatial_objects_destroy(
    OmniphonySpatialObjectProcessor *processor);
size_t omniphony_spatial_objects_latency_frames(
    const OmniphonySpatialObjectProcessor *processor);
uint64_t omniphony_spatial_objects_processed_blocks(
    const OmniphonySpatialObjectProcessor *processor);
int32_t omniphony_spatial_objects_process_f32(
    OmniphonySpatialObjectProcessor *processor,
    const float *static_input_planar,
    const OmniphonySpatialDynamicObjectDescriptor *dynamic_objects,
    uint32_t dynamic_object_count,
    const float *dynamic_input_planar,
    float *output_stereo,
    size_t frames);

/*
 * ABI 0.7 logical stream reset. Call only from a non-realtime control path
 * while the Spatial Audio stream is stopped. Clears source-sample time,
 * dynamic identity/slot history, renderer state, transport rings, delayed
 * safety audio, headphone EQ, and output protection state while preserving the
 * stream's negotiated static role set and dynamic capacity.
 */
int32_t omniphony_spatial_objects_reset(
    OmniphonySpatialObjectProcessor *processor);
uint32_t omniphony_spatial_objects_sample_rate_hz(
    const OmniphonySpatialObjectProcessor *processor);
uint32_t omniphony_spatial_objects_frames_per_quantum(
    const OmniphonySpatialObjectProcessor *processor);
uint32_t omniphony_spatial_objects_static_object_count(
    const OmniphonySpatialObjectProcessor *processor);
uint32_t omniphony_spatial_objects_max_dynamic_objects(
    const OmniphonySpatialObjectProcessor *processor);

#ifdef __cplusplus
}
#endif

#endif
