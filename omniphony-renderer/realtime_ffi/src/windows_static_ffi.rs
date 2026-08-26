//! Realtime C ABI for a fixed Windows Spatial Audio static-object stream.
//!
//! The Windows-facing callback never runs the allocating source renderer. It
//! copies one fixed-topology planar object quantum into a preallocated ring and
//! reads delayed stereo from another ring. A dedicated worker owns
//! `WindowsStaticObjectPipeline` and therefore the existing Omniphony renderer.

use crate::windows_objects_scene::WindowsSpatialObjectPipeline;
use crate::windows_spatial_contract::{
    WindowsSpatialPosition, WindowsStaticObject, WindowsStaticObjectRole,
};
use crate::{AudioRing, OUTPUT_CEILING, RING_SECONDS, StereoDelay};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc::sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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

#[derive(Clone, Copy, Debug)]
struct StaticDescriptor {
    role: WindowsStaticObjectRole,
    position: Option<WindowsSpatialPosition>,
}

fn copy_descriptors(
    input: &[OmniphonySpatialStaticObjectDescriptor],
) -> Result<Vec<StaticDescriptor>, String> {
    if input.is_empty() || input.len() > 17 {
        return Err("static object count must be 1..=17".to_string());
    }

    let mut seen = [false; 17];
    let mut out = Vec::with_capacity(input.len());
    for descriptor in input {
        let role = WindowsStaticObjectRole::from_canonical_scene_index(descriptor.role)
            .ok_or_else(|| format!("invalid static object role {}", descriptor.role))?;
        let index = role.canonical_scene_index();
        if seen[index] {
            return Err(format!("duplicate static object role {role:?}"));
        }
        seen[index] = true;

        let position = if role == WindowsStaticObjectRole::LowFrequency {
            None
        } else {
            if !descriptor.x_right_m.is_finite()
                || !descriptor.y_up_m.is_finite()
                || !descriptor.z_back_m.is_finite()
            {
                return Err(format!("non-finite static object position for {role:?}"));
            }
            Some(WindowsSpatialPosition::new(
                descriptor.x_right_m,
                descriptor.y_up_m,
                descriptor.z_back_m,
            ))
        };
        out.push(StaticDescriptor { role, position });
    }
    Ok(out)
}

fn safety_downmix_frame(
    descriptors: &[StaticDescriptor],
    planar: *const f32,
    frames: usize,
    frame_index: usize,
) -> [f32; 2] {
    let mut left = 0.0f32;
    let mut right = 0.0f32;

    for (object_index, descriptor) in descriptors.iter().enumerate() {
        let sample = unsafe { *planar.add(object_index * frames + frame_index) };
        let sample = if sample.is_finite() { sample } else { 0.0 };
        match descriptor.role {
            WindowsStaticObjectRole::FrontLeft => left += sample,
            WindowsStaticObjectRole::FrontRight => right += sample,
            WindowsStaticObjectRole::FrontCenter => {
                left += sample * 0.707_106_77;
                right += sample * 0.707_106_77;
            }
            WindowsStaticObjectRole::LowFrequency => {
                left += sample * 0.5;
                right += sample * 0.5;
            }
            WindowsStaticObjectRole::SideLeft | WindowsStaticObjectRole::BackLeft => {
                left += sample * 0.707_106_77;
            }
            WindowsStaticObjectRole::SideRight | WindowsStaticObjectRole::BackRight => {
                right += sample * 0.707_106_77;
            }
            WindowsStaticObjectRole::BackCenter => {
                left += sample * 0.5;
                right += sample * 0.5;
            }
            WindowsStaticObjectRole::TopFrontLeft
            | WindowsStaticObjectRole::TopBackLeft
            | WindowsStaticObjectRole::BottomFrontLeft
            | WindowsStaticObjectRole::BottomBackLeft => left += sample * 0.5,
            WindowsStaticObjectRole::TopFrontRight
            | WindowsStaticObjectRole::TopBackRight
            | WindowsStaticObjectRole::BottomFrontRight
            | WindowsStaticObjectRole::BottomBackRight => right += sample * 0.5,
        }
    }

    let peak = left.abs().max(right.abs());
    if peak > 1.0 {
        [left / peak, right / peak]
    } else {
        [left, right]
    }
}

struct AsyncStaticObjects {
    input: Arc<AudioRing>,
    output: Arc<AudioRing>,
    stop: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    processed_blocks: Arc<AtomicU64>,
    worker: Option<JoinHandle<()>>,
    fallback_delay: StereoDelay,
    missed_rendered_frames: usize,
    descriptors: Vec<StaticDescriptor>,
    frames_per_quantum: usize,
}

