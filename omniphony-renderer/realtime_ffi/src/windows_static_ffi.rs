//! Compatibility C ABI for fixed Windows Spatial Audio static-object streams.
//!
//! The legacy static entry points remain available for host compatibility, but
//! they no longer own a second renderer, worker, ring buffer, fallback mixer, or
//! lifecycle implementation. They adapt the static descriptor shape into the
//! unified static+dynamic object processor with dynamic capacity set to zero.

use crate::windows_objects_ffi::{
    OmniphonySpatialObjectProcessor, OmniphonySpatialObjectStaticDescriptor,
};
use std::ptr;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OmniphonySpatialStaticObjectDescriptor {
    /// Canonical Omniphony static-scene index, 0..=16.
    pub role: u32,
    /// Windows listener-relative Cartesian position in metres.
    /// Ignored for LFE; required and finite for every directional role.
    pub x_right_m: f32,
    pub y_up_m: f32,
    pub z_back_m: f32,
}

#[repr(C)]
pub struct OmniphonySpatialStaticConfig {
    pub sample_rate_hz: u32,
    pub frames_per_quantum: u32,
    pub object_count: u32,
    pub objects: *const OmniphonySpatialStaticObjectDescriptor,
}

fn unified_descriptors(
    input: &[OmniphonySpatialStaticObjectDescriptor],
) -> Vec<OmniphonySpatialObjectStaticDescriptor> {
    input
        .iter()
        .map(|descriptor| OmniphonySpatialObjectStaticDescriptor {
            role: descriptor.role,
            x_right_m: descriptor.x_right_m,
            y_up_m: descriptor.y_up_m,
            z_back_m: descriptor.z_back_m,
        })
        .collect()
}

pub struct OmniphonySpatialStaticProcessor {
    sample_rate_hz: u32,
    frames_per_quantum: u32,
    object_count: u32,
    inner: OmniphonySpatialObjectProcessor,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_create(
    config: *const OmniphonySpatialStaticConfig,
) -> *mut OmniphonySpatialStaticProcessor {
    crate::ffi_guard(ptr::null_mut(), || {
        if config.is_null() {
            return ptr::null_mut();
        }
        let config = unsafe { &*config };
        if config.sample_rate_hz == 0
            || config.frames_per_quantum == 0
            || config.object_count == 0
            || config.object_count > 17
            || config.objects.is_null()
        {
            return ptr::null_mut();
        }

        let input = unsafe {
            std::slice::from_raw_parts(config.objects, config.object_count as usize)
        };
        let descriptors = unified_descriptors(input);
        let Ok(inner) = OmniphonySpatialObjectProcessor::new_static_only(
            config.sample_rate_hz,
            config.frames_per_quantum,
            &descriptors,
        ) else {
            return ptr::null_mut();
        };

        Box::into_raw(Box::new(OmniphonySpatialStaticProcessor {
            sample_rate_hz: config.sample_rate_hz,
            frames_per_quantum: config.frames_per_quantum,
            object_count: config.object_count,
            inner,
        }))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_destroy(
    processor: *mut OmniphonySpatialStaticProcessor,
) {
    crate::ffi_guard((), || {
        if !processor.is_null() {
            unsafe { drop(Box::from_raw(processor)) };
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_latency_frames(
    processor: *const OmniphonySpatialStaticProcessor,
) -> usize {
    if processor.is_null() {
        0
    } else {
        unsafe { (*processor).inner.latency_frames() }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_processed_blocks(
    processor: *const OmniphonySpatialStaticProcessor,
) -> u64 {
    if processor.is_null() {
        0
    } else {
        unsafe { (*processor).inner.processed_blocks() }
    }
}

/// Process one fixed-topology Spatial Audio update quantum.
///
/// Input is planar mono float32 in the exact descriptor order supplied at
/// creation: `object0[frames] | object1[frames] | ...`. Output is interleaved
/// stereo float32. Input and output must not alias.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_process_f32(
    processor: *mut OmniphonySpatialStaticProcessor,
    input_planar: *const f32,
    output_stereo: *mut f32,
    frames: usize,
) -> i32 {
    crate::ffi_guard(-127, || {
        if processor.is_null() {
            return -1;
        }
        if frames == 0 {
            return 0;
        }
        if input_planar.is_null() || output_stereo.is_null() {
            return -2;
        }
        let processor = unsafe { &mut *processor };
        unsafe {
            processor
                .inner
                .process_static_only(input_planar, output_stereo, frames)
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_sample_rate_hz(
    processor: *const OmniphonySpatialStaticProcessor,
) -> u32 {
    if processor.is_null() {
        0
    } else {
        unsafe { (*processor).sample_rate_hz }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_frames_per_quantum(
    processor: *const OmniphonySpatialStaticProcessor,
) -> u32 {
    if processor.is_null() {
        0
    } else {
        unsafe { (*processor).frames_per_quantum }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_object_count(
    processor: *const OmniphonySpatialStaticProcessor,
) -> u32 {
    if processor.is_null() {
        0
    } else {
        unsafe { (*processor).object_count }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_descriptor_conversion_is_lossless() {
        let input = [
            OmniphonySpatialStaticObjectDescriptor {
                role: 0,
                x_right_m: -0.7,
                y_up_m: 0.0,
                z_back_m: -0.7,
            },
            OmniphonySpatialStaticObjectDescriptor {
                role: 3,
                x_right_m: f32::NAN,
                y_up_m: f32::INFINITY,
                z_back_m: f32::NEG_INFINITY,
            },
        ];
        let converted = unified_descriptors(&input);
        assert_eq!(converted.len(), input.len());
        assert_eq!(converted[0].role, input[0].role);
        assert_eq!(converted[0].x_right_m, input[0].x_right_m);
        assert_eq!(converted[1].role, input[1].role);
        assert!(converted[1].x_right_m.is_nan());
    }

    #[test]
    fn invalid_static_metadata_is_rejected_by_the_unified_owner() {
        let duplicate = [
            OmniphonySpatialObjectStaticDescriptor {
                role: 0,
                x_right_m: -0.7,
                y_up_m: 0.0,
                z_back_m: -0.7,
            },
            OmniphonySpatialObjectStaticDescriptor {
                role: 0,
                x_right_m: -0.7,
                y_up_m: 0.0,
                z_back_m: -0.7,
            },
        ];
        assert!(
            OmniphonySpatialObjectProcessor::new_static_only(48_000, 480, &duplicate).is_err()
        );

        let bad_role = [OmniphonySpatialObjectStaticDescriptor {
            role: 17,
            x_right_m: 0.0,
            y_up_m: 0.0,
            z_back_m: -1.0,
        }];
        assert!(
            OmniphonySpatialObjectProcessor::new_static_only(48_000, 480, &bad_role).is_err()
        );
    }
}