impl AsyncStaticObjects {
    fn new(
        sample_rate_hz: u32,
        frames_per_quantum: usize,
        descriptors: Vec<StaticDescriptor>,
    ) -> Result<Self, String> {
        if sample_rate_hz != 48_000 {
            return Err("Windows spatial object ABI currently requires 48 kHz".to_string());
        }
        if frames_per_quantum == 0 || frames_per_quantum > 4096 {
            return Err("invalid spatial object quantum size".to_string());
        }

        let object_count = descriptors.len();
        let quantum_samples = frames_per_quantum
            .checked_mul(object_count)
            .ok_or_else(|| "spatial object quantum size overflow".to_string())?;
        let input_capacity = (sample_rate_hz as usize)
            .saturating_mul(object_count)
            .saturating_mul(RING_SECONDS);
        let output_capacity = (sample_rate_hz as usize)
            .saturating_mul(2)
            .saturating_mul(RING_SECONDS);

        let input = Arc::new(AudioRing::new(input_capacity.max(quantum_samples * 2)));
        let output = Arc::new(AudioRing::new(output_capacity));
        let stop = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let processed_blocks = Arc::new(AtomicU64::new(0));
        let (init_tx, init_rx) = sync_channel::<Result<(), String>>(1);

        let input_worker = Arc::clone(&input);
        let output_worker = Arc::clone(&output);
        let stop_worker = Arc::clone(&stop);
        let failed_worker = Arc::clone(&failed);
        let blocks_worker = Arc::clone(&processed_blocks);
        let worker_descriptors = descriptors.clone();

        let worker = thread::Builder::new()
            .name("omniphony-spatial-static".to_string())
            .spawn(move || {
                let mut pipeline = match WindowsSpatialObjectPipeline::new(sample_rate_hz) {
                    Ok(pipeline) => {
                        let _ = init_tx.send(Ok(()));
                        pipeline
                    }
                    Err(error) => {
                        let _ = init_tx.send(Err(error));
                        failed_worker.store(true, Ordering::Release);
                        return;
                    }
                };

                let mut planar = vec![0.0f32; quantum_samples];

                while !stop_worker.load(Ordering::Acquire) {
                    if input_worker.available() < quantum_samples {
                        thread::sleep(Duration::from_micros(250));
                        continue;
                    }
                    if input_worker.pop_slice(&mut planar) != quantum_samples {
                        continue;
                    }

                    let rendered = {
                        // These borrowed views must not outlive this quantum;
                        // the allocating renderer runs on this dedicated worker.
                        let mut objects = Vec::with_capacity(worker_descriptors.len());
                        for (index, descriptor) in worker_descriptors.iter().enumerate() {
                            let start = index * frames_per_quantum;
                            let end = start + frames_per_quantum;
                            objects.push(WindowsStaticObject {
                                role: descriptor.role,
                                windows_position: descriptor.position,
                                mono_pcm: &planar[start..end],
                            });
                        }

                        objects.sort_by_key(|object| object.role.canonical_scene_index());
                        match pipeline.process(&objects, &[]) {
                            Ok(rendered) => rendered,
                            Err(_) => {
                                failed_worker.store(true, Ordering::Release);
                                return;
                            }
                        }
                    };
                    blocks_worker.fetch_add(1, Ordering::Relaxed);

                    if rendered.is_empty() {
                        continue;
                    }
                    while !stop_worker.load(Ordering::Acquire) {
                        if output_worker.free() >= rendered.len() && output_worker.push_slice(&rendered) {
                            break;
                        }
                        thread::sleep(Duration::from_micros(250));
                    }
                }
            })
            .map_err(|error| error.to_string())?;

        match init_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(())) => Ok(Self {
                input,
                output,
                stop,
                failed,
                processed_blocks,
                worker: Some(worker),
                fallback_delay: StereoDelay::new(sample_rate_hz),
                missed_rendered_frames: 0,
                descriptors,
                frames_per_quantum,
            }),
            Ok(Err(error)) => {
                stop.store(true, Ordering::Release);
                let _ = worker.join();
                Err(error)
            }
            Err(error) => {
                stop.store(true, Ordering::Release);
                let _ = worker.join();
                Err(format!("static spatial renderer initialization timed out: {error}"))
            }
        }
    }

    fn latency_frames(&self) -> usize {
        self.fallback_delay.delay_frames()
    }

    unsafe fn process_raw(
        &mut self,
        input_planar: *const f32,
        output_stereo: *mut f32,
        frames: usize,
    ) -> i32 {
        if frames != self.frames_per_quantum {
            return -10;
        }
        let Some(input_samples) = frames.checked_mul(self.descriptors.len()) else {
            return -11;
        };

        let mut use_rendered = !self.failed.load(Ordering::Acquire);
        if use_rendered && !unsafe { self.input.push_ptr(input_planar, input_samples) } {
            self.failed.store(true, Ordering::Release);
            use_rendered = false;
        }

        for frame_index in 0..frames {
            let fallback = safety_downmix_frame(
                &self.descriptors,
                input_planar,
                frames,
                frame_index,
            );
            let (delayed, primed) = self.fallback_delay.push(fallback);
            let mut rendered = if primed { delayed } else { [0.0, 0.0] };

            if primed && use_rendered && !self.failed.load(Ordering::Acquire) {
                while self.missed_rendered_frames > 0 && self.output.available() >= 2 {
                    if self.output.discard(2) != 2 {
                        break;
                    }
                    self.missed_rendered_frames -= 1;
                }

                if self.missed_rendered_frames == 0 && self.output.available() >= 2 {
                    let mut object_render = [0.0f32; 2];
                    if self.output.pop_slice(&mut object_render) == 2 {
                        rendered = object_render;
                    } else {
                        self.missed_rendered_frames = self.missed_rendered_frames.saturating_add(1);
                    }
                } else {
                    self.missed_rendered_frames = self.missed_rendered_frames.saturating_add(1);
                }
            }

            rendered[0] = if rendered[0].is_finite() { rendered[0] } else { 0.0 };
            rendered[1] = if rendered[1].is_finite() { rendered[1] } else { 0.0 };
            let peak = rendered[0].abs().max(rendered[1].abs());
            if peak > OUTPUT_CEILING {
                let gain = OUTPUT_CEILING / peak;
                rendered[0] *= gain;
                rendered[1] *= gain;
            }

            unsafe {
                *output_stereo.add(frame_index * 2) = rendered[0];
                *output_stereo.add(frame_index * 2 + 1) = rendered[1];
            }
        }
        0
    }
}

impl Drop for AsyncStaticObjects {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub struct OmniphonySpatialStaticProcessor {
    sample_rate_hz: u32,
    frames_per_quantum: u32,
    object_count: u32,
    inner: AsyncStaticObjects,
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

        let descriptors = unsafe {
            std::slice::from_raw_parts(config.objects, config.object_count as usize)
        };
        let Ok(descriptors) = copy_descriptors(descriptors) else {
            return ptr::null_mut();
        };
        let Ok(inner) = AsyncStaticObjects::new(
            config.sample_rate_hz,
            config.frames_per_quantum as usize,
            descriptors,
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
        unsafe { (*processor).inner.processed_blocks.load(Ordering::Relaxed) }
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
        unsafe { processor.inner.process_raw(input_planar, output_stereo, frames) }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_sample_rate_hz(
    processor: *const OmniphonySpatialStaticProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).sample_rate_hz } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_frames_per_quantum(
    processor: *const OmniphonySpatialStaticProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).frames_per_quantum } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_static_object_count(
    processor: *const OmniphonySpatialStaticProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).object_count } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_role_indices_are_total_and_bounded() {
        for index in 0..17 {
            assert_eq!(
                WindowsStaticObjectRole::from_canonical_scene_index(index)
                    .unwrap()
                    .canonical_scene_index(),
                index as usize
            );
        }
        assert!(WindowsStaticObjectRole::from_canonical_scene_index(17).is_none());
        assert!(WindowsStaticObjectRole::from_canonical_scene_index(u32::MAX).is_none());
    }

    #[test]
    fn descriptor_copy_rejects_duplicates_and_non_finite_positions() {
        let front = OmniphonySpatialStaticObjectDescriptor {
            role: 0,
            x_right_m: -0.7,
            y_up_m: 0.0,
            z_back_m: -0.7,
        };
        assert!(copy_descriptors(&[front, front]).is_err());

        let bad = OmniphonySpatialStaticObjectDescriptor {
            role: 1,
            x_right_m: f32::NAN,
            y_up_m: 0.0,
            z_back_m: -0.7,
        };
        assert!(copy_descriptors(&[bad]).is_err());
    }

    #[test]
    fn lfe_position_is_ignored_by_contract() {
        let lfe = OmniphonySpatialStaticObjectDescriptor {
            role: 3,
            x_right_m: f32::NAN,
            y_up_m: f32::INFINITY,
            z_back_m: f32::NEG_INFINITY,
        };
        let copied = copy_descriptors(&[lfe]).unwrap();
        assert_eq!(copied[0].role, WindowsStaticObjectRole::LowFrequency);
        assert_eq!(copied[0].position, None);
    }
}
